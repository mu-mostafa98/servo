/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<text>` element renderer.
//!
//! Uses WebRender's `push_text` for real glyph rendering when a
//! `FontInstanceKey` is available. Falls back to estimated rectangles.
//! Supports fill, stroke, and text-anchor alignment.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform};
use webrender_api::{
    ColorF, CommonItemProperties, GlyphInstance, PropertyBinding, ReferenceFrameKind, SpaceAndClipInfo,
    TransformStyle,
};

use crate::renderer::{Render, RenderContext, to_colorf};
use crate::text::TextSpan;

const FALLBACK_ADVANCE: f32 = 8.0;
const FALLBACK_HEIGHT: f32 = 16.0;
/// Approximate descent below the baseline, so descenders (p/g/y/j/q) are not
/// clipped by the text bounds rect.
const DESCENT: f32 = 4.0;

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
    /// Horizontal offset for text-anchor alignment.
    fn anchor_offset(&self) -> f32 {
        let offset = self.text_anchor.alignment_offset();
        if self.rtl {
            // Mirror the anchor: for RTL, `start` aligns to the right edge
            // (the text was pre-reversed, so it is laid out left-to-right).
            -1.0 - offset
        } else {
            offset
        }
    }

    /// Emit glyphs or fallback rectangles at the given position.
    fn emit(
        &self,
        ctx: &mut RenderContext,
        anchor_offset: f32,
        stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        if self.font_instance_key.is_some() {
            self.emit_glyphs(ctx, anchor_offset, stroke_color, fill_color);
        } else {
            self.emit_rects(ctx, anchor_offset, stroke_color, fill_color);
        }
    }

    fn emit_glyphs(
        &self,
        ctx: &mut RenderContext,
        anchor_offset: f32,
        stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        if self.glyphs.is_empty() {
            return;
        }
        let base_x = ctx.svg_origin.x + self.x + self.advance_offset + anchor_offset;
        let base_y = ctx.svg_origin.y + self.y;

        let last = self.glyphs.last().unwrap();
        let total_w = last.x + last.advance;
        // The bounds must encompass every glyph's vertical position — `dy` and
        // `dominant-baseline` shift the baseline, so clamp the rect to the
        // glyph y range (WebRender clips to this rect).
        let min_y = self.glyphs.iter().map(|g| g.y).fold(0.0f32, f32::min);
        let max_y = self.glyphs.iter().map(|g| g.y).fold(0.0f32, f32::max);
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(base_x, base_y + min_y - FALLBACK_HEIGHT),
            LayoutSize::new(
                total_w.max(1.0),
                (max_y - min_y + FALLBACK_HEIGHT + DESCENT).max(1.0),
            ),
        );

        // No per-character rotation — emit glyphs grouped by font (mixed-script
        // runs shape different characters with different fonts via fallback).
        if !self.rotate.iter().any(|a| *a != 0.0) {
            let mut i = 0;
            while i < self.glyphs.len() {
                let Some(font_key) = self.glyphs[i].font_instance_key else {
                    i += 1;
                    continue; // fallback glyph with no font — skip
                };
                let mut j = i + 1;
                while j < self.glyphs.len() && self.glyphs[j].font_instance_key == Some(font_key)
                {
                    j += 1;
                }
                let glyphs: Vec<GlyphInstance> = self.glyphs[i..j]
                    .iter()
                    .map(|g| GlyphInstance {
                        index: g.glyph_id,
                        point: LayoutPoint::new(base_x + g.x, base_y + g.y),
                    })
                    .collect();
                let common = CommonItemProperties::new(
                    bounds,
                    SpaceAndClipInfo {
                        spatial_id: ctx.spatial_id,
                        clip_chain_id: ctx.clip_chain_id,
                    },
                );

                if let Some(color) = fill_color {
                    ctx.wr
                        .push_text(&common, bounds, &glyphs, font_key, color, None);
                }
                if let Some(color) = stroke_color {
                    // Stroke via a second push_text call with different color.
                    // Full stroke rendering requires outline glyphs — this is a
                    // best-effort approximation.
                    ctx.wr
                        .push_text(&common, bounds, &glyphs, font_key, color, None);
                }
                i = j;
            }
            return;
        }

        // Per-character rotation: emit each glyph separately, wrapping rotated
        // glyphs in a reference frame so they rotate about their origin.
        // Per SVG, a shorter list applies its last value to remaining chars.
        let last_angle = self.rotate.last().copied().unwrap_or(0.0);
        for (i, g) in self.glyphs.iter().enumerate() {
            let Some(font_key) = g.font_instance_key else { continue };
            let angle = self.rotate.get(i).copied().unwrap_or(last_angle);
            if let Some(color) = fill_color {
                push_glyph(
                    ctx.wr,
                    font_key,
                    g,
                    base_x,
                    base_y,
                    angle,
                    ctx.spatial_id,
                    ctx.clip_chain_id,
                    color,
                );
            }
            if let Some(color) = stroke_color {
                push_glyph(
                    ctx.wr,
                    font_key,
                    g,
                    base_x,
                    base_y,
                    angle,
                    ctx.spatial_id,
                    ctx.clip_chain_id,
                    color,
                );
            }
        }
    }

    fn emit_rects(
        &self,
        ctx: &mut RenderContext,
        anchor_offset: f32,
        _stroke_color: Option<ColorF>,
        fill_color: Option<ColorF>,
    ) {
        let x = ctx.svg_origin.x + self.x + self.advance_offset + anchor_offset;
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
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo {
                    spatial_id: ctx.spatial_id,
                    clip_chain_id: ctx.clip_chain_id,
                },
            );

            if let Some(color) = fill_color {
                ctx.wr.push_rect(&common, bounds, color);
            }
        }
    }
}

/// Push a single glyph, optionally rotated by `angle` (degrees) about its
/// baseline origin via a reference frame.
#[allow(clippy::too_many_arguments)]
fn push_glyph(
    wr: &mut webrender_api::DisplayListBuilder,
    font_key: webrender_api::FontInstanceKey,
    g: &crate::text::ShapedGlyph,
    base_x: f32,
    base_y: f32,
    angle: f32,
    spatial_id: webrender_api::SpatialId,
    clip_chain_id: webrender_api::ClipChainId,
    color: ColorF,
) {
    let glyph_x = base_x + g.x;
    let glyph_y = base_y + g.y;

    let (glyph_spatial_id, point, bounds) = if angle != 0.0 {
        let frame_id = wr.push_reference_frame(
            LayoutPoint::new(glyph_x, glyph_y),
            spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(LayoutTransform::rotation(
                0.0,
                0.0,
                1.0,
                euclid::Angle::degrees(angle),
            )),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );
        let b = LayoutRect::from_origin_and_size(
            LayoutPoint::new(0.0, -FALLBACK_HEIGHT),
            LayoutSize::new(g.advance.max(1.0), FALLBACK_HEIGHT + DESCENT),
        );
        (frame_id, LayoutPoint::zero(), b)
    } else {
        let b = LayoutRect::from_origin_and_size(
            LayoutPoint::new(glyph_x, glyph_y - FALLBACK_HEIGHT),
            LayoutSize::new(g.advance.max(1.0), FALLBACK_HEIGHT + DESCENT),
        );
        (spatial_id, LayoutPoint::new(glyph_x, glyph_y), b)
    };

    let common = CommonItemProperties::new(
        bounds,
        SpaceAndClipInfo {
            spatial_id: glyph_spatial_id,
            clip_chain_id,
        },
    );
    let glyphs = [GlyphInstance {
        index: g.glyph_id,
        point,
    }];
    wr.push_text(&common, bounds, &glyphs, font_key, color, None);

    if angle != 0.0 {
        wr.pop_reference_frame();
    }
}
