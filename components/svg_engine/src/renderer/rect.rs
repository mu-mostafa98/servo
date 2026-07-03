/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    BorderSide, BorderStyle, BorderDetails, NormalBorder, BorderRadius,
    ClipChainId, ClipMode, ComplexClipRegion, ColorF,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutSideOffsets},
};

use crate::shapes::Rectangle;
use crate::renderer::{Render, RenderContext, make_common_props};
use crate::renderer::gradient;
use crate::style::gradient::PaintServer;

impl Render for Rectangle {
    fn render(&self, ctx: &mut RenderContext) {
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(ctx.svg_origin.x + self.x, ctx.svg_origin.y + self.y),
            LayoutSize::new(self.width, self.height),
        );

        let rx = self.rx.or(self.ry).unwrap_or(0.0).clamp(0.0, self.width / 2.0);
        let ry = self.ry.or(self.rx).unwrap_or(0.0).clamp(0.0, self.height / 2.0);
        let has_radius = rx > 0.0 || ry > 0.0;
        let radii = has_radius.then(|| BorderRadius {
            top_left: LayoutSize::new(rx, ry),
            top_right: LayoutSize::new(rx, ry),
            bottom_left: LayoutSize::new(rx, ry),
            bottom_right: LayoutSize::new(rx, ry),
        });

        // Compute clip for rounded rect (shared by fill and gradient).
        let clip = if let Some(r) = radii {
            let clip_id = ctx.wr.define_clip_rounded_rect(
                ctx.spatial_id,
                ComplexClipRegion { rect: bounds, radii: r, mode: ClipMode::Clip },
            );
            let parent = match ctx.clip_chain_id {
                ClipChainId::INVALID => None,
                id => Some(id),
            };
            ctx.wr.define_clip_chain(parent, [clip_id])
        } else {
            ctx.clip_chain_id
        };

        // ── FILL ──────────────────────────────────────────────────────
        if let Some(fill) = &ctx.style.fill {
            match &fill.paint_server {
                Some(PaintServer::Gradient(id)) => {
                    // Apply rounded-rect clip to gradient bands too.
                    let old_clip = ctx.clip_chain_id;
                    ctx.clip_chain_id = clip;
                    gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, fill.opacity);
                    ctx.clip_chain_id = old_clip;
                },
                Some(_) => {
                    // Pattern paint servers not yet implemented.
                },
                None => {
                    if let Some(mut color) = fill.color {
                        color.a *= fill.opacity;
                        let common = make_common_props(bounds, ctx.spatial_id, clip);
                        ctx.wr.push_rect(&common, bounds, color);
                    }
                },
            }
        }

        // ── STROKE ────────────────────────────────────────────────────
        if let Some(stroke) = &ctx.style.stroke {
            if let Some(mut color) = stroke.color {
                color.a *= stroke.opacity;
                let widths = LayoutSideOffsets::new_all_same(stroke.width);
                let details = BorderDetails::Normal(NormalBorder {
                    left: BorderSide { color, style: BorderStyle::Solid },
                    right: BorderSide { color, style: BorderStyle::Solid },
                    top: BorderSide { color, style: BorderStyle::Solid },
                    bottom: BorderSide { color, style: BorderStyle::Solid },
                    radius: radii.unwrap_or(BorderRadius {
                        top_left: LayoutSize::zero(), top_right: LayoutSize::zero(),
                        bottom_left: LayoutSize::zero(), bottom_right: LayoutSize::zero(),
                    }),
                    do_aa: true,
                });
                let common = make_common_props(bounds, ctx.spatial_id, ctx.clip_chain_id);
                ctx.wr.push_border(&common, bounds, widths, details);
            } else if let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
                // Gradient border: render gradient across full bounds, then
                // clip the interior with white to leave gradient only on the border.
                let inset = stroke.width;
                let inner_bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(bounds.min.x + inset, bounds.min.y + inset),
                    LayoutSize::new(
                        (bounds.size().width - inset * 2.0).max(0.0),
                        (bounds.size().height - inset * 2.0).max(0.0),
                    ),
                );
                // Draw gradient across the full rect.
                gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, stroke.opacity);
                // Cover the center with white via an interior clip.
                if inner_bounds.size().width > 0.0 && inner_bounds.size().height > 0.0 {
                    let inner_clip_id = ctx.wr.define_clip_rect(ctx.spatial_id, inner_bounds);
                    let inner_chain = ctx.wr.define_clip_chain(
                        if ctx.clip_chain_id == ClipChainId::INVALID { None } else { Some(ctx.clip_chain_id) },
                        [inner_clip_id],
                    );
                    let old_clip = ctx.clip_chain_id;
                    ctx.clip_chain_id = inner_chain;
                    let white = ColorF::new(1.0, 1.0, 1.0, 1.0);
                    let common = make_common_props(bounds, ctx.spatial_id, inner_chain);
                    ctx.wr.push_rect(&common, bounds, white);
                    ctx.clip_chain_id = old_clip;
                }
            }
        }
    }
}
