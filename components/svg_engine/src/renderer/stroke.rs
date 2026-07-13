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
use lyon::math::Point as LyonPoint;
use webrender_api::units::{
    LayoutPoint, LayoutRect, LayoutSideOffsets, LayoutSize, LayoutTransform,
};
use webrender_api::{
    BorderDetails, BorderRadius, BorderSide, BorderStyle, ClipMode, ColorF, ComplexClipRegion,
    NormalBorder, PropertyBinding, ReferenceFrameKind, TransformStyle,
};

use crate::renderer::{
    RenderContext, ZERO_LENGTH_EPSILON, clip_chain_option, effective_stroke_width, gradient,
    make_common_props, to_colorf,
};
use crate::style::gradient::{GradientDef, GradientUnits, PaintServer};
use crate::style::{Display, LineCap, NodeStyle, StrokeParams, Visibility};

// ======================= Dash Interval Decomposition =======================

/// Decompose a line segment of length `seg_len` into dash/gap intervals
/// based on the SVG `stroke-dasharray` and `stroke-dashoffset`.
///
/// Returns a list of `(t0, t1)` pairs along the segment [0, 1] where
/// dashes should be drawn.  Returns a single `[(0, 1)]` interval when
/// dash_array is empty or all gaps (no dashes remaining after offset).
///
/// Uses `butt` line caps — each dash covers exactly its interval.
pub(crate) fn dash_intervals(
    seg_len: f32,
    dash_array: &[f32],
    dash_offset: f32,
) -> Vec<(f32, f32)> {
    if dash_array.is_empty() || seg_len <= 0.0 {
        return vec![(0.0, 1.0)];
    }

    let pattern_len: f32 = dash_array.iter().sum();
    if pattern_len <= 1e-6 {
        return vec![(0.0, 1.0)];
    }

    // Normalise offset into [0, pattern_len).
    let offset = ((dash_offset % pattern_len) + pattern_len) % pattern_len;

    let mut intervals = Vec::new();
    let mut pos = 0.0; // distance consumed along the segment
    let mut pi: usize = 0; // index into dash_array
    let mut seg_rem = dash_array[0]; // remaining length in current array entry
    let mut is_dash = true; // index 0 is a dash (even = dash, odd = gap)

    // Advance past the offset so we start at the right pattern position.
    let mut o = offset;
    while o > 0.0 {
        if o < seg_rem {
            seg_rem -= o;
            o = 0.0;
        } else {
            o -= seg_rem;
            pi += 1;
            is_dash = !is_dash;
            seg_rem = dash_array[pi % dash_array.len()];
        }
    }

    // Walk the segment and record dash intervals.
    while pos < seg_len - 1e-6 {
        let remaining = seg_len - pos;
        let consume = remaining.min(seg_rem);

        if is_dash && consume > 0.0 {
            let t0 = pos / seg_len;
            let t1 = (pos + consume) / seg_len;
            intervals.push((t0, t1));
        }

        pos += consume;
        seg_rem -= consume;

        if seg_rem <= 1e-6 && pos < seg_len - 1e-6 {
            pi += 1;
            is_dash = !is_dash;
            seg_rem = dash_array[pi % dash_array.len()];
        }
    }

    intervals
}

// ======================= Shared line-segment stroke =======================

