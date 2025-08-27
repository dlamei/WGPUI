use glam::{Mat4, UVec2, UVec4, Vec2, Vec4};
use macros::vertex;
use rustc_hash::FxHashMap;
use wgpu::util::DeviceExt;

use std::{
    collections::VecDeque,
    fmt,
    hash::{Hash, Hasher},
    ops,
    time::{Duration, Instant},
};

use crate::{
    RGBA, RenderPassHandle, ShaderGenerics, ShaderHandle, VertexPosCol,
    gpu::{self, Vertex as VertexTyp, VertexDesc, WGPU},
    rect::Rect,
};

#[vertex]
pub struct Vertex {
    pub pos: Vec2,
    pub col: RGBA,
}

#[derive(Debug, Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GlobalUniform {
    pub proj: glam::Mat4,
}

impl GlobalUniform {
    pub fn build_bind_group(&self, wgpu: &WGPU) -> wgpu::BindGroup {
        let global_uniform = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_global_uniform_buffer"),
                contents: bytemuck::cast_slice(&[*self]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let global_bind_group_layout =
            wgpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("global_bind_group_layout"),
                });

        wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("global_bind_group"),
            layout: &global_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: global_uniform.as_entire_binding(),
            }],
        })
    }
}

fn vec2_to_point(v: Vec2) -> lyon::geom::Point<f32> {
    lyon::geom::Point::new(v.x, v.y)
}

fn path_from_points(points: &[Vec2], closed: bool) -> lyon::path::Path {
    let mut builder = lyon::path::Path::builder();
    if points.is_empty() {
        return builder.build();
    }
    builder.begin(vec2_to_point(points[0]));
    for &p in &points[1..] {
        builder.line_to(vec2_to_point(p));
    }
    builder.end(closed);
    builder.build()
}

pub fn tessellate_line(
    points: &[Vec2],
    col: RGBA,
    thickness: f32,
    is_closed: bool,
) -> (Vec<Vertex>, Vec<u32>) {
    use lyon::tessellation::{
        BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
    };
    if points.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let path = path_from_points(points, is_closed);

    let mut buffers = VertexBuffers::<Vertex, u32>::new();
    let mut tess = StrokeTessellator::new();
    let options = StrokeOptions::default()
        .with_line_width(thickness)
        .with_line_join(lyon::path::LineJoin::Round);

    let mut builder = BuffersBuilder::new(&mut buffers, |v: StrokeVertex| Vertex {
        pos: Vec2::new(v.position().x, v.position().y),
        col,
    });

    if let Err(e) = tess.tessellate_path(path.as_slice(), &options, &mut builder) {
        log::error!("Stroke tessellation failed: {:?}", e);
        return (Vec::new(), Vec::new());
    }

    (buffers.vertices, buffers.indices)
}

pub fn tessellate_fill(points: &[Vec2], fill: RGBA) -> (Vec<Vertex>, Vec<u32>) {
    use lyon::tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    };
    if points.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let path = path_from_points(points, true);

    let mut buffers = VertexBuffers::<Vertex, u32>::new();
    let mut tess = FillTessellator::new();
    let mut builder = BuffersBuilder::new(&mut buffers, |v: FillVertex| Vertex {
        pos: Vec2::new(v.position().x, v.position().y),
        col: fill,
    });

    if let Err(e) = tess.tessellate_path(&path, &FillOptions::default(), &mut builder) {
        log::error!("Fill tessellation failed: {:?}", e);
        return (Vec::new(), Vec::new());
    }

    (buffers.vertices, buffers.indices)
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawRect {
    pub rect: Rect,
    pub fill: Option<RGBA>,
    pub outline: Option<(RGBA, f32)>,
    pub corner_radius: f32,
}

impl Rect {
    pub fn draw(self) -> DrawRect {
        DrawRect::new(self)
    }
}

