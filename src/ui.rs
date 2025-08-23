use glam::{Mat4, UVec2, UVec4, Vec2, Vec4};
use macros::vertex;
use rustc_hash::FxHashMap;
use wgpu::util::DeviceExt;

use std::{
    collections::VecDeque, hash::{Hash, Hasher}, ops
};

use crate::{
    RGBA, RenderPassHandle, ShaderGenerics, ShaderHandle, Vertex, VertexPosCol,
    gpu::{self, VertexDesc, WGPU},
    rect::Rect,
};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub const NULL: NodeId = NodeId(0);

    pub fn from_str(s: &str) -> Self {
        let mut hasher = rustc_hash::FxHasher::default();
        s.hash(&mut hasher);
        Self(hasher.finish().max(1))
    }

    pub fn is_null(&self) -> bool {
        *self == Self::NULL
    }
}

impl Default for NodeId {
    fn default() -> Self {
        NodeId::NULL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    X = 0,
    Y = 1,
}

impl Axis {
    pub fn flip(&self) -> Self {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizingTyp {
    Null,
    Fit,
    Grow,
    Fixed(f32),
}

pub struct Size {
    typ: SizingTyp,
    strictness: f32,
}

impl Size {
    const NULL: Size = Size {
        typ: SizingTyp::Null,
        strictness: 0.0,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerAxis<T>(pub [T; 2]);

impl<T> ops::Index<Axis> for PerAxis<T> {
    type Output = T;

    fn index(&self, index: Axis) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl<T> ops::IndexMut<Axis> for PerAxis<T> {
    fn index_mut(&mut self, index: Axis) -> &mut Self::Output {
        &mut self.0[index as usize]
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Padding {
    left: f32, right: f32, top: f32, bottom: f32,
}

impl Padding {
    const ZERO: Padding = Padding::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left, right, top, bottom
        }
    }

    pub fn sum_along_axis(&self, a: Axis) -> f32 {
        match a {
            Axis::X => self.left + self.right,
            Axis::Y => self.top + self.bottom,
        }
    }

    pub fn axis_padding(&self, a: Axis) -> [f32; 2] {
        match a {
            Axis::X => [self.left, self.right],
            Axis::Y => [self.top, self.bottom],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,

    pub first: NodeId,
    pub last: NodeId,
    pub next: NodeId,
    pub prev: NodeId,
    pub parent: NodeId,
    pub n_children: u32,

    pub corner_radius: f32,
    pub background_color: RGBA,
    pub layout_direction: Axis,
    pub padding: Padding,
    pub child_gap: f32,

    pub sizing_typ: PerAxis<SizingTyp>,

    pub fixed_pos: Option<Vec2>,
    // comp_rel_pos: Vec2,

    pub size: Vec2,
    pub pos: Vec2,
    // fixed_pos: Vec2,
    // fixed_size: Vec2,
    // min_size: Vec2,

    // pref_size: PerAxis<Size>,

    pub rect: Rect,

    pub last_frame_used: u64,
}


impl Node {
    const NULL: Node = Node {
        id: NodeId::NULL,
        first: NodeId::NULL,
        last: NodeId::NULL,
        next: NodeId::NULL,
        prev: NodeId::NULL,
        parent: NodeId::NULL,
        n_children: 0,

        corner_radius: 0.0,
        background_color: RGBA::ZERO,
        layout_direction: Axis::X,
        padding: Padding::ZERO,
        child_gap: 0.0,

        fixed_pos: Some(Vec2::ZERO),
        // comp_rel_pos: Vec2::ZERO,
        sizing_typ: PerAxis([SizingTyp::Null;2]),

        size: Vec2::NAN,
        pos: Vec2::NAN,

        // fixed_pos: Vec2::ZERO,
        // fixed_size: Vec2::ZERO,
        // min_size: Vec2::ZERO,
        // pref_size: PerAxis([Size::NULL; 2]),
        rect: Rect::ZERO,
        last_frame_used: 0,
    };

    pub fn background_color(mut self, col: impl Into<RGBA>) -> Self {
        self.background_color = col.into();
        self
    }

    pub fn position(mut self, pos: impl Into<Vec2>) -> Self {
        self.fixed_pos = Some(pos.into());
        self
    }

    pub fn fixed_size_x(mut self, size: f32) -> Self {
        self.sizing_typ[Axis::X] = SizingTyp::Fixed(size);
        self
    }

    pub fn fixed_size_y(mut self, size: f32) -> Self {
        self.sizing_typ[Axis::Y] = SizingTyp::Fixed(size);
        self
    }

    pub fn fixed_size(self, size: impl Into<Vec2>) -> Self {
        let size: Vec2 = size.into();
        self.fixed_size_x(size.x).fixed_size_y(size.y)
    }

    pub fn grow_x(mut self) -> Self {
        self.sizing_typ[Axis::X] = SizingTyp::Grow;
        self
    }

    pub fn grow_y(mut self) -> Self {
        self.sizing_typ[Axis::Y] = SizingTyp::Grow;
        self
    }

    pub fn grow(self) -> Self {
        self.grow_x().grow_y()
    }

    pub fn child_gap(mut self, gap: f32) -> Self {
        self.child_gap = gap;
        self
    }

    pub fn layout_dir(mut self, axis: Axis) -> Self {
        self.layout_direction = axis;
        self
    }

    pub fn as_rect_inst(&self) -> RectInst {
        RectInst {
            min: self.pos,
            max: self.pos + self.size,
            color: self.background_color,
        }
    }
}


bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NodeFlags: u32 {
        const DRAW_BORDER       = 1 << 0;
        const DRAW_BACKGROUND   = 1 << 1;
        const DRAGGABLE         = 1 << 2;
    }
}

macro_rules! sig_bits {
    ($n:literal) => { 1 << $n };
    ($i:ident) => { SignalFlags::$i.bits() };
    ($($x:tt)|+) => {
        $(sig_bits!($x) | )* 0
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SignalFlags: u32 {
        const PRESSED_L = 1 << 0;
        const PRESSED_M = 1 << 1;
        const PRESSED_R = 1 << 2;

        const DRAGGING_L = 1 << 3;
        const DRAGGING_M = 1 << 4;
        const DRAGGING_R = 1 << 5;

        const DOUBLE_DRAGGING_L = 1 << 6;
        const DOUBLE_DRAGGING_M = 1 << 7;
        const DOUBLE_DRAGGING_R = 1 << 8;

        const RELEASED_L = 1 << 9;
        const RELEASED_M = 1 << 10;
        const RELEASED_R = 1 << 11;

        const CLICKED_L = 1 << 12;
        const CLICKED_M = 1 << 13;
        const CLICKED_R = 1 << 14;

        const DOUBLE_CLICKED_L = 1 << 15;
        const DOUBLE_CLICKED_M = 1 << 16;
        const DOUBLE_CLICKED_R = 1 << 17;

        const HOVERING = 1 << 18;
        const MOUSE_OVER = 1 << 19; // may be occluded

        const PRESSED_KEYBOARD = 1 << 20;

        // const PRESS = sig_bit!(PRESS_L | PRESS_KEYBOARD);
        // const RELEASE = sig_bit!(RELEASE_L);
        // const CLICK = sig_bit!(CLICK_L | PRESS_KEYBOARD);
        // const DOUBLE_CLICK = sig_bit!(DOUBLE_CLICK_L);
        // const DRAG = sig_bit!(DRAG_L);
    }
}

macro_rules! sig_fn {
    ($fn_name:ident => $($x:tt)*) => {
        impl SignalFlags {
            pub const fn $fn_name(&self) -> bool {
                let flag = SignalFlags::from_bits(sig_bits!($($x)*)).unwrap();
                self.contains(flag)
            }
        }
    }
}

sig_fn!(pressed => PRESSED_L | PRESSED_KEYBOARD);
sig_fn!(clicked => CLICKED_L | PRESSED_KEYBOARD);
sig_fn!(double_clicked => DOUBLE_CLICKED_L);
sig_fn!(dragging => DRAGGING_L);
sig_fn!(released => RELEASED_L);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MouseState {
    pub left: bool,
    pub middle: bool,
    pub right: bool,
}

impl ops::Index<MouseButton> for MouseState {
    type Output = bool;

    fn index(&self, index: MouseButton) -> &Self::Output {
        match index {
            MouseButton::Left => &self.left,
            MouseButton::Right => &self.right,
            MouseButton::Middle => &self.middle,
        }
    }
}

impl ops::IndexMut<MouseButton> for MouseState {
    fn index_mut(&mut self, index: MouseButton) -> &mut Self::Output {
        match index {
            MouseButton::Left => &mut self.left,
            MouseButton::Right => &mut self.right,
            MouseButton::Middle => &mut self.middle,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    pub mouse_pos: Vec2,
    pub mouse_pressed: MouseState,
    pub mouse_drag_start: Option<Vec2>,

    pub root: NodeId,

    pub hot_node: NodeId,
    pub active_node: NodeId,

    pub node_stack: Vec<Node>,
    pub cached_nodes: FxHashMap<NodeId, Node>,
    pub roots: Vec<NodeId>,

    pub frame_index: u64,

    pub id_stack: Vec<NodeId>,
}

impl State {

    pub fn update_mouse_pos(&mut self, x: f32, y: f32) {
        self.mouse_pos = Vec2::new(x, y);
    }

    pub fn update_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        self.mouse_pressed[button] = pressed;
    }

    pub fn node_id_from_str(&self, str: &str) -> NodeId {
        use std::hash::{Hash, Hasher};
        if let Some(id) = self.id_stack.last() {
            let mut hasher = rustc_hash::FxHasher::with_seed(id.0 as usize);
            str.hash(&mut hasher);
            NodeId(hasher.finish())
        } else {
            NodeId::from_str(str)
        }
    }

    pub fn node_fit_sizing(&mut self, mut n: Node) -> Node {
        match n.sizing_typ[Axis::X] {
            SizingTyp::Null => (),
            SizingTyp::Fit => (),
            SizingTyp::Grow => (),
            SizingTyp::Fixed(x) => n.size.x = x,
        }
        match n.sizing_typ[Axis::Y] {
            SizingTyp::Null => (),
            SizingTyp::Fit => (),
            SizingTyp::Grow => (),
            SizingTyp::Fixed(y) => n.size.y = y,
        }

        if let Some(p) = self.parent_node_mut() {
            n.size.x += n.padding.left + n.padding.right;
            n.size.y += n.padding.top + n.padding.bottom;
            match p.layout_direction {
                Axis::X => {
                    p.size.x += (p.n_children - 1) as f32 * p.child_gap;
                    p.size.x += n.size.x;
                    p.size.y = p.size.y.max(n.size.y);
                },
                Axis::Y => {
                    p.size.y += (p.n_children - 1) as f32 * p.child_gap;
                    p.size.x = p.size.x.max(n.size.x);
                    p.size.y += n.size.y;
                },
            }
        }
        n
    }

    pub fn cached_node(&self, id: NodeId) -> &Node {
        if id.is_null() {
            panic!("NULL id");
        }

        self.cached_nodes.get(&id).unwrap()
    }

    pub fn cached_node_mut(&mut self, id: NodeId) -> &mut Node {
        if id.is_null() {
            panic!("NULL id");
        }

        self.cached_nodes.get_mut(&id).unwrap()
    }

    pub fn node_grow_elements_along_axis(&mut self, p: &Node, a: Axis) {
        let mut growable = Vec::new();

        let mut child_id = p.first;
        while !child_id.is_null() {
            let n = self.cached_node_mut(child_id);
            if n.sizing_typ[a] == SizingTyp::Grow {
                growable.push(child_id);
            }
            child_id = n.next;
        }

        if growable.is_empty() {
            return
        }

        let a_idx = a as usize;
        let mut remaining = p.size[a_idx];
        remaining -= p.padding.sum_along_axis(a);

        for id in &growable {
            let n = self.cached_node(*id);
            remaining -= n.size[a_idx];
        }

        remaining -= (p.n_children - 1) as f32 * p.child_gap;

        while remaining > 0.0 && !growable.is_empty() {
            let mut smallest = self.cached_node(growable[0]).size[a_idx];
            let mut second_smallest = f32::INFINITY;
            let mut to_add = remaining;

            for id in &growable {
                let n = self.cached_node(*id);
                if n.size[a_idx] < smallest {
                    second_smallest = smallest;
                    smallest = n.size[a_idx];
                }
                if n.size[a_idx] > smallest {
                    second_smallest = second_smallest.min(n.size[a_idx]);
                    to_add = second_smallest - smallest;
                }
            }

            to_add = to_add.min(remaining / growable.len() as f32);

            for id in &growable {
                let n = self.cached_node_mut(*id);
                if n.size[a_idx] == smallest {
                    n.size[a_idx] += to_add;
                    remaining -= to_add;
                }
            }
        }
    }

    pub fn node_compute_element_positions(&mut self, p: &Node) {

        // let mut x_off = p.padding.left + p.pos;
        // let mut y_off = p.padding.top + p.pos;
        let mut pos = Vec2::new(p.padding.left, p.padding.top) + p.pos;

        let mut child_id = p.first;
        while !child_id.is_null() {
            let n = self.cached_node_mut(child_id);
            child_id = n.next;

            n.pos = pos;

            match p.layout_direction {
                Axis::X => pos.x += n.size.x + p.child_gap,
                Axis::Y => pos.y += n.size.y + p.child_gap,
            }
        }
    }

    pub fn pop_parent(&mut self) {
        self.id_stack.pop();
        if let Some(n) = self.node_stack.pop() {
            let mut n = self.node_fit_sizing(n);

            if n.parent.is_null() {
                self.node_grow_elements_along_axis(&n, Axis::X);
                self.node_grow_elements_along_axis(&n, Axis::Y);

                if let Some(p) = n.fixed_pos {
                    n.pos = p;
                }
                self.node_compute_element_positions(&n);
                self.roots.push(n.id);
            }

            self.cached_nodes.insert(n.id, n);
        }
    }

    pub fn push_parent(&mut self, p: Node) {
        self.id_stack.push(p.id);
        self.node_stack.push(p);
    }

    pub fn parent_node(&self) -> Option<&Node> {
        self.node_stack.last()
    }

    pub fn parent_node_mut(&mut self) -> Option<&mut Node> {
        self.node_stack.last_mut()
    }

    pub fn prune_nodes(&mut self) {
        self.cached_nodes.retain(|_, n| {
            n.last_frame_used == self.frame_index
        })
    }

    pub fn take_or_init_node(&mut self, id: NodeId) -> (Node, bool) {
        if let Some(n) = self.cached_nodes.remove(&id) {
            (n, false)
        } else {
            let mut n = Node::NULL;
            n.id = id;
            (n, true)
        }
    }

    pub fn build_node_from_id(&mut self, id: NodeId) -> Node {
        let (mut n, _is_new) = self.take_or_init_node(id);

        n.next = NodeId::NULL;
        n.prev = NodeId::NULL;

        n.n_children = 0;
        n.first = NodeId::NULL;
        n.last = NodeId::NULL;
        n.parent = NodeId::NULL;

        n.size = Vec2::ZERO;
        n.pos = Vec2::ZERO;

        n.last_frame_used = self.frame_index;

        if let Some(p) = self.parent_node_mut() {
            if p.first.is_null() {
                // first child
                p.first = n.id;
                p.last = n.id;
            } else if !p.last.is_null() {
                n.prev = p.last;
            }

            p.last = n.id;
            n.parent = p.id;
            p.n_children += 1;
        }

        if !n.prev.is_null() {
            self.cached_nodes.get_mut(&n.prev).unwrap().next = n.id;
        }

        n
    }

    pub fn build_node_from_str(&mut self, str: &str) -> Node {
        let id = self.node_id_from_str(str);
        self.build_node_from_id(id)
    }

    pub fn begin_node(&mut self, s: &str, f: impl FnOnce(Node) -> Node) {
        let n = self.build_node_from_str(s);
        self.push_parent(f(n));
    }

    pub fn end_node(&mut self) {
        self.pop_parent();
    }

    pub fn finish(&mut self) -> Vec<RectInst> {
        if !self.node_stack.is_empty() {
            log::error!("node stack not empty at the end of the frame");
            panic!();
        }

        let mut rects = vec![];

        let mut nodes: VecDeque<_> = self.roots.drain(..).collect();


        while let Some(id) = nodes.pop_back() {
            let n = self.cached_node(id);
            rects.push(n.as_rect_inst());

            let mut child_id = n.first;
            while !child_id.is_null() {
                nodes.push_front(child_id);
                let c = self.cached_node(id);
                child_id = c.next;
            }
        }
        // for &id in &self.roots {
        // }

        self.roots.clear();

        rects
    }
}


#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GlobalUniform {
    pub proj: Mat4,
}

#[vertex]
pub struct Vertex2D {
    pub pos: Vec2,
}

#[vertex]
pub struct RectInst {
    pub min: Vec2,
    pub max: Vec2,
    pub color: RGBA,
}

pub struct RectRender {
    pub global_data: GlobalUniform,

    pub unit_rectangle: wgpu::Buffer,
    pub global_uniform: wgpu::Buffer,

    pub rect_buffer: wgpu::Buffer,
    pub n_instances: u32,
}

impl RectRender {
    pub fn update_window_size(&mut self, width: u32, height: u32) {
        let aspect = width as f32 / height.max(1) as f32;
        self.global_data.proj =
            Mat4::orthographic_lh(0.0, width as f32, 0.0, height as f32, -1.0, 1.0);
    }

    pub fn update_rect_instances(&mut self, rect_instances: &[RectInst], wgpu: &WGPU) {
        if rect_instances.len() < 1024 {
            // self.rect_buffer = wgpu
            //     .device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            //         label: Some("test"),
            //         contents: bytemuck::cast_slice(rect_instances),
            //         usage: wgpu::BufferUsages::VERTEX,
            //     });
            wgpu.queue.write_buffer(&self.rect_buffer, 0, bytemuck::cast_slice(rect_instances));
            self.n_instances = rect_instances.len() as u32;
        }
    }

    pub fn new(wgpu: &WGPU) -> Self {
        // let vertices = [
        //     RectInst {
        //         min: Vec2::new(0.0, 0.0),
        //         max: Vec2::new(200.0, 200.0),
        //         color: RGBA::RED,
        //     },
        //     RectInst {
        //         min: Vec2::new(250.0, 200.0),
        //         max: Vec2::new(200.0, 400.0),
        //         color: RGBA::BLUE,
        //     },
        // ];
        let rect_buffer = wgpu
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_instances"),
                size: 1024 * std::mem::size_of::<RectInst>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        // let rect_buffer = wgpu
        //     .device
        //     .create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //         label: Some("debug_rect_instance_buffer"),
        //         contents: bytemuck::cast_slice(&vertices),
        //         usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        //     });



        let vertices = [
            Vertex2D {
                pos: Vec2::new(0.0, 0.0),
            },
            Vertex2D {
                pos: Vec2::new(1.0, 0.0),
            },
            Vertex2D {
                pos: Vec2::new(0.0, 1.0),
            },
            Vertex2D {
                pos: Vec2::new(1.0, 0.0),
            },
            Vertex2D {
                pos: Vec2::new(1.0, 1.0),
            },
            Vertex2D {
                pos: Vec2::new(0.0, 1.0),
            },
        ];

        let unit_rectangle = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("debug_unit_rect_vertex_buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let global_data = GlobalUniform {
            proj: Mat4::IDENTITY,
        };

        let global_uniform = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_global_uniform_buffer"),
                contents: bytemuck::cast_slice(&[global_data]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            rect_buffer,
            n_instances: 2,
            unit_rectangle,
            global_uniform,
            global_data,
        }
    }


    pub fn build_global_bind_group(&self, wgpu: &WGPU) -> wgpu::BindGroup {
        let global_uniform = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_global_uniform_buffer"),
                contents: bytemuck::cast_slice(&[self.global_data]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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

impl RenderPassHandle for RectRender {
    fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, wgpu: &WGPU) {
        rpass.set_vertex_buffer(0, self.unit_rectangle.slice(..));
        rpass.set_vertex_buffer(1, self.rect_buffer.slice(..));

        let bind_group = self.build_global_bind_group(wgpu);
        rpass.set_bind_group(0, &bind_group, &[]);

        let shader = RectShader;
        rpass.set_pipeline(&shader.get_pipeline(
            &[
                (&Vertex2D::desc(), "Vertex"),
                (&RectInst::instance_desc(), "RectInst"),
            ],
            wgpu,
        ));

        rpass.draw(0..6, 0..self.n_instances);
    }
}

pub struct RectShader;

impl ShaderHandle for RectShader {
    const RENDER_PIPELINE_ID: crate::ShaderID = "rect_shader";

    fn build_pipeline(&self, desc: &ShaderGenerics<'_>, wgpu: &WGPU) -> wgpu::RenderPipeline {
        const SHADER_SRC: &str = r#"


            @rust struct Vertex {
                pos: vec2<f32>,
            }

            @rust struct RectInst {
                min: vec2<f32>,
                max: vec2<f32>,
                color: vec4<f32>,
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
                    r: RectInst,
                ) -> VSOut {
                    var out: VSOut;

                    let size = r.max - r.min;

                    out.color = r.color;
                    out.pos = global.proj * vec4<f32>(
                        v.pos.x * size.x + r.min.x,
                        v.pos.y * size.y + r.min.y,
                        0.0,
                        1.0
                    );

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
            .build(&wgpu.device)
    }
}
