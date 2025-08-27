use std::{
    fmt, ops,
    time::{Duration, Instant},
};

use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct PerButton<T>(pub [T; 3]);

impl<T> ops::Index<MouseButton> for PerButton<T> {
    type Output = T;

    fn index(&self, index: MouseButton) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl<T> ops::IndexMut<MouseButton> for PerButton<T> {
    fn index_mut(&mut self, index: MouseButton) -> &mut Self::Output {
        &mut self.0[index as usize]
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
struct ButtonState {
    pressed: bool,
    double_press: bool,
    press_start_pos: Vec2,
    press_time: Option<Instant>,
    last_release_time: Option<Instant>,
    last_press_was_short: bool,
    dragging: bool,
}

impl fmt::Display for ButtonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.dragging {
            "dragging"
        } else if self.pressed {
            "pressed"
        } else {
            "released"
        };

        write!(f, "{}", status)?;

        if self.pressed {
            write!(
                f,
                " @({:.1}, {:.1})",
                self.press_start_pos.x, self.press_start_pos.y
            )?;
            if let Some(press_time) = self.press_time {
                write!(f, " {}ms", press_time.elapsed().as_millis())?;
            }
        }

        if self.double_press {
            write!(f, " [DOUBLE]")?;
        }

        Ok(())
    }
}

impl fmt::Debug for ButtonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MouseState {
    pub mouse_pos: Vec2,
    prev_pos: Vec2,
    buttons: PerButton<ButtonState>,

    // Configuration
    double_click_time: Duration,
    drag_threshold: f32,
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            prev_pos: Vec2::ZERO,
            buttons: PerButton::default(),
            double_click_time: Duration::from_millis(150),
            drag_threshold: 5.0,
        }
    }

    pub fn set_mouse_pos(&mut self, x: f32, y: f32) {
        self.prev_pos = self.mouse_pos;
        self.mouse_pos = Vec2::new(x, y);

        // Update drag states for all pressed buttons
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            let state = &mut self.buttons[button];
            if state.pressed && !state.dragging {
                let distance = self.mouse_pos.distance(state.press_start_pos);
                if distance > self.drag_threshold {
                    state.dragging = true;
                }
            }
        }
    }

    pub fn set_button_press(&mut self, button: MouseButton, pressed: bool) {
        let state = &mut self.buttons[button];
        let was_pressed = state.pressed;

        if pressed && !was_pressed {
            let now = Instant::now();
            state.pressed = true;
            state.press_start_pos = self.mouse_pos;
            state.press_time = Some(now);
            state.dragging = false;

            state.double_press = if let Some(last_release) = state.last_release_time {
                now.duration_since(last_release) <= self.double_click_time
                    && state.last_press_was_short
            } else {
                false
            };
        } else if !pressed && was_pressed {
            let now = Instant::now();
            state.pressed = false;
            state.dragging = false;

            if let Some(press_time) = state.press_time {
                let press_duration = now.duration_since(press_time);
                state.last_press_was_short = press_duration <= self.double_click_time;
            } else {
                state.last_press_was_short = false;
            }

            state.last_release_time = Some(now);
            state.press_time = None;
            state.double_press = false;
        }
    }

    pub fn drag_delta(&self) -> Vec2 {
        self.mouse_pos - self.prev_pos
    }

    pub fn pressed(&self, button: MouseButton) -> bool {
        self.buttons[button].pressed
    }

    pub fn dragging(&self, button: MouseButton) -> bool {
        self.buttons[button].dragging
    }

    pub fn drag_start(&self, button: MouseButton) -> Vec2 {
        self.buttons[button].press_start_pos
    }

    pub fn just_pressed(&self, button: MouseButton) -> bool {
        let state = &self.buttons[button];
        state.pressed
            && state
                .press_time
                .map_or(false, |t| t.elapsed() < Duration::from_millis(16))
    }

    pub fn double_clicked(&self, button: MouseButton) -> bool {
        self.buttons[button].double_press
    }
}

impl fmt::Display for MouseState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MouseState {{")?;
        writeln!(
            f,
            "  pos: ({:.1}, {:.1})",
            self.mouse_pos.x, self.mouse_pos.y
        )?;

        let delta = self.drag_delta();
        if delta.x != 0.0 || delta.y != 0.0 {
            writeln!(f, "  delta: ({:.1}, {:.1})", delta.x, delta.y)?;
        }

        writeln!(f, "  left: {}", self.buttons[MouseButton::Left])?;
        writeln!(f, "  right: {}", self.buttons[MouseButton::Right])?;
        writeln!(f, "  middle: {}", self.buttons[MouseButton::Middle])?;
        write!(f, "}}")
    }
}
