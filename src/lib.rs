mod gpu;
mod ui;
mod rect;

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use glam::{UVec2, Vec2, Vec3, Vec4};
use gpu::{PipelineID, PipelineRegistry, WGPU};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::WindowEvent,
    event_loop::ActiveEventLoop, window::Window,
};

use macros::vertex_struct;

pub extern crate self as wgpui;

pub use gpu::AsVertexFormat;

pub enum AppSetup {
    UnInit {
        window: Option<Arc<Window>>,
        #[cfg(target_arch = "wasm32")]
        renderer_rec: Option<futures::channel::oneshot::Receiver<Renderer>>,
    },
    Init(App),
}

impl Default for AppSetup {
    fn default() -> Self {
        Self::UnInit {
            window: None,
            #[cfg(target_arch = "wasm32")]
            renderer_rec: None,
        }
    }
}

impl AppSetup {
    pub fn is_init(&self) -> bool {
        matches!(self, Self::Init(_))
    }

    pub fn init_app(window: Arc<Window>, renderer: Renderer) -> App {
        let scale_factor = window.scale_factor() as f32;

        let wgpu = &renderer.wgpu;

        App {
            renderer,
            window,
            last_size: UVec2::ONE,
            prev_frame_time: Instant::now(),
            delta_time: Duration::ZERO,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn resumed_native(&mut self, event_loop: &ActiveEventLoop) {
        if self.is_init() {
            return;
        }

        let window = event_loop
            .create_window(winit::window::Window::default_attributes().with_title("Atlas"))
            .unwrap();

        let window_handle = Arc::new(window);
        // self.window = Some(window_handle.clone());

        let size = window_handle.inner_size();
        let scale_factor = window_handle.scale_factor() as f32;

        let window_handle_2 = window_handle.clone();
        let renderer = pollster::block_on(async move {
            Renderer::new_async(window_handle_2, size.width, size.height).await
        });

        *self = Self::Init(Self::init_app(window_handle, renderer));
    }

    fn try_init(&mut self) -> Option<&mut App> {
        if let Self::Init(app) = self {
            return Some(app);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let Self::UnInit {
                window,
                renderer_rec,
            } = self
            else {
                panic!();
            };
            // let mut renderer_received = false;
            if let Some(receiver) = renderer_rec.as_mut() {
                if let Ok(Some(renderer)) = receiver.try_recv() {
                    *self = Self::Init(Self::init_app(window.as_ref().unwrap().clone(), renderer));
                    if let Self::Init(app) = self {
                        return Some(app);
                    }
                }
            }
        }

        None
    }
}

impl ApplicationHandler for AppSetup {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(not(target_arch = "wasm32"))]
        self.resumed_native(event_loop);
        #[cfg(target_arch = "wasm32")]
        self.resumed_wasm(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(app) = self.try_init() {
            app.on_window_event(event_loop, window_id, event);
        }
    }

    // fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    //         println!("waiting... ");
    //     if let Some(app) = self.try_init() {
    //         app.window.request_redraw();
    //     }
    // }
}

pub struct App {
    renderer: Renderer,

    prev_frame_time: Instant,
    delta_time: Duration,

    last_size: UVec2,
    window: Arc<Window>,
}

impl App {
    fn on_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        use WindowEvent as WE;
        if self.window.id() != window_id {
            return;
        }

        let clear_screen = ClearScreen::new(RGBA::hex("#242933"));
        let dbg_tri = DebugTriangle::new(&self.renderer.wgpu);

        match event {
            WE::RedrawRequested => {
                self.window.pre_present_notify();

                let status = self.renderer.prepare_frame();
                match status {
                    Ok(_) => (),
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size = self.window.inner_size();
                        self.renderer.resize(size.width, size.height);
                        return;
                    }
                    Err(e) => {
                        log::error!("prepare_frame: {e}");
                        panic!();
                    }
                }

                self.on_redraw(event_loop);
                {
                    let mut surface = self.renderer.surface_target();
                    surface.render(&clear_screen);
                    surface.render(&dbg_tri);
                }

                self.renderer.present_frame();
            }
            WE::Resized(PhysicalSize { width, height }) => {
                let (width, height) = (width.max(1), height.max(1));
                self.last_size = (width, height).into();
                self.resize(width, height);
            }
            WE::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    fn on_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let prev_time = self.prev_frame_time;
        let curr_time = Instant::now();
        let dt = curr_time - prev_time;
        self.prev_frame_time = curr_time;
        self.delta_time = dt;
    }

