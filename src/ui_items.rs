use glam::Vec2;

use crate::{
    core::RGBA,
    mouse::{CursorIcon, MouseBtn},
    rect::Rect,
    ui::{self, CornerRadii, Id},
};

macro_rules! ui_text {
    ($ui:ident: $($tt:tt)*) => {
        $ui.text(&format!($($tt)*));
    }
}
pub(crate) use ui_text;

impl ui::Context {
    pub fn image(&mut self, size: Vec2, uv_min: Vec2, uv_max: Vec2, tex_id: u32) {
        let id = self.gen_id(tex_id);
        let rect = self.place_item(id, size);
        self.register_item(id);
        self.draw(|list| {
            list.rect(rect.min, rect.max)
                .texture_uv(uv_min, uv_max, tex_id)
                .add()
        })
    }

    pub fn button(&mut self, label: &str) -> bool {
        let id = self.gen_id(label);
        let active = self.style.btn_press();
        let hover = self.style.btn_hover();
        let default = self.style.btn_default();

        let total_h = self.style.line_height();
        let text_shape = self.shape_text(label, self.style.text_size());
        let text_dim = text_shape.size();

        let vert_pad = ((total_h - text_dim.y) / 2.0).max(0.0);
        let horiz_pad = vert_pad;
        let size = Vec2::new(text_dim.x + horiz_pad * 2.0, total_h);

        let rect = self.place_item(id, size);
        let sig = self.register_item(id);

        let start_drag_outside = self
            .mouse
            .drag_start(MouseBtn::Left)
            .map_or(false, |pos| !rect.contains(pos));

        let (btn_col, text_col) = if sig.pressed() && !start_drag_outside {
            (active, self.style.btn_press_text())
        } else if sig.hovering() {
            (hover, self.style.text_col())
        } else {
            (default, self.style.text_col())
        };

        let text_pos =
            rect.min + Vec2::new((size.x - text_dim.x) * 0.5, (size.y - text_dim.y) * 0.5);

        self.draw(|list| {
            list.rect(rect.min, rect.max)
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(btn_col)
                .add();
            list.add_text(text_pos, &text_shape, text_col);
        });

        sig.released() && !start_drag_outside
    }

    pub fn switch(&mut self, label: &str, b: &mut bool) -> bool {
        let height = self.style.line_height();
        let width = height * 1.8;
        let size = Vec2::new(width, self.style.line_height());
        let text_shape = self.shape_text(label, self.style.text_size());
        let text_dim = text_shape.size();

        let id = self.gen_id(label);
        let rect = self.place_item(id, size);
        let sig = self.register_item(id);

        if sig.released() {
            *b = !*b;
        }

        let mut bg_col = if sig.hovering() {
            self.style.btn_hover()
        } else {
            self.style.btn_default()
        };
        let mut knob_col = self.style.btn_press();

        if *b {
            std::mem::swap(&mut bg_col, &mut knob_col);
        }

        self.draw(|list| {
            let rail_min = rect.min;
            let rail_max = rail_min + Vec2::new(width, height);
            list.rect(rect.min, rect.max)
                // .corners(CornerRadii::all(height * 0.5))
                // .corners(CornerRadii::all(height * 0.3))
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(bg_col)
                .add();

            let knob_r = height * 0.8 * 0.5;
            let knob_x = if *b {
                rail_max.x - height * 0.5
            } else {
                rail_min.x + height * 0.5
            };
            let knob_center = Vec2::new(knob_x, rail_min.y + height * 0.5);
            list.circle(knob_center, knob_r)
                // .corners(CornerRadii::all(height * 0.8 * 0.3))
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(knob_col)
                .add();
        });

        self.same_line();
        self.text(label);

        *b
    }

    pub fn checkbox(&mut self, label: &str, b: &mut bool) -> bool {
        let id = self.gen_id(label);
        let active = self.style.btn_press();
        let hover = self.style.btn_hover();
        let default = self.style.btn_default();

        let box_size = self.style.line_height();
        let text_shape = self.shape_text(label, self.style.text_size());

        let rect = self.place_item(id, Vec2::splat(box_size));
        let sig = self.register_item(id);

        if sig.released() {
            *b = !*b;
        }

        let col = if sig.pressed() {
            active
        } else if sig.hovering() {
            hover
        } else {
            default
        };

        let radii = CornerRadii::all(self.style.btn_corner_radius());
        self.draw(|list| {
            let inset = box_size * 0.15;
            let inner_min = rect.min + Vec2::splat(inset);
            let inner_max = rect.max - Vec2::splat(inset);

            list.rect(rect.min, rect.max).fill(col).corners(radii).add();
            if *b {
                list.rect(inner_min, inner_max)
                    .corners(radii)
                    .fill(active)
                    .add();
            }
        });

        self.same_line();
        self.text(label);

        *b
    }

    pub fn separator_h(&mut self, thickness: f32, fill: RGBA) {
        let width = self.available_content().x;
        let rect = self.place_item(Id::NULL, Vec2::new(width, thickness));
        let col = self.style.panel_dark_bg();

        self.draw(|list| list.rect(rect.min, rect.max).fill(fill).add());
    }

