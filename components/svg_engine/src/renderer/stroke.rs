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

use euclid::Angle;
use webrender_api::{
    BorderSide, BorderStyle, BorderDetails, NormalBorder, BorderRadius,
    ColorF, PropertyBinding, ReferenceFrameKind, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutSideOffsets, LayoutTransform},
};

use lyon::math::Point as LyonPoint;

use crate::renderer::{RenderContext, make_common_props, to_colorf, effective_stroke_width, clip_chain_option, ZERO_LENGTH_EPSILON};
use crate::renderer::gradient;
use crate::style::{NodeStyle, Visibility, Display, StrokeParams};
use crate::style::gradient::{GradientDef, GradientUnits, PaintServer};

// ======================= Shared line-segment stroke =======================

/// Render a single line segment as a rotated rect filled with the stroke color
/// or gradient. Handles both solid-color and gradient paint servers.
///
/// Coordinates are in absolute layout space (svg_origin already added).
pub(crate) fn stroke_line_segment(
    x1: f32, y1: f32, x2: f32, y2: f32,
    ctx: &mut RenderContext,
) {
    let Some(stroke) = &ctx.style.stroke else { return };
    let stroke_width = effective_stroke_width(ctx, stroke.width);
    if stroke_width <= 0.0 { return; }

    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ZERO_LENGTH_EPSILON { return; }

    let mx = (x1 + x2) / 2.0;
    let my = (y1 + y2) / 2.0;
    let angle = dy.atan2(dx);
    let half_w = stroke_width / 2.0;

    // Rotated reference frame aligned with the line direction.
    // The line becomes a horizontal rect centered at the origin of this frame.
    let transform = LayoutTransform::rotation(0.0, 0.0, 1.0, Angle::radians(angle));
    let line_spatial_id = ctx.wr.push_reference_frame(
        LayoutPoint::new(mx, my),
        ctx.spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(transform),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    );

    let line_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(-len / 2.0, -half_w),
        LayoutSize::new(len, stroke_width),
    );

    if let Some(svg_color) = stroke.color {
        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;
        let common = make_common_props(line_bounds, line_spatial_id, ctx.clip_chain_id);
        ctx.wr.push_rect(&common, line_bounds, color);
    } else if let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
        // Gradient stroke: fill the rotated rect with the gradient.
        // NOTE: For userSpaceOnUse gradients the coordinates are in the parent
        // (unrotated) frame, so the gradient bands appear rotated with the line.
        // This is correct for objectBoundingBox (default) mode.
        let mut grad_ctx = RenderContext {
            style: ctx.style,
            svg_origin: LayoutPoint::new(0.0, 0.0),
            spatial_id: line_spatial_id,
            clip_chain_id: ctx.clip_chain_id,
            wr: &mut *ctx.wr,
            paints: ctx.paints,
            accumulated_scale: ctx.accumulated_scale,
        };
        gradient::fill_rect_with_gradient_by_id(
            id, line_bounds, &mut grad_ctx,
            stroke.opacity * ctx.style.opacity,
        );
    }

    ctx.wr.pop_reference_frame();
}

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
/// line segments.
///
/// `pts` are expected to be in **local shape coordinates** (not yet shifted
/// by `svg_origin`) — the absolute coordinates are computed here and passed
/// to [`stroke_line_segment`].
///
/// Handles both solid-color and gradient paint servers. For gradient strokes,
/// evaluates the gradient at each segment's midpoint so the gradient spans
/// the **entire shape** as one continuous unit, not per-segment independently.
pub(crate) fn stroke_polyline(pts: &[LyonPoint], ctx: &mut RenderContext) {
    let Some(stroke) = &ctx.style.stroke else { return };
    let adjusted_width = effective_stroke_width(ctx, stroke.width);
    if (!stroke.color.is_some() && stroke.paint_server.is_none()) || adjusted_width <= 0.0 {
        return;
    }

    // Gradient stroke: evaluate at each segment's midpoint so the gradient
    // spans the whole shape, not each segment independently.
    if stroke.color.is_none() && let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
        return stroke_polyline_gradient(pts, ctx, adjusted_width, id);
    }

    // Solid color stroke — original per-segment approach.
    let stroke_style = NodeStyle {
        visibility: Visibility::Visible,
        display: Display::Inline,
        transform: Vec::new(),
        fill: None,
        render_hints: None,
        effects: None,
        opacity: ctx.style.opacity,
        stroke: Some(StrokeParams {
            color: stroke.color.clone(),
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
        stroke_line_segment(
            line_ctx.svg_origin.x + pair[0].x as f32,
            line_ctx.svg_origin.y + pair[0].y as f32,
            line_ctx.svg_origin.x + pair[1].x as f32,
            line_ctx.svg_origin.y + pair[1].y as f32,
            &mut line_ctx,
        );
    }
}

/// Minimum subdivision size for gradient strokes along polylines.
/// Smaller values give smoother gradient transitions at the cost
/// of more WebRender draw calls.  4px matches the fill scanline
/// rasterizer cell size.
const STROKE_GRADIENT_SUBDIVISION_PX: f32 = 4.0;

/// Gradient stroke for polylines: subdivides each line segment into
/// small pieces (~4px) and evaluates the gradient at each piece's
/// midpoint in **absolute coordinates**, so the gradient varies
/// smoothly along the entire polyline rather than being constant
/// per full segment.
fn stroke_polyline_gradient(
    pts: &[LyonPoint],
    ctx: &mut RenderContext,
    adjusted_width: f32,
    grad_id: &str,
) {
    let Some(stroke) = &ctx.style.stroke else { return };
    let Some(grad_def) = ctx.paints.gradient(grad_id) else {
        log::warn!("SVG gradient \"{}\" not found for stroke", grad_id);
        return;
    };

    // Compute overall bounding box of the polyline.
    let mut min_x = f32::MAX; let mut min_y = f32::MAX;
    let mut max_x = f32::MIN; let mut max_y = f32::MIN;
    for p in pts {
        if p.x < min_x { min_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.x > max_x { max_x = p.x; }
        if p.y > max_y { max_y = p.y; }
    }
    let bbox_w = (max_x - min_x).max(1.0);
    let bbox_h = (max_y - min_y).max(1.0);

    // Convert gradient coordinates to absolute space using overall bbox.
    let (gx1, gy1, gx2, gy2) = match grad_def {
        GradientDef::Linear(lg) => match lg.units {
            GradientUnits::ObjectBoundingBox => (
                ctx.svg_origin.x + min_x + lg.x1.to_object_bbox() * bbox_w,
                ctx.svg_origin.y + min_y + lg.y1.to_object_bbox() * bbox_h,
                ctx.svg_origin.x + min_x + lg.x2.to_object_bbox() * bbox_w,
                ctx.svg_origin.y + min_y + lg.y2.to_object_bbox() * bbox_h,
            ),
            GradientUnits::UserSpaceOnUse => (
                ctx.svg_origin.x + lg.x1.to_user_space(bbox_w),
                ctx.svg_origin.y + lg.y1.to_user_space(bbox_h),
                ctx.svg_origin.x + lg.x2.to_user_space(bbox_w),
                ctx.svg_origin.y + lg.y2.to_user_space(bbox_h),
            ),
        },
        GradientDef::Radial(rg) => match rg.units {
            GradientUnits::ObjectBoundingBox => (
                ctx.svg_origin.x + min_x + rg.cx.to_object_bbox() * bbox_w,
                ctx.svg_origin.y + min_y + rg.cy.to_object_bbox() * bbox_h,
                0.0, 0.0, // unused for radial
            ),
            GradientUnits::UserSpaceOnUse => (
                ctx.svg_origin.x + rg.cx.to_user_space(bbox_w),
                ctx.svg_origin.y + rg.cy.to_user_space(bbox_h),
                0.0, 0.0, // unused for radial
            ),
        },
    };

    // Pre-compute radial focal point and radius in absolute space for later use.
    let rad_fx_fy_r2 = match grad_def {
        GradientDef::Radial(rg) => {
            let (fx, fy, r2) = match rg.units {
                GradientUnits::ObjectBoundingBox => {
                    let scale = bbox_w.max(bbox_h);
                    (ctx.svg_origin.x + min_x + rg.fx.to_object_bbox() * bbox_w,
                     ctx.svg_origin.y + min_y + rg.fy.to_object_bbox() * bbox_h,
                     (rg.r.to_object_bbox() * scale).max(1.0))
                },
                GradientUnits::UserSpaceOnUse => {
                    let scale = bbox_w.max(bbox_h);
                    (ctx.svg_origin.x + rg.fx.to_user_space(bbox_w),
                     ctx.svg_origin.y + rg.fy.to_user_space(bbox_h),
                     rg.r.to_user_space(scale).max(1.0))
                },
            };
            Some((fx, fy, r2))
        },
        GradientDef::Linear(_) => None,
    };

    let opacity = stroke.opacity * ctx.style.opacity;
    // Clamp subdivision size so extremely short segments still split at least once.
    let subdiv = STROKE_GRADIENT_SUBDIVISION_PX.max(adjusted_width * 0.25);

    for pair in pts.windows(2) {
        let ax = ctx.svg_origin.x + pair[0].x as f32;
        let ay = ctx.svg_origin.y + pair[0].y as f32;
        let bx = ctx.svg_origin.x + pair[1].x as f32;
        let by = ctx.svg_origin.y + pair[1].y as f32;

        let seg_dx = bx - ax;
        let seg_dy = by - ay;
        let seg_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();
        if seg_len < ZERO_LENGTH_EPSILON { continue; }

        let num_pieces = (seg_len / subdiv).ceil() as u32;
        let num_pieces = num_pieces.max(1);

        for i in 0..num_pieces {
            let t0 = (i as f32) / (num_pieces as f32);
            let t1 = ((i + 1) as f32) / (num_pieces as f32);
            let p0x = ax + seg_dx * t0;
            let p0y = ay + seg_dy * t0;
            let p1x = ax + seg_dx * t1;
            let p1y = ay + seg_dy * t1;
            let mx = (p0x + p1x) / 2.0;
            let my = (p0y + p1y) / 2.0;

            // Evaluate gradient at sub-segment midpoint (absolute coordinates).
            let piece_color = match grad_def {
                GradientDef::Linear(lg) => {
                    let t = gradient::gradient_projection(mx, my, gx1, gy1, gx2, gy2);
                    let mut c = gradient::color_at_t(&lg.stops, t);
                    c.a *= opacity;
                    c
                },
                GradientDef::Radial(rg) => {
                    let (fx, fy, r2) = rad_fx_fy_r2.unwrap();
                    let dx = mx - fx;
                    let dy = my - fy;
                    let dist_sq = (dx * dx + dy * dy) / (r2 * r2).max(1.0);
                    let t = dist_sq.sqrt().min(1.0);
                    let mut c = gradient::color_at_t(&rg.stops, t);
                    c.a *= opacity;
                    c
                },
            };

            // Draw this sub-segment as a solid-colored rotated rect.
            // The color was evaluated in global (parent) coordinates so the
            // gradient spans the entire polyline uniformly.
            draw_rotated_stroke_segment(p0x, p0y, p1x, p1y, piece_color, adjusted_width, ctx);
        }
    }
}

/// Draw a single rotated-rect line segment with a solid color.
/// This is the inner rendering step extracted from [`stroke_line_segment`]
/// so we can call it per-sub-segment without creating a full [`NodeStyle`].
fn draw_rotated_stroke_segment(
    x1: f32, y1: f32, x2: f32, y2: f32,
    color: ColorF,
    stroke_width: f32,
    ctx: &mut RenderContext,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ZERO_LENGTH_EPSILON { return; }

    let mx = (x1 + x2) / 2.0;
    let my = (y1 + y2) / 2.0;
    let angle = dy.atan2(dx);
    let half_w = stroke_width / 2.0;

    let transform = LayoutTransform::rotation(0.0, 0.0, 1.0, Angle::radians(angle));
    let line_spatial_id = ctx.wr.push_reference_frame(
        LayoutPoint::new(mx, my),
        ctx.spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(transform),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    );

    let line_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(-len / 2.0, -half_w),
        LayoutSize::new(len, stroke_width),
    );

    let common = make_common_props(line_bounds, line_spatial_id, ctx.clip_chain_id);
    ctx.wr.push_rect(&common, line_bounds, color);
    ctx.wr.pop_reference_frame();
}
