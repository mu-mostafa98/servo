/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Text emitter — renders usvg::Text nodes.
//!
//! Simple text (solid fill, no textPath, no per-glyph transforms, and a
//! resolved `font_handle` on the span) uses the backend's native glyph API
//! for GPU-accelerated rendering via WebRender's `push_text`. The pre-shaped
//! glyphs and `FontInstanceKey` are produced by the integration layer using
//! Servo's `FontContext` (correct per-codepoint font fallback) and stored in
//! [`crate::GlyphStore`] / [`crate::FontKeyRegistry`] keyed by that handle.
//!
//! Complex text — or text whose font could not be resolved — falls back to
//! the usvg flattened-path approach (CPU rasterization via vello_cpu).

use super::{Emit, EmitContext, PaintCommand, TextGlyph, color_from_usvg};

impl Emit for usvg::Text {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        // Check if this text is suitable for native glyph rendering.
        // Complex features (textPath, dx/dy, rotate) require path rendering.
        if has_complex_features(self) {
            // Complex text: use flattened paths (already in the tree).
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Check all spans for gradient fills/strokes — native text only does solid color.
        if has_gradient_paint(self) {
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Check for stroke on any span. WebRender's push_text only renders
        // glyphs in a single solid color — it cannot produce outline strokes.
        // Stroked text must go through the path fallback for correct rendering.
        if has_stroke(self) {
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Check for text-decoration. push_text can't draw underlines/overlines
        // or line-through — route to the path fallback.
        if has_decoration(self) {
            emit_group_flattened(self.flattened(), ctx, commands);
            return;
        }

        // Simple text: emit native glyph commands using pre-shaped glyphs.
        emit_simple_text(self, ctx, commands);
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

/// Check if any span has a stroke. WebRender's `push_text` doesn't support
/// outline glyphs, so stroked text must go through the path fallback.
fn has_stroke(text: &usvg::Text) -> bool {
    for chunk in text.chunks() {
        for span in chunk.spans() {
            if span.stroke().is_some() {
                return true;
            }
        }
    }
    false
}

/// Check if any span has text decoration.
fn has_decoration(text: &usvg::Text) -> bool {
    for chunk in text.chunks() {
        for span in chunk.spans() {
            let d = span.decoration();
            if d.underline().is_some() || d.overline().is_some() || d.line_through().is_some() {
                return true;
            }
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

/// Emit flattened paths from a Group (recursively). Used as fallback for
/// complex or unresolvable text.
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

/// Emit simple text using native glyph rendering.
///
/// Each span that has a resolved `font_handle` and pre-shaped glyphs in
/// [`GlyphStore`](crate::GlyphStore) produces a `PaintCommand::Text`. Spans
/// without a handle fall back to flattened paths for the whole text node.
///
/// Multiple spans in a chunk (e.g. `<tspan>` elements) are laid out
/// sequentially: each span's glyphs are positioned after the previous spans'
/// cumulative advance. Text-anchor alignment uses the combined advance of all
/// spans so the entire text is centered or right-aligned as a unit.
fn emit_simple_text(
    text: &usvg::Text,
    ctx: &EmitContext,
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

        // Collect spans — empty chunks were handled above.
        let spans = chunk.spans();
        if spans.is_empty() {
            continue;
        }

        // First pass: validate every span has a font_handle + shaped glyphs,
        // and compute the total advance across all spans for text-anchor
        // alignment. If any span is unresolvable, fall back to flattened paths.
        let mut total_advance: f32 = 0.0;
        for span in spans {
            let Some(handle) = span.font_handle() else {
                emit_group_flattened(text.flattened(), ctx, commands);
                return;
            };
            let Some(shaped) = ctx.glyphs.get(handle) else {
                emit_group_flattened(text.flattened(), ctx, commands);
                return;
            };
            total_advance += shaped.total_advance;
        }

        let anchor_offset = match anchor {
            usvg::TextAnchor::Start => 0.0,
            usvg::TextAnchor::Middle => -total_advance / 2.0,
            usvg::TextAnchor::End => -total_advance,
        };

        // Second pass: emit each span at its accumulated X position.
        let mut x_cursor: f32 = 0.0;
        for span in spans {
            // Handles already validated in the first pass.
            let handle = span.font_handle().unwrap();
            let shaped = ctx.glyphs.get(handle).unwrap();

            if shaped.glyphs.is_empty() {
                continue;
            }

            // Solid fill only — gradient was checked before we reach here.
            let paint_color = match span.fill() {
                Some(fill) => match fill.paint() {
                    usvg::Paint::Color(c) => {
                        color_from_usvg(&c, fill.opacity().get(), ctx.group_opacity)
                    }
                    _ => {
                        // Non-color fill on this span — advance cursor and skip.
                        x_cursor += shaped.total_advance;
                        continue;
                    }
                },
                None => {
                    // No fill — advance cursor past invisible span.
                    x_cursor += shaped.total_advance;
                    continue;
                }
            };

            // Map glyph-local X positions (already relative to span start).
            let text_glyphs: Vec<TextGlyph> = shaped.glyphs.iter().map(|g| {
                TextGlyph {
                    glyph_id: g.glyph_id,
                    x: g.x,
                    y: g.y,
                    advance: g.advance,
                }
            }).collect();

            commands.push(PaintCommand::Text {
                x: ctx.svg_origin.x + x + anchor_offset + x_cursor,
                y: ctx.svg_origin.y + y,
                glyphs: text_glyphs,
                font_handle: handle,
                font_size: shaped.font_size,
                color: paint_color,
            });

            x_cursor += shaped.total_advance;
        }
    }
}