    fn resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }
}

vertex_struct!(VertexPosCol {
    pos(0): Vec4,
    col(1): RGBA,
});


pub struct DebugTriangle {
    vertex_buffer: wgpu::Buffer,
}

impl DebugTriangle {
    pub fn new(wgpu: &WGPU) -> Self {
        let vertices = [
            VertexPosCol {
                pos: [-0.5, -0.5, 0.0, 1.0].into(),
                col: RGBA::RED,
            },
            VertexPosCol {
                pos: [0.0, 0.5, 0.0, 1.0].into(),
                col: RGBA::GREEN, // green
            },
            VertexPosCol {
                pos: [0.5, -0.25, 0.0, 1.0].into(),
                col: RGBA::BLUE, // blue
            },
        ];

        let vertex_buffer = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("debug_triangle_vertex_buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        Self { vertex_buffer }
    }
}

impl RenderPassInst for DebugTriangle {
    fn load_op(&self) -> wgpu::LoadOp<wgpu::Color> {
        wgpu::LoadOp::Load
    }

    fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.draw(0..3, 0..1);
    }

    fn render_pipeline_id() -> PipelineID {
        PipelineID::DebugTriangle
    }

    fn load_render_pipeline(wgpu: &WGPU) -> wgpu::RenderPipeline {
        const SHADER_SRC: &str = r#"
        struct VSOut {
            @builtin(position) pos: vec4<f32>,
            @location(0) color: vec4<f32>,
        };
        
        @vertex
        fn vs_main(
            @location(0) position: vec3<f32>,
            @location(1) color: vec4<f32>
        ) -> VSOut {
            var out: VSOut;
            out.pos = vec4<f32>(position, 1.0);
            out.color = color;
            return out;
        }
        
        @fragment
        fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
            return in.color;
        }
        "#;

        gpu::PipelineBuilder::new(SHADER_SRC, wgpu.surface_format)
            .label("debug_triangle_pipeline")
            .vertex_buffers(&[VertexPosCol::buffer_layout()])
            .build(&wgpu.device)
    }
}

#[derive(Debug, Clone)]
pub struct ClearScreen {
    clear_col: RGBA,
}

impl RenderPassInst for ClearScreen {
    fn load_op(&self) -> wgpu::LoadOp<wgpu::Color> {
        wgpu::LoadOp::Clear(self.clear_col.into())
    }

    fn store_op(&self) -> wgpu::StoreOp {
        wgpu::StoreOp::Store
    }

    fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        // rpass.set_pipeline(&self.pipeline);
        rpass.draw(0..0, 0..0);
    }

    fn render_pipeline_id() -> PipelineID {
        PipelineID::ClearScreen
    }

    fn load_render_pipeline(wgpu: &WGPU) -> wgpu::RenderPipeline {
        const SHADER_SRC: &'static str = r#"
        @vertex
            fn vs_main() -> @builtin(position) vec4<f32> {
                return vec4(0);
            }
        @fragment
            fn fs_main() -> @location(0) vec4<f32> {
                return vec4<f32>(1.0, 1.0, 1.0, 1.0);
            }
        "#;

        gpu::PipelineBuilder::new(SHADER_SRC, wgpu.surface_format)
            .label("debug_triangle_pipeline")
            .vertex_buffers(&[])
            .build(&wgpu.device)
    }
}

impl ClearScreen {
    pub fn new(clear_col: RGBA) -> Self {
        Self { clear_col }
    }
}

pub trait PipelineInst {
    fn pipeline_id(&self) -> PipelineID;
    fn compile_pipeline(&self, wgpu: &WGPU) -> wgpu::RenderPipeline;

    fn get_pipeline(&self, wgpu: &WGPU) -> Arc<wgpu::RenderPipeline> {
        wgpu.get_or_init_pipeline(self.pipeline_id(), || {
            self.compile_pipeline(wgpu)
        })
    }
}

