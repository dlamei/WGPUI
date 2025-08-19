use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub trait AsVertexFormat {
    const FORMAT: wgpu::VertexFormat;
}

macro_rules! impl_as_vertex_fmt {
    ($ty:ty: $fmt:ident) => {
        impl AsVertexFormat for $ty {
            const FORMAT: wgpu::VertexFormat = wgpu::VertexFormat::$fmt;
            // fn format() -> wgpu::VertexFormat {
            //     wgpu::VertexFormat::$fmt
            // }
        }
    };
}

// u8
impl_as_vertex_fmt!(u8: Uint8);
impl_as_vertex_fmt!([u8; 1]: Uint8);
impl_as_vertex_fmt!([u8; 2]: Uint8x2);
impl_as_vertex_fmt!([u8; 4]: Uint8x4);

// i8
impl_as_vertex_fmt!(i8: Sint8);
impl_as_vertex_fmt!([i8; 1]: Sint8);
impl_as_vertex_fmt!([i8; 2]: Sint8x2);
impl_as_vertex_fmt!([i8; 4]: Sint8x4);

// u16
impl_as_vertex_fmt!(u16: Uint16);
impl_as_vertex_fmt!([u16; 1]: Uint16);
impl_as_vertex_fmt!([u16; 2]: Uint16x2);
impl_as_vertex_fmt!([u16; 4]: Uint16x4);

// i16
impl_as_vertex_fmt!(i16: Sint16);
impl_as_vertex_fmt!([i16; 1]: Sint16);
impl_as_vertex_fmt!([i16; 2]: Sint16x2);
impl_as_vertex_fmt!([i16; 4]: Sint16x4);

// u32
impl_as_vertex_fmt!(u32: Uint32);
impl_as_vertex_fmt!([u32; 1]: Uint32);
impl_as_vertex_fmt!([u32; 2]: Uint32x2);
impl_as_vertex_fmt!([u32; 3]: Uint32x3);
impl_as_vertex_fmt!([u32; 4]: Uint32x4);

// i32
impl_as_vertex_fmt!(i32: Sint32);
impl_as_vertex_fmt!([i32; 1]: Sint32);
impl_as_vertex_fmt!([i32; 2]: Sint32x2);
impl_as_vertex_fmt!([i32; 3]: Sint32x3);
impl_as_vertex_fmt!([i32; 4]: Sint32x4);

// f32
impl_as_vertex_fmt!(f32: Float32);
impl_as_vertex_fmt!([f32; 1]: Float32);
impl_as_vertex_fmt!([f32; 2]: Float32x2);
impl_as_vertex_fmt!([f32; 3]: Float32x3);
impl_as_vertex_fmt!([f32; 4]: Float32x4);

impl_as_vertex_fmt!(f64: Float64);
impl_as_vertex_fmt!([f64; 1]: Float64);
impl_as_vertex_fmt!([f64; 2]: Float64x2);
impl_as_vertex_fmt!([f64; 3]: Float64x3);
impl_as_vertex_fmt!([f64; 4]: Float64x4);

impl_as_vertex_fmt!(glam::UVec2: Uint32x2);
impl_as_vertex_fmt!(glam::UVec3: Uint32x3);
impl_as_vertex_fmt!(glam::UVec4: Uint32x4);

impl_as_vertex_fmt!(glam::IVec2: Sint32x2);
impl_as_vertex_fmt!(glam::IVec3: Sint32x3);
impl_as_vertex_fmt!(glam::IVec4: Sint32x4);

impl_as_vertex_fmt!(glam::Vec2: Float32x2);
impl_as_vertex_fmt!(glam::Vec3: Float32x3);
impl_as_vertex_fmt!(glam::Vec4: Float32x4);
impl_as_vertex_fmt!(crate::RGBA: Float32x4);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PipelineID {
    ClearScreen,
    DebugTriangle,
}

pub struct PipelineRegistry {
    pub map: HashMap<PipelineID, Arc<wgpu::RenderPipeline>>,
}

impl PipelineRegistry {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn register(&mut self, id: PipelineID, pipeline: impl Into<Arc<wgpu::RenderPipeline>>) {
        self.map.insert(id, pipeline.into());
    }

    fn get(&self, id: PipelineID) -> Option<Arc<wgpu::RenderPipeline>> {
        self.map.get(&id).cloned()
    }

    /// lazy create helper (if you want one-shot creation)
    fn get_or_insert_with<F>(&mut self, id: PipelineID, load_fn: F) -> Arc<wgpu::RenderPipeline>
    where
        F: FnOnce() -> wgpu::RenderPipeline,
    {
        self.map
            .entry(id)
            .or_insert_with(|| Arc::new(load_fn()))
            .clone()
    }
}