/// Render a single line segment as a rotated rect filled with the stroke color
/// or gradient. Handles both solid-color and gradient paint servers.
///
/// Coordinates are in absolute layout space (svg_origin already added).
pub(crate) fn stroke_line_segment(x1: f32, y1: f32, x2: f32, y2: f32, ctx: &mut RenderContext) {
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };
    let stroke_width = effective_stroke_width(ctx, stroke.width);
    if stroke_width <= 0.0 {
        return;
    }

    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ZERO_LENGTH_EPSILON {
        return;
    }

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

    if let Some(svg_color) = stroke.color {
        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;
        emit_rotated_rects_for_segment(len, half_w, color, stroke, line_spatial_id, ctx);
    } else if let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
        if let Some(dash_array) = &stroke.dash_array &&
            !dash_array.is_empty()
        {
            let intervals = dash_intervals(len, dash_array, stroke.dash_offset);
            for (t0, t1) in intervals {
                if t1 <= t0 {
                    continue;
                }
                let dash_start = -len / 2.0 + t0 * len;
                let dash_len = (t1 - t0) * len;
                let mut grad_bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(dash_start, -half_w),
                    LayoutSize::new(dash_len, stroke_width),
                );
                // Apply cap extension for square/round caps.
                if stroke.line_cap != LineCap::Butt {
                    let ext = half_w;
                    grad_bounds = LayoutRect::from_origin_and_size(
                        LayoutPoint::new(dash_start - ext, -half_w),
                        LayoutSize::new(dash_len + 2.0 * ext, stroke_width),
                    );
                }

                // Build clip chain for round caps BEFORE creating grad_ctx
                // (which borrows ctx.wr mutably).
                let round_chain = if stroke.line_cap == LineCap::Round {
                    let radii = BorderRadius {
                        top_left: LayoutSize::new(half_w, half_w),
                        top_right: LayoutSize::new(half_w, half_w),
                        bottom_left: LayoutSize::new(half_w, half_w),
                        bottom_right: LayoutSize::new(half_w, half_w),
                    };
                    let clip_id = ctx.wr.define_clip_rounded_rect(
                        line_spatial_id,
                        ComplexClipRegion {
                            rect: grad_bounds,
                            radii,
                            mode: ClipMode::Clip,
                        },
                    );
                    Some(
                        ctx.wr
                            .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [clip_id]),
                    )
                } else {
                    None
                };

                let mut grad_ctx = RenderContext {
                    style: ctx.style,
                    svg_origin: LayoutPoint::new(0.0, 0.0),
                    spatial_id: line_spatial_id,
                    clip_chain_id: round_chain.unwrap_or(ctx.clip_chain_id),
                    wr: &mut *ctx.wr,
                    paints: ctx.paints,
                    accumulated_scale: ctx.accumulated_scale,
                };
                gradient::fill_rect_with_gradient_by_id(
                    id,
                    grad_bounds,
                    &mut grad_ctx,
                    stroke.opacity * ctx.style.opacity,
                );
            }
            ctx.wr.pop_reference_frame();
            return;
        }
        // Gradient stroke (no dashes) — fill full rotated rect with gradient.
        let mut grad_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(-len / 2.0, -half_w),
            LayoutSize::new(len, stroke_width),
        );
        if stroke.line_cap != LineCap::Butt {
            let ext = half_w;
            grad_bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(-len / 2.0 - ext, -half_w),
                LayoutSize::new(len + 2.0 * ext, stroke_width),
            );
        }

        // Build clip chain for round caps before creating grad_ctx.
        let round_chain = if stroke.line_cap == LineCap::Round {
            let radii = BorderRadius {
                top_left: LayoutSize::new(half_w, half_w),
                top_right: LayoutSize::new(half_w, half_w),
                bottom_left: LayoutSize::new(half_w, half_w),
                bottom_right: LayoutSize::new(half_w, half_w),
            };
            let clip_id = ctx.wr.define_clip_rounded_rect(
                line_spatial_id,
                ComplexClipRegion {
                    rect: grad_bounds,
                    radii,
                    mode: ClipMode::Clip,
                },
            );
            Some(
                ctx.wr
                    .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [clip_id]),
            )
        } else {
            None
        };

        let mut grad_ctx = RenderContext {
            style: ctx.style,
            svg_origin: LayoutPoint::new(0.0, 0.0),
            spatial_id: line_spatial_id,
            clip_chain_id: round_chain.unwrap_or(ctx.clip_chain_id),
            wr: &mut *ctx.wr,
            paints: ctx.paints,
            accumulated_scale: ctx.accumulated_scale,
        };
        gradient::fill_rect_with_gradient_by_id(
            id,
            grad_bounds,
            &mut grad_ctx,
            stroke.opacity * ctx.style.opacity,
        );
    }

    ctx.wr.pop_reference_frame();
}

