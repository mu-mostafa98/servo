/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Unified stroke pipeline — dispatches solid / gradient strokes for both
//! rectangles (via WebRender borders) and polylines (decomposed into
//! individual line segments).
//!
//! **Single responsibility:** given a stroke style from [`RenderContext`],
//! produce the display-list commands for that stroke.  Every shape renderer
//! delegates its stroke work here.

use webrender_api::{
    BorderSide, BorderStyle, BorderDetails, NormalBorder, BorderRadius,
    ColorF,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutSideOffsets},
};

use lyon::math::Point as LyonPoint;

use crate::shapes::Line;
use crate::renderer::{Render, RenderContext, make_common_props, to_colorf, effective_stroke_width, clip_chain_option};
use crate::renderer::gradient;
use crate::style::{NodeStyle, Visibility, Display, StrokeParams};
use crate::style::gradient::PaintServer;

// ======================= Rect stroke =======================

/// Stroke an axis-aligned rectangle using the current style's stroke properties.
///
/// Handles both solid-colour borders (via [`push_border`]) and gradient
/// borders (via full-rect gradient + interior white clip).
pub(crate) fn stroke_rect(bounds: LayoutRect, radii: Option<BorderRadius>, ctx: &mut RenderContext) {
    let Some(stroke) = &ctx.style.stroke else { return };

    if let Some(svg_color) = stroke.color {
        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;
        let stroke_width = effective_stroke_width(ctx, stroke.width);
        let widths = LayoutSideOffsets::new_all_same(stroke_width);
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
        // Gradient border: render gradient across the full rect, then clip
        // the interior with white so only the border band shows the gradient.
        let stroke_width = effective_stroke_width(ctx, stroke.width);
        let inset = stroke_width;
        let inner_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bounds.min.x + inset, bounds.min.y + inset),
            LayoutSize::new(
                (bounds.size().width - inset * 2.0).max(0.0),
                (bounds.size().height - inset * 2.0).max(0.0),
            ),
        );
        gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, stroke.opacity);
        if inner_bounds.size().width > 0.0 && inner_bounds.size().height > 0.0 {
            let inner_clip_id = ctx.wr.define_clip_rect(ctx.spatial_id, inner_bounds);
            let inner_chain = ctx.wr.define_clip_chain(
                clip_chain_option(ctx.clip_chain_id),
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

// ======================= Polyline stroke =======================

/// Stroke an open or closed polyline by decomposing it into individual
/// [`Line`] segments.
///
/// `pts` are expected to be in **local shape coordinates** (not yet shifted
/// by `svg_origin`) — each segment's renderer will add `ctx.svg_origin`.
///
/// Only solid-color strokes are supported for polylines; gradient strokes
/// on polylines are not yet implemented.
pub(crate) fn stroke_polyline(pts: &[LyonPoint], ctx: &mut RenderContext) {
    let Some(stroke) = &ctx.style.stroke else { return };
    let adjusted_width = effective_stroke_width(ctx, stroke.width);
    if stroke.color.is_some() && adjusted_width > 0.0 {
        let stroke_style = NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill: None,
            render_hints: None,
            effects: None,
            opacity: ctx.style.opacity,
            stroke: Some(StrokeParams {
                color: stroke.color,
                paint_server: None,
                opacity: stroke.opacity,
                width: adjusted_width,
                line_cap: stroke.line_cap,
                line_join: stroke.line_join,
                miter_limit: stroke.miter_limit,
                dash_array: stroke.dash_array.clone(),
                dash_offset: stroke.dash_offset,
            }),
        };

        let mut line_ctx = RenderContext {
            style: &stroke_style,
            svg_origin: ctx.svg_origin,
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
            wr: &mut *ctx.wr,
            paints: ctx.paints,
            accumulated_scale: ctx.accumulated_scale,
        };

        for pair in pts.windows(2) {
            let line = Line {
                x1: pair[0].x as f32,
                y1: pair[0].y as f32,
                x2: pair[1].x as f32,
                y2: pair[1].y as f32,
            };
            line.render(&mut line_ctx);
        }
    }
}