impl DrawRect {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            fill: None,
            outline: None,
            corner_radius: 0.0,
        }
    }

    pub fn fill(mut self, fill: RGBA) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn outline(mut self, col: RGBA, width: f32) -> Self {
        self.outline = Some((col, width));
        self
    }

    pub fn corner_radius(mut self, rad: f32) -> Self {
        self.corner_radius = rad;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawList {
    pub vtx_buffer: Vec<Vertex>,
    pub idx_buffer: Vec<u32>,
    pub screen_size: Vec2,

    pub path: Vec<Vec2>,
    pub path_closed: bool,

    pub resolution: f32,
}

fn vtx(pos: impl Into<Vec2>, col: impl Into<RGBA>) -> Vertex {
    Vertex {
        pos: pos.into(),
        col: col.into(),
    }
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            vtx_buffer: Vec::new(),
            idx_buffer: Vec::new(),
            screen_size: Vec2::ONE,
            path: Vec::new(),
            path_closed: false,
            resolution: 16.0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.vtx_buffer.clear();
        self.idx_buffer.clear();
        self.path_clear();
    }

    pub fn add_rect(&mut self, dr: DrawRect) {
        self.path_rect(dr.rect.min, dr.rect.max, dr.corner_radius);

        if let Some(fill) = dr.fill {
            let (vtx, idx) = tessellate_fill(&self.path, fill);
            let off = self.vtx_buffer.len() as u32;
            self.vtx_buffer.extend(vtx);
            self.idx_buffer.extend(idx.into_iter().map(|i| i + off));
        }

        if let Some((col, width)) = dr.outline {
            let (vtx, idx) = tessellate_line(&self.path, col, width, true);
            let off = self.vtx_buffer.len() as u32;
            self.vtx_buffer.extend(vtx);
            self.idx_buffer.extend(idx.into_iter().map(|i| i + off));
        }

        self.path_clear();
    }

    pub fn path_arc_around(
        &mut self,
        center: Vec2,
        radius: f32,
        start_angle: f32,
        sweep_angle: f32,
        // segments: usize,
    ) {
        if radius <= 0.0 || sweep_angle == 0.0 {
            return;
        }

        let arc_len = radius * sweep_angle.abs();
        let segments = (arc_len / self.resolution).ceil() as usize;
        if segments == 0 {
            return;
        }

        let step = sweep_angle / segments as f32;
        for i in 0..=segments {
            let theta = start_angle + step * (i as f32);
            let p = Vec2::new(
                center.x + theta.cos() * radius,
                center.y - theta.sin() * radius,
            );
            self.path.push(p);
        }
    }

    pub fn path_rect(&mut self, min: Vec2, max: Vec2, rad: f32) {
        const PI: f32 = std::f32::consts::PI;
        let rounded = rad != 0.0;
        // let segs = 8;

        self.path_to(Vec2::new(min.x + rad, min.y));
        self.path_to(Vec2::new(max.x - rad, min.y));
        if rounded {
            self.path_arc_around(
                Vec2::new(max.x - rad, min.y + rad),
                rad,
                PI / 2.0,
                -PI / 2.0,
                // segs,
            );
        }

        self.path_to(Vec2::new(max.x, min.y + rad));
        self.path_to(Vec2::new(max.x, max.y - rad));
        if rounded {
            self.path_arc_around(
                Vec2::new(max.x - rad, max.y - rad),
                rad,
                0.0,
                -PI / 2.0,
                // segs,
            );
        }

        self.path_to(Vec2::new(max.x - rad, max.y));
        self.path_to(Vec2::new(min.x + rad, max.y));
        if rounded {
            self.path_arc_around(
                Vec2::new(min.x + rad, max.y - rad),
                rad,
                -PI / 2.0,
                -PI / 2.0,
                // segs,
            );
        }

        self.path_to(Vec2::new(min.x, max.y - rad));
        self.path_to(Vec2::new(min.x, min.y + rad));
        if rounded {
            self.path_arc_around(
                Vec2::new(min.x + rad, min.y + rad),
                rad,
                PI,
                -PI / 2.0,
                // segs,
            );
        }

        self.path_close();
    }

    pub fn path_clear(&mut self) {
        self.path.clear();
        self.path_closed = false;
    }

    pub fn path_to(&mut self, p: Vec2) {
        self.path.push(p);
    }

    pub fn path_close(&mut self) {
        self.path_closed = true;
    }

    pub fn build_path_stroke_multi_color(&mut self, thickness: f32, cols: &[RGBA]) {
        if cols.is_empty() {
            return;
        }
        let (vtx, idx) = tessellate_line(&self.path, cols[0], thickness, self.path_closed);
        let offset = self.vtx_buffer.len() as u32;
        self.vtx_buffer
            .extend(vtx.into_iter().enumerate().map(|(i, mut v)| {
                v.col = cols[i % cols.len()];
                v
            }));
        self.idx_buffer.extend(idx.into_iter().map(|i| i + offset));
        self.path_clear();
    }

    pub fn build_path_stroke(&mut self, thickness: f32, col: RGBA) {
        let (vtx, idx) = tessellate_line(&self.path, col, thickness, self.path_closed);
        let offset = self.vtx_buffer.len() as u32;
        self.vtx_buffer.extend(vtx.into_iter().map(|mut v| {
            v.col = col;
            v
        }));
        self.idx_buffer.extend(idx.into_iter().map(|i| i + offset));
        self.path_clear();
    }

    pub fn debug_wireframe(&mut self, thickness: f32) {
        self.path_clear();

        let mut vtx_buffer = Vec::new();
        std::mem::swap(&mut vtx_buffer, &mut self.vtx_buffer);
        let mut idx_buffer = Vec::new();
        std::mem::swap(&mut idx_buffer, &mut self.idx_buffer);

        for idxs in idx_buffer.chunks_exact(3) {
            let v0 = vtx_buffer[idxs[0] as usize];
            let v1 = vtx_buffer[idxs[1] as usize];
            let v2 = vtx_buffer[idxs[2] as usize];
            let cols = [v0.col, v1.col, v2.col, v0.col];
            self.path
                .extend_from_slice(&[v0.pos, v1.pos, v2.pos, v0.pos]);
            self.build_path_stroke_multi_color(thickness, &cols);
        }
    }
}