pub trait RenderPassInst {
    fn load_op(&self) -> wgpu::LoadOp<wgpu::Color> {
        wgpu::LoadOp::Load
    }
    fn store_op(&self) -> wgpu::StoreOp {
        wgpu::StoreOp::Store
    }

    fn render_pipeline_id() -> PipelineID;
    fn load_render_pipeline(wgpu: &WGPU) -> wgpu::RenderPipeline;

    fn render_pipeline(wgpu: &WGPU) -> Arc<wgpu::RenderPipeline> {
        wgpu.get_or_init_pipeline(Self::render_pipeline_id(), || {
            Self::load_render_pipeline(wgpu)
        })
    }

    fn render<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>);
}

pub struct Renderer {
    framebuffer_msaa: Option<wgpu::TextureView>,
    framebuffer_resolve: wgpu::TextureView,
    depthbuffer: wgpu::TextureView,
    active_surface: Option<wgpu::SurfaceTexture>,
    wgpu: WGPU,
}

pub struct RenderTarget<'a> {
    target_view: wgpu::TextureView,
    encoder: std::mem::ManuallyDrop<wgpu::CommandEncoder>,
    wgpu: &'a WGPU,
}

impl<'a> Drop for RenderTarget<'a> {
    fn drop(&mut self) {
        unsafe {
            let encoder = std::mem::ManuallyDrop::take(&mut self.encoder);
            self.wgpu.queue.submit(Some(encoder.finish()));
        }
        //     let encoder = std::ptr::read(&*self.encoder);
        //     let finished = encoder.finish();
        //     self.wgpu.queue.submit(Some(finished));
        // }
    }
}

impl<'a> RenderTarget<'a> {
    pub fn render<T: RenderPassInst>(&mut self, obj: &T) {
        let mut rpass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: obj.load_op(),
                    store: obj.store_op(),
                },
            })],
            depth_stencil_attachment: None,
            label: Some("main render pass"),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&T::render_pipeline(&self.wgpu));
        obj.render(&mut rpass);
    }
}