    pub fn slider_f32(&mut self, label: &str, min: f32, max: f32, val: &mut f32) {
        let height = self.style.line_height();
        let width = self.available_content().x / 2.5;
        let rect = self.place_item(self.gen_id(label), Vec2::new(width, height));
        let sig = self.register_item(self.gen_id(label));

        let knob_size = height * 0.8;
        let rail_pad = height - knob_size;
        let usable_width = (rect.width() - knob_size - rail_pad).max(0.0);

        if sig.pressed() || sig.dragging() {
            let denom = usable_width.max(1.0);
            let t = ((self.mouse.pos.x - (rect.min.x + knob_size)) / denom).clamp(0.0, 1.0);
            if (max - min).abs() > f32::EPSILON {
                *val = min + t * (max - min);
            }
        }

        let ratio = if (max - min).abs() < f32::EPSILON {
            0.0
        } else {
            ((*val - min) / (max - min)).clamp(0.0, 1.0)
        };

        let mut knob_min = rect.min + Vec2::splat(rail_pad / 2.0);
        knob_min.x += ratio * usable_width;
        let knob_max = knob_min + Vec2::splat(knob_size);

        if sig.hovering() {
            self.set_cursor_icon(CursorIcon::MoveH);
        }
        if sig.pressed() && !sig.dragging() {
            self.expect_drag = true;
        }

        let (mut rail_col, mut knob_col) = if sig.dragging() {
            (self.style.btn_press(), self.style.btn_hover())
        } else if sig.hovering() {
            (self.style.btn_hover(), self.style.btn_press())
        } else {
            (self.style.btn_default(), self.style.btn_press())
        };

        self.draw(|list| {
            list.rect(rect.min, rect.max)
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(rail_col)
                .add();

            list.rect(knob_min, knob_max)
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(knob_col)
                .add()
        });

        self.same_line();
        self.text(label);
    }

    pub fn collapsing_header(&mut self, label: &str, open: &mut bool) -> bool {
        let id = self.gen_id(label);
        let active = self.style.btn_press();
        let hover = self.style.btn_hover();
        let default = self.style.btn_default();

        let total_h = self.style.line_height();

        let text_shape = self.shape_text(label, self.style.text_size());
        let text_dim = text_shape.size();

        let icon = if *open {
            ui::PhosphorFont::CARET_DOWN
        } else {
            ui::PhosphorFont::CARET_RIGHT
        };
        let icon_shape = self.shape_icon(icon, self.style.text_size());
        let icon_dim = text_shape.size();

        let vert_pad = ((total_h - text_dim.y) / 2.0).max(0.0);
        let avail = self.available_content();
        let size = Vec2::new(avail.x, total_h);

        let rect = self.place_item(id, size);
        let sig = self.register_item(id);

        let start_drag_outside = self
            .mouse
            .drag_start(MouseBtn::Left)
            .map_or(false, |pos| !rect.contains(pos));

        if sig.released() {
            *open = !*open;
        }

        let (btn_col, text_col) = if *open || sig.pressed() && !start_drag_outside {
            (active, self.style.btn_press_text())
        } else if sig.hovering() {
            (hover, self.style.text_col())
        } else {
            (default, self.style.text_col())
        };

        let icon_pos = rect.min + Vec2::new(vert_pad, (size.y - icon_dim.y) * 0.5);

        let text_pos = icon_pos + Vec2::new(self.style.text_size() * 2.0, 0.0);

        self.draw(|list| {
            list.rect(rect.min, rect.max)
                .corners(CornerRadii::all(self.style.btn_corner_radius()))
                .fill(btn_col)
                .add();

            list.add_text(icon_pos, &icon_shape, text_col);
            list.add_text(text_pos, &text_shape, text_col);
        });

        *open
    }

    pub fn text(&mut self, text: &str) {
        let text_height = self.style.text_size();
        let line_height = self.style.line_height().max(text_height);

        let pad = (line_height - text_height) / 2.0;
        self.move_down(pad);
        let shape = self.shape_text(text, self.style.text_size());

        let p = self.get_current_panel();
        let id = p.gen_id(text);

        let size = Vec2::new(shape.width, shape.height);
        let rect = self.place_item(id, size);
        // self.register_item(id);
        self.move_down(pad);

        self.draw(|list| list.add_text(rect.min, &shape, self.style.text_col()));
    }

    pub fn begin_tabbar(&mut self, label: &str) {
        // TODO[NOTE] tabbar stack
        let id = self.gen_id(label);
        self.tabbars.map.entry(id).or_insert(ui::TabBar::new());
        self.current_tabbar_id = id;
        self.push_id(id);

        let avail = self.available_content();

        self.push_style(ui::StyleVar::SpacingV(0.0));
        let rect = self.place_item(id, Vec2::new(avail.x, self.style.line_height()));
        self.pop_style();
        self.separator_h(3.0, self.style.btn_hover());

        let cursor = self.get_current_panel()._cursor.clone().into_inner();

        let tb = &mut self.tabbars[id];
        tb.id = id;
        tb.panel_id = self.current_panel_id;
        tb.cursor_backup = cursor;
        tb.bar_rect = rect;

        tb.layout_tabs();
    }