/// Draw the on-parts of a line segment as rotated sub-rects, respecting
/// `stroke-dasharray`.  Falls back to a single full-length rect when
/// dashes are not enabled or dash_array is empty.
///
/// Must be called inside a push_reference_frame / pop_reference_frame pair
/// where the frame is rotated to align with the segment direction.
fn emit_rotated_rects_for_segment(
    len: f32,
    half_w: f32,
    color: ColorF,
    stroke: &StrokeParams,
    line_spatial_id: webrender_api::SpatialId,
    ctx: &mut RenderContext,
) {
    let stroke_width = half_w * 2.0;

    if let Some(dash_array) = &stroke.dash_array &&
        !dash_array.is_empty()
    {
        let intervals = dash_intervals(len, dash_array, stroke.dash_offset);
        for (t0, t1) in intervals {
            if t1 <= t0 {
                continue;
            }
            let dash_start = -len / 2.0 + t0 * len;
            let dash_len = (t1 - t0) * len;
            let bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(dash_start, -half_w),
                LayoutSize::new(dash_len, stroke_width),
            );
            draw_capped_rect(bounds, half_w, color, stroke.line_cap, line_spatial_id, ctx);
        }
        return;
    }
    // No dashes or empty array — full segment.
    let full_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(-len / 2.0, -half_w),
        LayoutSize::new(len, stroke_width),
    );
    draw_capped_rect(
        full_bounds,
        half_w,
        color,
        stroke.line_cap,
        line_spatial_id,
        ctx,
    );
}

/// Draw a single rotated-rect with the given line cap style.
///
/// - **Butt**: rect covers the exact bounds.
/// - **Square**: rect extends by `half_w` at each end.
/// - **Round**: rect extends by `half_w` and uses a pill-shaped clip
///   (semicircular end caps).
fn draw_capped_rect(
    bounds: LayoutRect,
    half_w: f32,
    color: ColorF,
    line_cap: LineCap,
    spatial_id: webrender_api::SpatialId,
    ctx: &mut RenderContext,
) {
    match line_cap {
        LineCap::Butt => {
            let common = make_common_props(bounds, spatial_id, ctx.clip_chain_id);
            ctx.wr.push_rect(&common, bounds, color);
        },
        LineCap::Square => {
            let extended = LayoutRect::from_origin_and_size(
                LayoutPoint::new(bounds.min.x - half_w, bounds.min.y),
                LayoutSize::new(bounds.size().width + 2.0 * half_w, bounds.size().height),
            );
            let common = make_common_props(extended, spatial_id, ctx.clip_chain_id);
            ctx.wr.push_rect(&common, extended, color);
        },
        LineCap::Round => {
            let extended = LayoutRect::from_origin_and_size(
                LayoutPoint::new(bounds.min.x - half_w, bounds.min.y),
                LayoutSize::new(bounds.size().width + 2.0 * half_w, bounds.size().height),
            );
            let radii = BorderRadius {
                top_left: LayoutSize::new(half_w, half_w),
                top_right: LayoutSize::new(half_w, half_w),
                bottom_left: LayoutSize::new(half_w, half_w),
                bottom_right: LayoutSize::new(half_w, half_w),
            };
            let clip_id = ctx.wr.define_clip_rounded_rect(
                spatial_id,
                ComplexClipRegion {
                    rect: extended,
                    radii,
                    mode: ClipMode::Clip,
                },
            );
            let chain = ctx
                .wr
                .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [clip_id]);
            let common = make_common_props(extended, spatial_id, chain);
            ctx.wr.push_rect(&common, extended, color);
        },
    }
}

// ======================= Rect stroke =======================

