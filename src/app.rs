use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use glam::{UVec2, Vec2};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    window::Window,
};

use crate::{
    ClearScreen, ColorTint, DbgTriangle, ShaderHandle, Vertex, VertexPosCol,
    gpu::Renderer,
    rect::Rect,
    ui,
    utils::{self, RGBA},
};

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
            mouse_pos: Vec2::ZERO,
            renderer,
            draw_list: ui::DrawList::new(),
            dbg_wireframe: false,
            // rect_render,
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
        let renderer = utils::futures::wait_for(async move {
            Renderer::new_async(window_handle_2, size.width, size.height).await
        });
        // let renderer = pollster::block_on(async move {
        //     Renderer::new_async(window_handle_2, size.width, size.height).await
        // });

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
    draw_list: ui::DrawList,
    dbg_wireframe: bool,
    renderer: Renderer,

    prev_frame_time: Instant,
    delta_time: Duration,

    mouse_pos: Vec2,

    last_size: UVec2,
    window: Arc<Window>,
}

impl App {
    pub fn window_size(&self) -> UVec2 {
        let size = self.window.inner_size();
        (size.width, size.height).into()
    }

    pub fn width(&self) -> u32 {
        self.window_size().x
    }

    pub fn height(&self) -> u32 {
        self.window_size().y
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.width() as f32 / self.height() as f32
    }

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

        self.on_update();

        match event {
            WE::CursorMoved { position: pos, .. } => {
                self.mouse_pos = (pos.x as f32, pos.y as f32).into();
            }
            WE::MouseInput { state, button, .. } => {
                use winit::event::{ElementState, MouseButton};
                let state = match state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                };

                match button {
                    // MouseButton::Left => self.ui.mouse.set_button_press(ui::MouseButton::Left, state),
                    // MouseButton::Right => self.ui.mouse.set_button_press(ui::MouseButton::Right, state),
                    // MouseButton::Middle => self.ui.mouse.set_button_press(ui::MouseButton::Middle, state),
                    _ => (),
                }
            }
            WE::RedrawRequested => {
                self.on_redraw(event_loop);
            }
            WE::KeyboardInput { event, .. } => {
                self.on_keyboard(&event, event_loop);
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

    fn on_update(&mut self) {
        self.draw_list.screen_size = self.window_size().as_vec2();
        self.draw_list.begin_frame();

        self.draw_list.add_rect(
            Rect::from_min_max((100.0, 100.0).into(), (500.0, 500.0).into())
                .draw()
                .fill(RGBA::INDIGO)
                .corner_radius(30.0),
        );


        self.draw_list.add_rect(
            Rect::from_min_max((300.0, 300.0).into(), (600.0, 800.0).into())
                .draw()
                .outline(RGBA::PURPLE, 30.0)
                .fill(RGBA::MAGENTA)
                .corner_radius(80.0),
        );

        self.draw_list
            .path_arc_around(Vec2::new(500.0, 700.0), 200.0, 0.0, 3.141592653 / 2.0);
        self.draw_list.path_to((200.0, 500.0).into());
        self.draw_list.build_path_stroke(100.0, RGBA::FOLLY);

        if self.dbg_wireframe {
            self.draw_list.debug_wireframe(3.0);
        }
    }

    fn on_keyboard(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Escape) => {
                event_loop.exit();
            }
            PhysicalKey::Code(KeyCode::KeyD) => {
                if event.state.is_pressed() {
                    self.dbg_wireframe = !self.dbg_wireframe;
                }
            }
            PhysicalKey::Code(KeyCode::KeyR) => {
                let shader = ColorTint(RGBA::rand());
                shader.try_rebuild(&[(&VertexPosCol::desc(), "Vertex")], &self.renderer.wgpu);
            }
            _ => (),
        }
    }

    fn on_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let prev_time = self.prev_frame_time;
        let curr_time = Instant::now();
        let dt = curr_time - prev_time;
        self.prev_frame_time = curr_time;
        self.delta_time = dt;

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

        {
            let mut surface = self.renderer.surface_target();
            surface.render(&ClearScreen("#242933".into()));
            surface.render(&self.draw_list);
        }

        self.renderer.present_frame();
        self.window.request_redraw();
    }

    fn resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }
}