    pub fn end_tabbar(&mut self) {
        let tb = &self.tabbars[self.current_tabbar_id];
        // let cursor = tb.cursor_backup;
        let tb_id = tb.id;
        assert!(self.pop_id() == tb_id);

        self.current_tabbar_id = Id::NULL;
        // self.get_current_panel()._cursor.replace(cursor);
    }

    pub fn tabitem(&mut self, label: &str) -> bool {
        let tb_id = self.current_tabbar_id;
        let tb_rect = self.tabbars[tb_id].bar_rect;
        assert!(!tb_id.is_null());

        let id = self.gen_id(label);
        let tb = &mut self.tabbars[tb_id];
        if tb.tabs.is_empty() {
            tb.selected_tab_id = id;
        }

        let text_shape = self.shape_text(label, self.style.text_size());
        let text_dim = text_shape.size();
        let vert_pad = ((tb_rect.height() - text_dim.y) / 2.0).max(0.0);
        let item_width = vert_pad * 2.0 + text_dim.x;

        let tb = &mut self.tabbars[tb_id];
        let is_selected = tb.selected_tab_id == id;

        let indx = tb.tabs.iter().position(|t| t.id == id);
        let Some(indx) = indx else {
            let mut item = ui::TabItem::default();
            item.id = id;
            item.width = item_width;
            tb.tabs.push(item);
            return is_selected;
        };

        tb.tabs[indx].width = item_width;
        let item = tb.tabs[indx];

        let tab_size = Vec2::new(item.width, tb_rect.height());
        let rect = Rect::from_min_size(tb_rect.min + Vec2::new(item.offset, 0.0), tab_size);
        let sig = self.register_rect(id, rect);

        let (btn_col, text_col) = if is_selected {
            (self.style.btn_hover(), self.style.text_col())
        } else if sig.hovering() {
            (self.style.btn_default(), self.style.text_col())
        } else {
            (self.style.panel_bg(), self.style.text_col())
        };

        let tb = &mut self.tabbars[tb_id];

        if sig.pressed() {
            tb.selected_tab_id = id;
        }
        if sig.dragging() && self.active_id == id && !tb.is_dragging {
            tb.is_dragging = true;
            tb.selected_tab_id = id;
            tb.dragging_offset = rect.min.x - self.mouse.pos.x;
        }

        if is_selected && !self.mouse.pressed(MouseBtn::Left) && tb.is_dragging {
            tb.is_dragging = false;
        }

        let mut item_pos = rect.min;

        if tb.is_dragging && tb.selected_tab_id == id {
            item_pos.x = tb.dragging_offset + self.mouse.pos.x;
        }

        if is_selected {
            let new_indx = tb.get_insert_pos(item_pos.x, rect.width(), indx);
            tb.move_tab(indx, new_indx);
        }

        item_pos.x = item_pos
            .x
            .max(tb_rect.min.x)
            .min(tb_rect.max.x - rect.width());

        let text_pos = item_pos
            + Vec2::new(
                (item.width - text_dim.x) * 0.5,
                (tb_rect.height() - text_dim.y) * 0.5,
            );

        if tb.is_dragging && tb.selected_tab_id == id {
            self.draw_over(|list| {
                list.rect(item_pos, item_pos + rect.size())
                    .fill(btn_col)
                    .corners(CornerRadii::top(self.style.btn_corner_radius()))
                    .add();

                list.add_text(text_pos, &text_shape, text_col);
            });
        } else {
            self.draw(|list| {
                list.rect(item_pos, item_pos + rect.size())
                    .fill(btn_col)
                    .corners(CornerRadii::top(self.style.btn_corner_radius() * 1.5))
                    .add();

                list.add_text(text_pos, &text_shape, text_col);
            });
        }

        is_selected
    }
}

// BEGIN INTERN
//---------------------------------------------------------------------------------------

impl ui::Context {
    pub fn checkbox_intern(&mut self, label: &str) -> bool {
        let id = self.gen_id(label);
        let mut toggle = *self.widget_data.get_or_insert(id, false);
        self.checkbox(label, &mut toggle);
        self.widget_data.insert(id, toggle);
        toggle
    }

    pub fn switch_intern(&mut self, label: &str) -> bool {
        let id = self.gen_id(label);
        let mut toggle = *self.widget_data.get_or_insert(id, false);
        self.switch(label, &mut toggle);
        self.widget_data.insert(id, toggle);
        toggle
    }

    pub fn slider_f32_intern(&mut self, label: &str, min: f32, max: f32) -> f32 {
        let id = self.gen_id(label);
        let mut val = *self.widget_data.get_or_insert(id, (min + max) / 2.0);
        self.slider_f32(label, min, max, &mut val);
        self.widget_data.insert(id, val);
        val
    }

    pub fn collapsing_header_intern(&mut self, label: &str) -> bool {
        let id = self.gen_id(label);
        let mut b = *self.widget_data.get_or_insert(id, false);
        self.collapsing_header(label, &mut b);
        self.widget_data.insert(id, b);
        b
    }
}