impl RenderPassHandle for DrawList {
    fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, wgpu: &WGPU) {
        let vtx = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui_vtx_buffer"),
                contents: &bytemuck::cast_slice(&self.vtx_buffer),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let idx = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui_idx_buffer"),
                contents: &bytemuck::cast_slice(&self.idx_buffer),
                usage: wgpu::BufferUsages::INDEX,
            });

        let uniform = GlobalUniform {
            proj: Mat4::orthographic_lh(
                0.0,
                self.screen_size.x,
                self.screen_size.y,
                0.0,
                -1.0,
                0.0,
            ),
        }
        .build_bind_group(wgpu);

        rpass.set_bind_group(0, &uniform, &[]);

        rpass.set_vertex_buffer(0, vtx.slice(..));
        rpass.set_index_buffer(idx.slice(..), wgpu::IndexFormat::Uint32);

        rpass.set_pipeline(&UiShader.get_pipeline(&[(&Vertex::desc(), "Vertex")], wgpu));

        rpass.draw_indexed(0..self.idx_buffer.len() as u32, 0, 0..1);
    }
}

pub struct UiShader;

impl ShaderHandle for UiShader {
    const RENDER_PIPELINE_ID: crate::ShaderID = "ui_shader";

    fn build_pipeline(&self, desc: &ShaderGenerics<'_>, wgpu: &WGPU) -> wgpu::RenderPipeline {
        const SHADER_SRC: &str = r#"


            @rust struct Vertex {
                pos: vec2<f32>,
                col: vec4<f32>,
                ...
            }

            struct GlobalUniform {
                proj: mat4x4<f32>,
            }

            @group(0) @binding(0)
            var<uniform> global: GlobalUniform;

            struct VSOut {
                @builtin(position) pos: vec4<f32>,
                @location(0) color: vec4<f32>,
            };

            @vertex
                fn vs_main(
                    v: Vertex,
                ) -> VSOut {
                    var out: VSOut;
                    out.color = v.col;
                    out.pos = global.proj * vec4(v.pos, 0.0, 1.0);

                    return out;
                }


            @fragment
                fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
                    return in.color;
                }
            "#;

        let global_bind_group_layout =
            wgpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("global_bind_group_layout"),
                });

        let shader_src = gpu::process_shader_code(SHADER_SRC, &desc).unwrap();
        let vertices = desc.iter().map(|d| d.0).collect::<Vec<_>>();
        gpu::PipelineBuilder::new(&shader_src, wgpu.surface_format)
            .label("rect_pipeline")
            .vertex_buffers(&vertices)
            .bind_groups(&[&global_bind_group_layout])
            .sample_count(gpu::Renderer::multisample_count())
            .build(&wgpu.device)
    }
}