impl Renderer {
    pub fn surface_target(&mut self) -> RenderTarget<'_> {
        let Some(surface_texture) = &mut self.active_surface else {
            log::error!("Renderer::prepare_frame must be called before calling this function");
            panic!();
        };

        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: wgpu::Label::default(),
                    aspect: wgpu::TextureAspect::default(),
                    format: Some(self.wgpu.surface_format),
                    dimension: None,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                });

        let encoder = self
            .wgpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("renderpass_encoder"),
            });

        RenderTarget {
            target_view: surface_texture_view,
            encoder: std::mem::ManuallyDrop::new(encoder),
            wgpu: &self.wgpu,
        }
    }

    pub fn prepare_frame(&mut self) -> Result<(), wgpu::SurfaceError> {
        if self.active_surface.is_some() {
            log::error!("Renderer::prepare_frame called with active surface!");
            panic!();
        }

        let surface_texture = self.wgpu.surface.get_current_texture()?;

        self.active_surface = Some(surface_texture);
        Ok(())
    }

    pub fn present_frame(&mut self) {
        if let Some(surface) = self.active_surface.take() {
            surface.present();
            self.active_surface = None;
        }
    }

    pub async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let wgpu = WGPU::new_async(window, width, height).await;

        let framebuffer_msaa = Self::create_framebuffer_msaa_texture(&wgpu, width, height);
        let framebuffer_resolve = Self::create_framebuffer_resolve_texture(&wgpu, width, height);
        let depthbuffer = Self::create_depthbuffer(&wgpu, width, height);

        Self {
            framebuffer_msaa,
            framebuffer_resolve,
            depthbuffer,
            active_surface: None,
            wgpu,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.wgpu.resize(width, height);
        self.framebuffer_msaa = Self::create_framebuffer_msaa_texture(&self.wgpu, width, height);
        self.framebuffer_resolve =
            Self::create_framebuffer_resolve_texture(&self.wgpu, width, height);
        self.depthbuffer = Self::create_depthbuffer(&self.wgpu, width, height);
    }

    pub fn create_framebuffer_resolve_texture(
        wgpu: &WGPU,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let width = width.max(1);
        let height = height.max(1);
        let texture = wgpu.device.create_texture(
            &(wgpu::TextureDescriptor {
                label: Some("Framebuffer Resolve Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu.surface_format),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
            usage: None,
        })
    }

    pub fn depth_format() -> wgpu::TextureFormat {
        wgpu::TextureFormat::Depth32Float
    }

    pub const fn use_multisample() -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return true;
        #[cfg(target_arch = "wasm32")]
        return false;
    }

    pub fn multisample_state() -> wgpu::MultisampleState {
        if Self::use_multisample() {
            wgpu::MultisampleState {
                mask: !0,
                alpha_to_coverage_enabled: false,
                count: 4,
            }
        } else {
            Default::default()
        }
    }

    pub fn create_framebuffer_msaa_texture(
        wgpu: &WGPU,
        width: u32,
        height: u32,
    ) -> Option<wgpu::TextureView> {
        let width = width.max(1);
        let height = height.max(1);
        if !Self::use_multisample() {
            return None;
        }

        let texture = wgpu.device.create_texture(
            &(wgpu::TextureDescriptor {
                label: Some("Framebuffer Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        Some(texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu.surface_format),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
            usage: None,
        }))
    }

    pub fn create_depthbuffer(wgpu: &WGPU, width: u32, height: u32) -> wgpu::TextureView {
        let width = width.max(1);
        let height = height.max(1);
        let texture = wgpu.device.create_texture(
            &(wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: if Self::use_multisample() { 4 } else { 1 },
                dimension: wgpu::TextureDimension::D2,
                format: Self::depth_format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::depth_format()),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
            usage: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct RGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl fmt::Display for RGBA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 1.0 {
            write!(f, "({:.2}, {:.2}, {:.2})", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "({:.2}, {:.2}, {:.2}, {:.2})",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

impl RGBA {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba_f(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba_f(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    pub const fn rgb_f(r: f32, g: f32, b: f32) -> Self {
        Self::rgba_f(r, g, b, 1.0)
    }

    pub const fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn hex(hex: &str) -> Self {
        fn to_linear(u: u8) -> f32 {
            let srgb = u as f32 / 255.0;
            if srgb <= 0.04045 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        }

        let hex = hex.trim_start_matches('#');
        let vals: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();

        let (r8, g8, b8, a8) = match vals.as_slice() {
            [r, g, b] => (*r, *g, *b, 255),
            [r, g, b, a] => (*r, *g, *b, *a),
            _ => panic!("Hex code must be 6 or 8 characters long"),
        };

        Self::rgba_f(
            to_linear(r8),
            to_linear(g8),
            to_linear(b8),
            a8 as f32 / 255.0,
        )
    }

    pub const RED: RGBA = RGBA::rgb(255, 0, 0);
    pub const GREEN: RGBA = RGBA::rgb(0, 255, 0);
    pub const BLUE: RGBA = RGBA::rgb(0, 0, 255);

    pub const WHITE: RGBA = RGBA::rgb(255, 255, 255);
    pub const BLACK: RGBA = RGBA::rgb(0, 0, 0);

    pub const DEBUG: RGBA = RGBA::rgb(200, 0, 100);

    pub const ZERO: RGBA = RGBA::rgba(0, 0, 0, 0);
}

impl From<RGBA> for wgpu::Color {
    fn from(c: RGBA) -> Self {
        wgpu::Color {
            r: c.r as f64,
            g: c.g as f64,
            b: c.b as f64,
            a: c.a as f64,
        }
    }
}

pub fn hex_to_col(hex: &str) -> wgpu::Color {
    fn to_linear(u: u8) -> f64 {
        let srgb = u as f64 / 255.0;
        if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    }

    let hex = hex.trim_start_matches('#');
    let vals: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();

    let (r8, g8, b8, a8) = match vals.as_slice() {
        [r, g, b] => (*r, *g, *b, 255),
        [r, g, b, a] => (*r, *g, *b, *a),
        _ => panic!("Hex code must be 6 or 8 characters long"),
    };

    wgpu::Color {
        r: to_linear(r8),
        g: to_linear(g8),
        b: to_linear(b8),
        a: a8 as f64 / 255.0, // alpha is linear already
    }
}
