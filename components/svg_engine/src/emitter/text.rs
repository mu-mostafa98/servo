/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Text emitter — renders usvg::Text nodes.
//!
//! Simple text (solid fill, no textPath, no per-glyph transforms) uses
//! the backend's native text/glyph API for GPU-accelerated rendering.
//! Complex text falls back to the usvg flattened-path approach.

use super::{Emit, EmitContext, PaintColor, PaintCommand, TextGlyph, color_from_usvg};

impl Emit for usvg::Text {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        // Check if this text is suitable for native glyph rendering.
        // Complex features (textPath, dx/dy, rotate) require path rendering.
        if has_complex_features(self) {
            // Complex text: use flattened paths (already in the tree as Path nodes).
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Check all spans for gradient fills/strokes — native text only does solid color.
        if has_gradient_paint(self) {
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Simple text: emit native glyph commands.
        // We need a font database for shaping. If none is available, fall back.
        let Some(fontdb) = ctx.fontdb.as_ref() else {
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        };

        emit_simple_text(self, ctx, fontdb, commands);
    }
}

/// Check if the text element uses features that require path rendering.
fn has_complex_features(text: &usvg::Text) -> bool {
    // Per-glyph transforms
    if !text.dx().is_empty() || !text.dy().is_empty() || !text.rotate().is_empty() {
        return true;
    }
    // textPath
    for chunk in text.chunks() {
        if !matches!(chunk.text_flow(), usvg::TextFlow::Linear) {
            return true;
        }
    }
    false
}

/// Check if any span has a gradient fill or stroke.
fn has_gradient_paint(text: &usvg::Text) -> bool {
    for chunk in text.chunks() {
        for span in chunk.spans() {
            if let Some(fill) = span.fill() {
                if matches!(fill.paint(), usvg::Paint::LinearGradient(_) | usvg::Paint::RadialGradient(_)) {
                    return true;
                }
            }
            if let Some(stroke) = span.stroke() {
                if matches!(stroke.paint(), usvg::Paint::LinearGradient(_) | usvg::Paint::RadialGradient(_)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Emit flattened paths from a Group (recursively). Used as fallback for complex text.
fn emit_group_flattened(
    group: &usvg::Group,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    for child in group.children() {
        match child {
            usvg::Node::Group(g) => emit_group_flattened(g, ctx, commands),
            usvg::Node::Path(path) => path.emit(ctx, commands),
            _ => {}
        }
    }
}

/// Shape and emit simple text using native glyph rendering.
fn emit_simple_text(
    text: &usvg::Text,
    ctx: &EmitContext,
    fontdb: &fontdb::Database,
    commands: &mut Vec<PaintCommand>,
) {
    for chunk in text.chunks() {
        let x = chunk.x().unwrap_or(0.0);
        let y = chunk.y().unwrap_or(0.0);
        let anchor = chunk.anchor();
        let full_text = chunk.text();

        if full_text.is_empty() {
            continue;
        }

        for span in chunk.spans() {
            let span_text = &full_text[span.start()..span.end()];
            if span_text.is_empty() {
                continue;
            }

            // Get fill color (solid only — gradient was checked above).
            let fill = match span.fill() {
                Some(f) => f,
                None => continue,
            };
            let color = match fill.paint() {
                usvg::Paint::Color(c) => c,
                _ => continue,
            };
            let opacity = fill.opacity().get();
            let paint_color = color_from_usvg(color, opacity);

            // Shape glyphs using fontdb.
            let font_size = span.font_size().get();
            let Some((glyphs, total_advance)) =
                shape_text(fontdb, span.font(), font_size, span_text)
            else {
                continue;
            };

            if glyphs.is_empty() {
                continue;
            }

            // Apply text-anchor offset.
            let anchor_offset = match anchor {
                usvg::TextAnchor::Start => 0.0,
                usvg::TextAnchor::Middle => -total_advance / 2.0,
                usvg::TextAnchor::End => -total_advance,
            };

            // Map to TextGlyph with cumulative X positions.
            let text_glyphs: Vec<TextGlyph> = glyphs.into_iter().map(|g| {
                TextGlyph {
                    glyph_id: g.glyph_id,
                    x: g.x + anchor_offset,
                    y: g.y,
                    advance: g.advance,
                }
            }).collect();

            // Find font index in the database (for backend lookups).
            let font_index = ctx.font_indices.as_ref()
                .and_then(|m| m.get(&span.font()))
                .copied()
                .unwrap_or(0);

            commands.push(PaintCommand::Text {
                x: ctx.svg_origin.x + x + anchor_offset,
                y: ctx.svg_origin.y + y,
                glyphs: text_glyphs,
                font_index,
                font_size,
                color: paint_color,
            });
        }
    }
}

/// A shaped glyph with position data.
struct ShapedGlyph {
    glyph_id: u32,
    x: f32,
    y: f32,
    advance: f32,
}

/// Shape a string of text using fontdb + rustybuzz.
fn shape_text(
    db: &fontdb::Database,
    font: &usvg::Font,
    font_size: f32,
    text: &str,
) -> Option<(Vec<ShapedGlyph>, f32)> {
    let family = font.families().first()?;
    let family_str = match family {
        usvg::FontFamily::Named(s) => s.as_str(),
        usvg::FontFamily::Serif => "serif",
        usvg::FontFamily::SansSerif => "sans-serif",
        usvg::FontFamily::Cursive => "cursive",
        usvg::FontFamily::Fantasy => "fantasy",
        usvg::FontFamily::Monospace => "monospace",
    };
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family_str)],
        weight: fontdb::Weight(font.weight()),
        style: match font.style() {
            usvg::FontStyle::Normal => fontdb::Style::Normal,
            usvg::FontStyle::Italic => fontdb::Style::Italic,
            usvg::FontStyle::Oblique => fontdb::Style::Oblique,
        },
        stretch: match font.stretch() {
            usvg::FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
            usvg::FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
            usvg::FontStretch::Condensed => fontdb::Stretch::Condensed,
            usvg::FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
            usvg::FontStretch::Normal => fontdb::Stretch::Normal,
            usvg::FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
            usvg::FontStretch::Expanded => fontdb::Stretch::Expanded,
            usvg::FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
            usvg::FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
        },
    };
    let font_id = db.query(&query)?;
    // Load font data and parse the face.
    let face_info = db.face(font_id)?;
    let font_data: Vec<u8> = match &face_info.source {
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec(),
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        _ => return None,
    };
    let ttf_face = ttf_parser::Face::parse(&font_data, face_info.index).ok()?;
    let rb_face = rustybuzz::Face::from_face(ttf_face);

    let units_per_em = rb_face.units_per_em() as f32;
    let scale = font_size / units_per_em;

    // Shape using rustybuzz.
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let glyph_buffer = rustybuzz::shape(&rb_face, &[], buffer);

    let glyph_infos = glyph_buffer.glyph_infos();
    let glyph_positions = glyph_buffer.glyph_positions();

    let mut glyphs = Vec::with_capacity(glyph_infos.len());
    let mut x_cursor = 0.0f32;
    let mut total_advance = 0.0f32;

    for (i, info) in glyph_infos.iter().enumerate() {
        let pos = glyph_positions.get(i);
        let x_advance = pos.map(|p| p.x_advance as f32 * scale).unwrap_or(0.0);
        let y_offset = pos.map(|p| p.y_offset as f32 * scale).unwrap_or(0.0);
        let x_offset = pos.map(|p| p.x_offset as f32 * scale).unwrap_or(0.0);

        glyphs.push(ShapedGlyph {
            glyph_id: info.glyph_id,
            x: x_cursor + x_offset,
            y: y_offset,
            advance: x_advance,
        });

        x_cursor += x_advance;
        total_advance += x_advance;
    }

    Some((glyphs, total_advance))
}
