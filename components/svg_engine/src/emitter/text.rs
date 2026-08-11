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
/// For each span that has a resolved `font_handle`, looks up its pre-shaped
/// glyphs in [`GlyphStore`](crate::GlyphStore) and emits a `PaintCommand::Text`
/// carrying the handle (the WebRender backend resolves it to a
/// `FontInstanceKey`). Spans without a handle fall back to flattened paths.
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

        for span in chunk.spans() {
            // Solid fill only — gradient was checked above.
            let Some(fill) = span.fill() else { continue };
            let color = match fill.paint() {
                usvg::Paint::Color(c) => c,
                _ => continue,
            };
            let opacity = fill.opacity().get();
            let paint_color = color_from_usvg(color, opacity);

            // Look up the pre-shaped glyphs by handle. If no handle was set
            // (font resolution failed at construction time), fall back to
            // flattened paths for the whole text node.
            let Some(handle) = span.font_handle() else {
                emit_group_flattened(text.flattened(), ctx, commands);
                return;
            };
            let Some(shaped) = ctx.glyphs.get(handle) else {
                emit_group_flattened(text.flattened(), ctx, commands);
                return;
            };
            if shaped.glyphs.is_empty() {
                continue;
            }

            let font_size = shaped.font_size;

            // Apply text-anchor offset.
            let anchor_offset = match anchor {
                usvg::TextAnchor::Start => 0.0,
                usvg::TextAnchor::Middle => -shaped.total_advance / 2.0,
                usvg::TextAnchor::End => -shaped.total_advance,
            };

            // Map to TextGlyph with cumulative X positions.
            let text_glyphs: Vec<TextGlyph> = shaped.glyphs.iter().map(|g| {
                TextGlyph {
                    glyph_id: g.glyph_id,
                    x: g.x + anchor_offset,
                    y: g.y,
                    advance: g.advance,
                }
            }).collect();

            commands.push(PaintCommand::Text {
                x: ctx.svg_origin.x + x + anchor_offset,
                y: ctx.svg_origin.y + y,
                glyphs: text_glyphs,
                font_handle: handle,
                font_size,
                color: paint_color,
            });
        }
    }
}