pub struct WGPU {
    pub pipeline_reg: Mutex<PipelineRegistry>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

impl WGPU {
    pub fn width(&self) -> u32 {
        self.surface_config.width.max(1)
    }

    pub fn height(&self) -> u32 {
        self.surface_config.height.max(1)
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.width() as f32 / self.height() as f32
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn instance() -> wgpu::Instance {
        wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(any(target_os = "linux"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_os = "macos")]
            backends: wgpu::Backends::METAL,
            #[cfg(target_os = "windows")]
            backends: wgpu::Backends::DX12 | wgpu::Backends::GL,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        })
    }

    /// Register a new render pipeline with the given ID
    pub fn register_pipeline(&self, id: PipelineID, pipeline: wgpu::RenderPipeline) {
        self.pipeline_reg.lock().unwrap().register(id, pipeline);
    }

    /// Get a registered pipeline by ID
    pub fn get_pipeline(&self, id: PipelineID) -> Option<Arc<wgpu::RenderPipeline>> {
        self.pipeline_reg.lock().unwrap().get(id)
    }

    /// Get or create a pipeline
    pub fn get_or_init_pipeline<F>(&self, id: PipelineID, load: F) -> Arc<wgpu::RenderPipeline>
    where
        F: FnOnce() -> wgpu::RenderPipeline,
    {
        self.pipeline_reg
            .lock()
            .unwrap()
            .get_or_insert_with(id, load)
            .clone()
    }

    /// Get the current surface texture and its view
    pub fn current_frame(
        &self,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((surface_texture, view))
    }

    pub async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let instance = Self::instance();
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter!");

        let (device, queue) = {
            log::info!("WGPU Adapter Features: {:#?}", adapter.features());
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("WGPU Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,

                    #[cfg(not(target_arch = "wasm32"))]
                    required_features: wgpu::Features::POLYGON_MODE_LINE,
                    #[cfg(target_arch = "wasm32")]
                    required_features: wgpu::Features::default(),

                    #[cfg(not(target_arch = "wasm32"))]
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    #[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    #[cfg(all(target_arch = "wasm32", feature = "webgl"))]
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                })
                .await
                .expect("Failed to request a device!")
        };

        let surface_capabilities = surface.get_capabilities(&adapter);

        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Self {
            pipeline_reg: Mutex::new(PipelineRegistry::new()),
            surface,
            device,
            queue,
            surface_config,
            surface_format,
        }
    }
}

pub struct PipelineBuilder<'a> {
    pub label: Option<&'a str>,
    pub shader_source: &'a str,
    pub vertex_entry: &'a str,
    pub fragment_entry: &'a str,
    pub vertex_buffers: &'a [wgpu::VertexBufferLayout<'a>],
    pub bind_group_layouts: &'a [&'a wgpu::BindGroupLayout],
    pub surface_format: wgpu::TextureFormat,
    pub blend_state: Option<wgpu::BlendState>,
    pub primitive_topology: wgpu::PrimitiveTopology,
    pub cull_mode: Option<wgpu::Face>,
    pub depth_format: Option<wgpu::TextureFormat>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new(shader_source: &'a str, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            label: None,
            shader_source,
            vertex_entry: "vs_main",
            fragment_entry: "fs_main",
            vertex_buffers: &[],
            bind_group_layouts: &[],
            surface_format,
            blend_state: Some(wgpu::BlendState::REPLACE),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            depth_format: None,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn vertex_entry(mut self, entry: &'a str) -> Self {
        self.vertex_entry = entry;
        self
    }

    pub fn fragment_entry(mut self, entry: &'a str) -> Self {
        self.fragment_entry = entry;
        self
    }

    pub fn vertex_buffers(mut self, buffers: &'a [wgpu::VertexBufferLayout<'a>]) -> Self {
        self.vertex_buffers = buffers;
        self
    }

    pub fn bind_groups(mut self, layouts: &'a [&'a wgpu::BindGroupLayout]) -> Self {
        self.bind_group_layouts = layouts;
        self
    }

    pub fn blend_state(mut self, blend: Option<wgpu::BlendState>) -> Self {
        self.blend_state = blend;
        self
    }

    pub fn primitive_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.primitive_topology = topology;
        self
    }

    pub fn cull_mode(mut self, cull_mode: Option<wgpu::Face>) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    pub fn depth(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_format = Some(format);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: self.label,
            source: wgpu::ShaderSource::Wgsl(self.shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: self.label,
            bind_group_layouts: self.bind_group_layouts,
            push_constant_ranges: &[],
        });

        let depth_stencil = self.depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: self.label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(self.vertex_entry),
                buffers: self.vertex_buffers,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(self.fragment_entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.surface_format,
                    blend: self.blend_state,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: self.primitive_topology,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: self.cull_mode,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }
}
