/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<text>` element renderer.
//!
//! Uses WebRender's `push_text` for real glyph rendering when a
//! `FontInstanceKey` is available. Falls back to estimated rectangles.
//! Supports fill, stroke, and text-anchor alignment.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ColorF, CommonItemProperties, GlyphInstance, SpaceAndClipInfo};

use crate::renderer::{Render, RenderContext, to_colorf};
use crate::text::TextSpan;

const FALLBACK_ADVANCE: f32 = 8.0;
const FALLBACK_HEIGHT: f32 = 16.0;

impl Render for TextSpan {
    fn render(&self, ctx: &mut RenderContext) {
        let total_advance = self.total_advance();
        let anchor_offset = self.anchor_offset() * total_advance;

        // Render stroke first if paint-order dictates.
        if let Some(stroke) = &ctx.style.stroke {
            if let Some(svg_color) = stroke.color {
                let color = to_colorf(&svg_color);
                self.emit(ctx, anchor_offset, Some(color), None);
            }
        }

        // Render fill.
        if let Some(fill) = &ctx.style.fill {
            if let Some(svg_color) = fill.color {
                let color = to_colorf(&svg_color);
                self.emit(ctx, anchor_offset, None, Some(color));
            }
        }
    }
}

impl TextSpan {
    /// Total advance width of all glyphs (or estimated text width).
    fn total_advance(&self) -> f32 {
        if let Some(last) = self.glyphs.last() {
            last.x + last.advance
        } else {
            self.text.chars().count() as f32 * FALLBACK_ADVANCE
        }
    }

    /// Horizontal offset for text-anchor alignment.
    fn anchor_offset(&self) -> f32 {
        self.text_anchor.alignment_offset()
    }

    /// Emit glyphs or fallback rectangles at the given position.
    fn emit(
        &self,
        ctx: &mut RenderContext,
        anchor_offset: f32,
        stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        if let Some(font_key) = self.font_instance_key {
            self.emit_glyphs(ctx, font_key, anchor_offset, stroke_color, fill_color);
        } else {
            self.emit_rects(ctx, anchor_offset, stroke_color, fill_color);
        }
    }

    fn emit_glyphs(
        &self,
        ctx: &mut RenderContext,
        font_key: webrender_api::FontInstanceKey,
        anchor_offset: f32,
        stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        if self.glyphs.is_empty() { return; }
        let base_x = ctx.svg_origin.x + self.x + anchor_offset;
        let base_y = ctx.svg_origin.y + self.y;

        let glyphs: Vec<GlyphInstance> = self
            .glyphs
            .iter()
            .map(|g| GlyphInstance {
                index: g.glyph_id,
                point: LayoutPoint::new(base_x + g.x, base_y + g.y),
            })
            .collect();

        let last = self.glyphs.last().unwrap();
        let total_w = last.x + last.advance;
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(base_x, base_y - FALLBACK_HEIGHT),
            LayoutSize::new(total_w.max(1.0), FALLBACK_HEIGHT),
        );
        let common = CommonItemProperties::new(bounds, SpaceAndClipInfo {
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
        });

        if let Some(color) = fill_color {
            ctx.wr.push_text(&common, bounds, &glyphs, font_key, color, None);
        }
        if let Some(color) = stroke_color {
            // Stroke via a second push_text call with different color.
            // Full stroke rendering requires outline glyphs — this is a
            // best-effort approximation.
            ctx.wr.push_text(&common, bounds, &glyphs, font_key, color, None);
        }
    }

    fn emit_rects(
        &self,
        ctx: &mut RenderContext,
        anchor_offset: f32,
        _stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        let x = ctx.svg_origin.x + self.x + anchor_offset;
        let y = ctx.svg_origin.y + self.y;
        let gap = 1.0f32;

        for (i, _ch) in self.text.chars().enumerate() {
            let mut cx = 0.0f32;
            for j in 0..i {
                cx += self.dx.get(j).copied().unwrap_or(0.0);
            }
            let char_x = x + cx + i as f32 * (FALLBACK_ADVANCE + gap);
            let bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(char_x, y - FALLBACK_HEIGHT),
                LayoutSize::new(FALLBACK_ADVANCE, FALLBACK_HEIGHT),
            );
            let common = CommonItemProperties::new(bounds, SpaceAndClipInfo {
                spatial_id: ctx.spatial_id,
                clip_chain_id: ctx.clip_chain_id,
            });

            if let Some(color) = fill_color {
                ctx.wr.push_rect(&common, bounds, color);
            }
        }
    }
}
