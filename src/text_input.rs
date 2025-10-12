use cosmic_text::Edit;
use glam::Vec2;

use crate::{ctext, gpu::WGPU, ui};

#[derive(Debug, Clone)]
pub struct TextInputState {
    pub edit: ctext::Editor<'static>,
}

impl TextInputState {
    pub fn new(fonts: &mut ui::FontTable, text: ui::TextItem) -> Self {
        let mut buffer = ctext::Buffer::new(
            &mut fonts.sys,
            ctext::Metrics {
                font_size: text.font_size(),
                line_height: text.scaled_line_height(),
            },
        );

        let font_attrib = fonts.get_font_attrib(text.font);
        buffer.set_text(
            &mut fonts.sys,
            &text.string,
            &font_attrib,
            ctext::Shaping::Advanced,
        );

        let edit = ctext::Editor::new(buffer);

        Self { edit }
    }

    pub fn shape(
        &self,
        fonts: &mut ui::FontTable,
        cache: &mut ui::GlyphCache,
        wgpu: &WGPU,
    ) -> ui::ShapedText {
        let buffer = match self.edit.buffer_ref() {
            cosmic_text::BufferRef::Owned(b) => b,
            _ => panic!(),
        };

        let mut glyphs = Vec::new();
        let mut width = 0.0;
        let mut height = 0.0;

        for run in buffer.layout_runs() {
            width = run.line_w.max(width);
            // TODO[CHECK]: is it the sum?
            // height = run.line_height.max(height);
            height += run.line_height;

            for g in run.glyphs {
                let g_phys = g.physical((0.0, 0.0), 1.0);
                let mut key = g_phys.cache_key;
                // TODO[CHECK]: what does this do
                key.x_bin = ctext::SubpixelBin::Three;
                key.y_bin = ctext::SubpixelBin::Three;

                if let Some(mut glyph) = cache.get_glyph(key, fonts, wgpu) {
                    glyph.meta.pos += Vec2::new(g_phys.x as f32, g_phys.y as f32 + run.line_y);
                    glyphs.push(glyph);
                }
            }
        }

        let text = ui::ShapedText {
            glyphs,
            width,
            height,
        };
        text
    }
}