/// Stroke an axis-aligned rectangle using the current style's stroke properties.
///
/// Handles both solid-colour borders (via [`push_border`]) and gradient
/// borders (via full-rect gradient + interior white clip).
pub(crate) fn stroke_rect(
    bounds: LayoutRect,
    radii: Option<BorderRadius>,
    ctx: &mut RenderContext,
) {
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };

    if let Some(svg_color) = stroke.color {
        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;
        let stroke_width = effective_stroke_width(ctx, stroke.width);
        let widths = LayoutSideOffsets::new_all_same(stroke_width);
        let details = BorderDetails::Normal(NormalBorder {
            left: BorderSide {
                color,
                style: BorderStyle::Solid,
            },
            right: BorderSide {
                color,
                style: BorderStyle::Solid,
            },
            top: BorderSide {
                color,
                style: BorderStyle::Solid,
            },
            bottom: BorderSide {
                color,
                style: BorderStyle::Solid,
            },
            radius: radii.unwrap_or(BorderRadius {
                top_left: LayoutSize::zero(),
                top_right: LayoutSize::zero(),
                bottom_left: LayoutSize::zero(),
                bottom_right: LayoutSize::zero(),
            }),
            do_aa: true,
        });
        let common = make_common_props(bounds, ctx.spatial_id, ctx.clip_chain_id);
        ctx.wr.push_border(&common, bounds, widths, details);
    } else if let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
        // Gradient border: render gradient clipped to the outer shape
        // (with radii for circles/ellipses), then punch out the interior
        // with white so only the border band shows the gradient.
        let stroke_width = effective_stroke_width(ctx, stroke.width);
        let inset = stroke_width;
        let inner_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bounds.min.x + inset, bounds.min.y + inset),
            LayoutSize::new(
                (bounds.size().width - inset * 2.0).max(0.0),
                (bounds.size().height - inset * 2.0).max(0.0),
            ),
        );
        // Compute inner radii for the punch-out (outer radii shrunk by stroke).
        let inner_radii = radii.map(|r| BorderRadius {
            top_left: LayoutSize::new(
                (r.top_left.width - stroke_width).max(0.0),
                (r.top_left.height - stroke_width).max(0.0),
            ),
            top_right: LayoutSize::new(
                (r.top_right.width - stroke_width).max(0.0),
                (r.top_right.height - stroke_width).max(0.0),
            ),
            bottom_left: LayoutSize::new(
                (r.bottom_left.width - stroke_width).max(0.0),
                (r.bottom_left.height - stroke_width).max(0.0),
            ),
            bottom_right: LayoutSize::new(
                (r.bottom_right.width - stroke_width).max(0.0),
                (r.bottom_right.height - stroke_width).max(0.0),
            ),
        });

        // Phase 1: fill the full rect with gradient, clipped to outer radii.
        let outer_clip = if let Some(r) = radii {
            let clip_id = ctx.wr.define_clip_rounded_rect(
                ctx.spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii: r,
                    mode: ClipMode::Clip,
                },
            );
            ctx.wr
                .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [clip_id])
        } else {
            ctx.clip_chain_id
        };
        let saved_clip = ctx.clip_chain_id;
        ctx.clip_chain_id = outer_clip;
        gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, stroke.opacity);

        // Phase 2: punch out the interior so only the border band remains.
        if inner_bounds.size().width > 0.0 && inner_bounds.size().height > 0.0 {
            let inner_clip_id = if let Some(ir) = inner_radii {
                ctx.wr.define_clip_rounded_rect(
                    ctx.spatial_id,
                    ComplexClipRegion {
                        rect: inner_bounds,
                        radii: ir,
                        mode: ClipMode::Clip,
                    },
                )
            } else {
                ctx.wr.define_clip_rect(ctx.spatial_id, inner_bounds)
            };
            let inner_chain = ctx
                .wr
                .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [inner_clip_id]);
            ctx.clip_chain_id = inner_chain;
            let white = ColorF::new(1.0, 1.0, 1.0, 1.0);
            let common = make_common_props(bounds, ctx.spatial_id, inner_chain);
            ctx.wr.push_rect(&common, bounds, white);
        }
        ctx.clip_chain_id = saved_clip;
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
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };
    let adjusted_width = effective_stroke_width(ctx, stroke.width);
    if (stroke.color.is_none() && stroke.paint_server.is_none()) || adjusted_width <= 0.0 {
        return;
    }

    // Gradient stroke: evaluate at each segment's midpoint so the gradient
    // spans the whole shape, not each segment independently.
    if stroke.color.is_none() &&
        let Some(PaintServer::Gradient(id)) = &stroke.paint_server
    {
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
        stroke_line_segment(
            line_ctx.svg_origin.x + pair[0].x,
            line_ctx.svg_origin.y + pair[0].y,
            line_ctx.svg_origin.x + pair[1].x,
            line_ctx.svg_origin.y + pair[1].y,
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
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };
    let Some(grad_def) = ctx.paints.gradient(grad_id) else {
        log::warn!("SVG gradient \"{}\" not found for stroke", grad_id);
        return;
    };

    // Compute overall bounding box of the polyline.
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in pts {
        if p.x < min_x {
            min_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y > max_y {
            max_y = p.y;
        }
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
                0.0,
                0.0, // unused for radial
            ),
            GradientUnits::UserSpaceOnUse => (
                ctx.svg_origin.x + rg.cx.to_user_space(bbox_w),
                ctx.svg_origin.y + rg.cy.to_user_space(bbox_h),
                0.0,
                0.0, // unused for radial
            ),
        },
    };

    // Pre-compute radial focal point and radius in absolute space for later use.
    let rad_fx_fy_r2 = match grad_def {
        GradientDef::Radial(rg) => {
            let (fx, fy, r2) = match rg.units {
                GradientUnits::ObjectBoundingBox => {
                    let scale = bbox_w.max(bbox_h);
                    (
                        ctx.svg_origin.x + min_x + rg.fx.to_object_bbox() * bbox_w,
                        ctx.svg_origin.y + min_y + rg.fy.to_object_bbox() * bbox_h,
                        (rg.r.to_object_bbox() * scale).max(1.0),
                    )
                },
                GradientUnits::UserSpaceOnUse => {
                    let scale = bbox_w.max(bbox_h);
                    (
                        ctx.svg_origin.x + rg.fx.to_user_space(bbox_w),
                        ctx.svg_origin.y + rg.fy.to_user_space(bbox_h),
                        rg.r.to_user_space(scale).max(1.0),
                    )
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
        let ax = ctx.svg_origin.x + pair[0].x;
        let ay = ctx.svg_origin.y + pair[0].y;
        let bx = ctx.svg_origin.x + pair[1].x;
        let by = ctx.svg_origin.y + pair[1].y;

        let seg_dx = bx - ax;
        let seg_dy = by - ay;
        let seg_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();
        if seg_len < ZERO_LENGTH_EPSILON {
            continue;
        }

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
            let seg_stroke = SegmentStrokeParams {
                color: piece_color,
                width: adjusted_width,
                line_cap: stroke.line_cap,
            };
            draw_rotated_stroke_segment(p0x, p0y, p1x, p1y, &seg_stroke, ctx);
        }
    }
}

/// Bundled stroke parameters for [`draw_rotated_stroke_segment`].
/// Reduces argument count from 8 to 6 (clippy: too_many_arguments).
struct SegmentStrokeParams {
    color: ColorF,
    width: f32,
    line_cap: LineCap,
}

/// Draw a single rotated-rect line segment with a solid color.
/// This is the inner rendering step extracted from [`stroke_line_segment`]
/// so we can call it per-sub-segment without creating a full [`NodeStyle`].
fn draw_rotated_stroke_segment(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    stroke: &SegmentStrokeParams,
    ctx: &mut RenderContext,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ZERO_LENGTH_EPSILON {
        return;
    }

    let mx = (x1 + x2) / 2.0;
    let my = (y1 + y2) / 2.0;
    let angle = dy.atan2(dx);
    let half_w = stroke.width / 2.0;

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
        LayoutSize::new(len, stroke.width),
    );

    draw_capped_rect(
        line_bounds,
        half_w,
        stroke.color,
        stroke.line_cap,
        line_spatial_id,
        ctx,
    );
    ctx.wr.pop_reference_frame();
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::dash_intervals;

    fn approx(a: &[(f32, f32)], b: &[(f32, f32)]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        const EPS: f32 = 0.001;
        for (i, (x, y)) in a.iter().enumerate() {
            let (bx, by) = b[i];
            if (x - bx).abs() > EPS || (y - by).abs() > EPS {
                return false;
            }
        }
        true
    }

    #[test]
    fn empty_dash_array_returns_full_segment() {
        assert_eq!(dash_intervals(100.0, &[], 0.0), vec![(0.0, 1.0)]);
    }

    #[test]
    fn single_dash_no_gap() {
        // Single value [10] means 10-on, 10-off, 10-on, 10-off, ...
        let r = dash_intervals(20.0, &[10.0], 0.0);
        assert!(approx(&r, &[(0.0, 0.5)]), "got {:?}", r);
    }

    #[test]
    fn basic_dash_pattern() {
        let r = dash_intervals(20.0, &[6.0, 4.0], 0.0);
        // 6 dash, 4 gap, 6 dash, 4 gap (but only 20 total)
        // 0-6 dash, 6-10 gap, 10-16 dash, 16-20 gap
        assert!(approx(&r, &[(0.0, 0.3), (0.5, 0.8)]), "got {:?}", r);
    }

    #[test]
    fn dash_offset_shifts_pattern() {
        let r = dash_intervals(20.0, &[6.0, 4.0], 5.0);
        // offset=5 into [6,4]: 5 consumed from first 6 → 1 left of dash
        // 0-1 dash, 1-5 gap, 5-11 dash, 11-15 gap, 15-20 dash (partial)
        // Wait, let me re-check: offset=5 means we start 5 units into the pattern.
        // pattern [6,4] at offset 5: we've consumed 5 of the first 6 → 1 of dash left
        // 0-1 dash, 1-5 gap (4 units), 5-11 dash (6 units), 11-15 gap (4 units), 15-20 dash (5 units)
        assert!(
            approx(&r, &[(0.0, 0.05), (0.25, 0.55), (0.75, 1.0)]),
            "got {:?}",
            r
        );
    }

    #[test]
    fn dash_offset_start_of_gap() {
        let r = dash_intervals(20.0, &[6.0, 4.0], 6.0);
        // offset=6: consumed all 6 of dash → at start of gap
        // 0-4 gap, 4-10 dash, 10-14 gap, 14-20 dash
        assert!(approx(&r, &[(0.2, 0.5), (0.7, 1.0)]), "got {:?}", r);
    }

    #[test]
    fn dash_offset_middle_of_gap() {
        let r = dash_intervals(20.0, &[6.0, 4.0], 8.0);
        // offset=8: consumed 6 dash + 2 gap → 2 of gap left
        // 0-2 gap, 2-8 dash, 8-12 gap, 12-18 dash, 18-20 gap
        assert!(approx(&r, &[(0.1, 0.4), (0.6, 0.9)]), "got {:?}", r);
    }

    #[test]
    fn segment_shorter_than_dash() {
        let r = dash_intervals(3.0, &[10.0, 5.0], 0.0);
        assert!(approx(&r, &[(0.0, 1.0)]), "got {:?}", r);
    }

    #[test]
    fn segment_shorter_than_gap() {
        let r = dash_intervals(3.0, &[2.0, 10.0], 2.0);
        // offset=2: consumed all 2 of dash → at start of 10-unit gap
        // Entire segment is in gap → no dashes
        assert!(r.is_empty(), "got {:?}", r);
    }

    #[test]
    fn single_element_pattern_alternating() {
        let r = dash_intervals(30.0, &[5.0], 0.0);
        // [5] means 5-on, 5-off, 5-on, 5-off, ...
        // 0-5 dash, 5-10 gap, 10-15 dash, 15-20 gap, 20-25 dash, 25-30 gap
        assert!(
            approx(
                &r,
                &[(0.0, 1.0 / 6.0), (1.0 / 3.0, 0.5), (2.0 / 3.0, 5.0 / 6.0)]
            ),
            "got {:?}",
            r
        );
    }

    #[test]
    fn zero_length_segment() {
        assert_eq!(dash_intervals(0.0, &[5.0, 2.0], 0.0), vec![(0.0, 1.0)]);
    }

    #[test]
    fn negative_offset() {
        let r = dash_intervals(20.0, &[6.0, 4.0], -2.0);
        // offset=-2: from end of pattern [6,4]=10, -2 = position 8
        // = middle of gap (consumed 6 dash + 2 gap → 2 of gap left)
        assert!(approx(&r, &[(0.1, 0.4), (0.6, 0.9)]), "got {:?}", r);
    }

    #[test]
    fn large_offset_wraps_pattern() {
        let r = dash_intervals(20.0, &[6.0, 4.0], 25.0);
        // offset=25: 25 % 10 = 5, same as offset=5 test
        assert!(
            approx(&r, &[(0.0, 0.05), (0.25, 0.55), (0.75, 1.0)]),
            "got {:?}",
            r
        );
    }

    #[test]
    fn segment_exactly_one_dash() {
        let r = dash_intervals(6.0, &[6.0, 4.0], 0.0);
        assert!(approx(&r, &[(0.0, 1.0)]), "got {:?}", r);
    }

    #[test]
    fn segment_exactly_one_full_pattern() {
        let r = dash_intervals(10.0, &[6.0, 4.0], 0.0);
        assert!(approx(&r, &[(0.0, 0.6)]), "got {:?}", r);
    }
}
