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

use std::sync::Arc;

use euclid::Angle;
use lyon::math::Point as LyonPoint;
use webrender_api::units::{
    LayoutPoint, LayoutRect, LayoutSideOffsets, LayoutSize, LayoutTransform,
};
use webrender_api::{
    BorderDetails, BorderRadius, BorderSide, BorderStyle, ClipMode, ColorF, ComplexClipRegion,
    NormalBorder, PropertyBinding, ReferenceFrameKind, TransformStyle,
};

use crate::model::document::{GradientDef, GradientUnits};
use crate::model::style::paint_servers::PaintServer;
use crate::model::style::{Display, FillRule, LineCap, LineJoin, NodeStyle, StrokeParams, Visibility};
use crate::model::units::Length;
use crate::render::renderer::{
    RenderContext, ZERO_LENGTH_EPSILON, clip_chain_option, crisp_edges, effective_stroke_width,
    effective_stroke_widths, gradient, make_common_props, to_colorf,
};
use crate::render::tessellator::{FillStyle, tessellate_contours, tessellate_polygon};

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
    let stroke_width = effective_stroke_width(ctx, stroke.width.get());
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

    // `shape-rendering="crispEdges"` — a solid stroke is rasterized via the
    // scanline tessellator (aliased spans) instead of a rotated rect, because
    // WebRender always anti-aliases a rotated rect's diagonal edges.
    if crisp_edges(ctx) {
        let paint = stroke
            .paint_server
            .as_ref()
            .and_then(|p| ctx.defs.resolve_paint_server(p));
        if let Some(PaintServer::Solid(svg_color)) = &paint {
            let mut color = to_colorf(svg_color);
            color.a *= stroke.opacity.get() * ctx.style.opacity.get();
            if let Some(dash_array) = &stroke.dash_array &&
                !dash_array.is_empty()
            {
                for (t0, t1) in dash_intervals(len, dash_array, stroke.dash_offset) {
                    if t1 <= t0 {
                        continue;
                    }
                    stroke_line_segment_crisp(
                        x1 + dx * t0,
                        y1 + dy * t0,
                        x1 + dx * t1,
                        y1 + dy * t1,
                        half_w,
                        color,
                        stroke.line_cap,
                        ctx,
                    );
                }
            } else {
                stroke_line_segment_crisp(x1, y1, x2, y2, half_w, color, stroke.line_cap, ctx);
            }
            return;
        }
    }

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

    let paint = stroke
        .paint_server
        .as_ref()
        .and_then(|p| ctx.defs.resolve_paint_server(p));
    if let Some(PaintServer::Solid(svg_color)) = &paint {
        let mut color = to_colorf(svg_color);
        color.a *= stroke.opacity.get() * ctx.style.opacity.get();
        emit_rotated_rects_for_segment(len, half_w, color, stroke, line_spatial_id, ctx);
    } else if let Some(PaintServer::Gradient(def)) = &paint {
        let def = Arc::clone(def);
        if let Some(dash_array) = &stroke.dash_array &&
            !dash_array.is_empty()
        {
            for (t0, t1) in dash_intervals(len, dash_array, stroke.dash_offset) {
                if t1 <= t0 {
                    continue;
                }
                let dash_start = -len / 2.0 + t0 * len;
                let dash_len = (t1 - t0) * len;
                fill_gradient_stroke(
                    def.as_ref(),
                    dash_start,
                    dash_len,
                    stroke_width,
                    half_w,
                    stroke,
                    line_spatial_id,
                    ctx,
                );
            }
            ctx.wr.pop_reference_frame();
            return;
        }
        // Gradient stroke (no dashes) — fill full rotated rect.
        fill_gradient_stroke(
            def.as_ref(),
            -len / 2.0,
            len,
            stroke_width,
            half_w,
            stroke,
            line_spatial_id,
            ctx,
        );
    }

    ctx.wr.pop_reference_frame();
}

/// Fill a gradient stroke segment with bounds expansion for non-butt caps
/// and rounded-rect clip for round caps.
fn fill_gradient_stroke(
    def: &GradientDef,
    start_x: f32,
    rect_len: f32,
    stroke_width: f32,
    half_w: f32,
    stroke: &crate::model::style::StrokeParams,
    line_spatial_id: webrender_api::SpatialId,
    ctx: &mut RenderContext,
) {
    let mut grad_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(start_x, -half_w),
        LayoutSize::new(rect_len, stroke_width),
    );
    // Extend bounds for square/round caps.
    if stroke.line_cap != LineCap::Butt {
        let ext = half_w;
        grad_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(start_x - ext, -half_w),
            LayoutSize::new(rect_len + 2.0 * ext, stroke_width),
        );
    }
    // Build rounded-rect clip for round caps.
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
        accumulated_scale: ctx.accumulated_scale,
        viewbox_scale: ctx.viewbox_scale,
        device_scale: ctx.device_scale,
        raster_offset: ctx.raster_offset,
        native_rendering: ctx.native_rendering,
        sink: ctx.sink,
        defs: ctx.defs,
        def_depth: ctx.def_depth,
    };
    gradient::fill_rect_with_gradient(
        def,
        grad_bounds,
        &mut grad_ctx,
        stroke.opacity.get() * ctx.style.opacity.get(),
    );
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

    let paint = stroke
        .paint_server
        .as_ref()
        .and_then(|p| ctx.defs.resolve_paint_server(p));

    // `shape-rendering="crispEdges"` — a rounded (circle/ellipse/rounded-rect)
    // border is always anti-aliased by WebRender, so rasterize the ring via the
    // scanline tessellator to get genuinely aliased edges. Plain axis-aligned
    // rects are already crisp and need no special handling.
    if crisp_edges(ctx) &&
        let Some(r) = radii &&
        let Some(PaintServer::Solid(svg_color)) = &paint
    {
        let mut color = to_colorf(svg_color);
        color.a *= stroke.opacity.get() * ctx.style.opacity.get();
        let stroke_width = effective_stroke_width(ctx, stroke.width.get());
        stroke_rounded_rect_crisp(bounds, r, stroke_width, color, ctx);
        return;
    }

    if let Some(PaintServer::Solid(svg_color)) = &paint {
        let mut color = to_colorf(svg_color);
        color.a *= stroke.opacity.get() * ctx.style.opacity.get();
        let (stroke_width_x, stroke_width_y) = effective_stroke_widths(ctx, stroke.width.get());
        let widths = LayoutSideOffsets::new(
            stroke_width_y,
            stroke_width_x,
            stroke_width_y,
            stroke_width_x,
        );
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
    } else if let Some(PaintServer::Gradient(def)) = &paint {
        let def = Arc::clone(def);
        // Gradient border: clip a full-rect gradient fill to a band between the
        // outer shape outline (with radii for circles/ellipses) and the inset
        // inner outline, using a Clip/ClipOut clip chain. This works on any
        // background, unlike the previous white interior "punch-out".
        let stroke_width = effective_stroke_width(ctx, stroke.width.get());
        let inset = stroke_width;
        let inner_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bounds.min.x + inset, bounds.min.y + inset),
            LayoutSize::new(
                (bounds.size().width - inset * 2.0).max(0.0),
                (bounds.size().height - inset * 2.0).max(0.0),
            ),
        );
        // Compute inner radii for the clip-out (outer radii shrunk by stroke).
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

        // Outer outline clip (Clip) — the gradient is confined to the shape.
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

        // Build the band clip chain: outer outline (Clip) minus inner outline
        // (ClipOut), leaving only the stroke band visible.
        let band_clip = if inner_bounds.size().width > 0.0 && inner_bounds.size().height > 0.0 {
            let inner_id = if let Some(ir) = inner_radii {
                ctx.wr.define_clip_rounded_rect(
                    ctx.spatial_id,
                    ComplexClipRegion {
                        rect: inner_bounds,
                        radii: ir,
                        mode: ClipMode::ClipOut,
                    },
                )
            } else {
                ctx.wr.define_clip_rounded_rect(
                    ctx.spatial_id,
                    ComplexClipRegion {
                        rect: inner_bounds,
                        radii: BorderRadius {
                            top_left: LayoutSize::zero(),
                            top_right: LayoutSize::zero(),
                            bottom_left: LayoutSize::zero(),
                            bottom_right: LayoutSize::zero(),
                        },
                        mode: ClipMode::ClipOut,
                    },
                )
            };
            ctx.wr
                .define_clip_chain(clip_chain_option(outer_clip), [inner_id])
        } else {
            outer_clip
        };

        let saved_clip = ctx.clip_chain_id;
        ctx.clip_chain_id = band_clip;
        gradient::fill_rect_with_gradient(def.as_ref(), bounds, ctx, stroke.opacity.get());
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
    let adjusted_width = effective_stroke_width(ctx, stroke.width.get());
    if stroke.paint_server.is_none() || adjusted_width <= 0.0 {
        return;
    }

    // Gradient stroke: evaluate at each segment's midpoint so the gradient
    // spans the whole shape, not each segment independently.
    let paint = stroke
        .paint_server
        .as_ref()
        .and_then(|p| ctx.defs.resolve_paint_server(p));
    if let Some(PaintServer::Gradient(def)) = &paint {
        let def = Arc::clone(def);
        return stroke_polyline_gradient(pts, ctx, adjusted_width, def.as_ref());
    }

    // Solid color stroke — original per-segment approach.
    let stroke_style = NodeStyle {
        visibility: Visibility::Visible,
        display: Display::Inline,
        fill: None,
        render_hints: None,
        clip_path: None,
        mask: None,
        filter: None,
        opacity: ctx.style.opacity,
        markers: None,
        pointer_events_none: false,
        stroke: Some(StrokeParams {
            paint_server: stroke.paint_server.clone(),
            opacity: stroke.opacity,
            width: Length::new(adjusted_width),
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
        accumulated_scale: ctx.accumulated_scale,
        viewbox_scale: ctx.viewbox_scale,
        device_scale: ctx.device_scale,
        raster_offset: ctx.raster_offset,
        native_rendering: ctx.native_rendering,
        sink: ctx.sink,
        defs: ctx.defs,
        def_depth: ctx.def_depth,
    };

    // Fill the join wedges first so the per-segment bands paint over any
    // shared edge, avoiding double-blend seams at the corner.
    if pts.len() >= 3 {
        stroke_polyline_joins(pts, adjusted_width, &mut line_ctx);
    }

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

/// Fill the `stroke-linejoin` wedges at each interior vertex of a polyline.
///
/// The per-segment rotated rects are drawn with butt ends, leaving a
/// wedge-shaped gap at each corner. This fills that gap according to
/// `stroke-linejoin` (miter / bevel / round) using the scanline tessellator,
/// which — unlike vello_cpu — honors reference frames (required for pattern
/// content).
fn stroke_polyline_joins(pts: &[LyonPoint], adjusted_width: f32, ctx: &mut RenderContext) {
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };
    let Some(PaintServer::Solid(svg_color)) = stroke
        .paint_server
        .as_ref()
        .and_then(|p| ctx.defs.resolve_paint_server(p))
    else {
        return;
    };
    let mut color = to_colorf(&svg_color);
    color.a *= stroke.opacity.get() * ctx.style.opacity.get();
    let half_width = adjusted_width / 2.0;
    if half_width <= 0.0 {
        return;
    }

    for i in 1..pts.len().saturating_sub(1) {
        let a = LyonPoint::new(ctx.svg_origin.x + pts[i - 1].x, ctx.svg_origin.y + pts[i - 1].y);
        let b = LyonPoint::new(ctx.svg_origin.x + pts[i].x, ctx.svg_origin.y + pts[i].y);
        let c = LyonPoint::new(ctx.svg_origin.x + pts[i + 1].x, ctx.svg_origin.y + pts[i + 1].y);
        let Some(wedge) = stroke_join_wedge(a, b, c, half_width, stroke.line_join, stroke.miter_limit)
        else {
            continue;
        };
        let fill = FillStyle::Solid(color);
        tessellate_polygon(&wedge, FillRule::NonZero, &fill, ctx);
    }

    // Closed contour: `close()` re-appends the start point, so `pts.first()`
    // equals `pts.last()`. The interior loop above skips both the first and the
    // duplicate last vertex, leaving a wedge-shaped gap at the seam where the
    // closing segment meets the first segment. Fill that seam too, using the
    // closing segment (last distinct point → start) and the first segment
    // (start → second point).
    if pts.len() >= 3 && pts.first() == pts.last() {
        let a = LyonPoint::new(
            ctx.svg_origin.x + pts[pts.len() - 2].x,
            ctx.svg_origin.y + pts[pts.len() - 2].y,
        );
        let b = LyonPoint::new(ctx.svg_origin.x + pts[0].x, ctx.svg_origin.y + pts[0].y);
        let c = LyonPoint::new(ctx.svg_origin.x + pts[1].x, ctx.svg_origin.y + pts[1].y);
        if let Some(wedge) =
            stroke_join_wedge(a, b, c, half_width, stroke.line_join, stroke.miter_limit)
        {
            let fill = FillStyle::Solid(color);
            tessellate_polygon(&wedge, FillRule::NonZero, &fill, ctx);
        }
    }
}

/// Compute the stroke join wedge polygon at vertex `b` (between segments `a→b`
/// and `b→c`), in absolute coordinates. Returns `None` when no join is needed
/// (degenerate or collinear segments).
///
/// The wedge is the small convex region the per-segment stroke bands leave
/// uncovered at a corner. `stroke-linejoin` selects its shape:
/// - **miter**: the two outer edges are extended to their intersection;
///   falls back to a bevel when the miter length ratio exceeds `miter_limit`.
/// - **bevel**: a straight cut between the two outer corners.
/// - **round**: an arc between the two outer corners.
fn stroke_join_wedge(
    a: LyonPoint,
    b: LyonPoint,
    c: LyonPoint,
    half_width: f32,
    join: LineJoin,
    miter_limit: f32,
) -> Option<Vec<LyonPoint>> {
    let (mut d_inx, mut d_iny) = (b.x - a.x, b.y - a.y);
    let l1 = (d_inx * d_inx + d_iny * d_iny).sqrt();
    if l1 < ZERO_LENGTH_EPSILON {
        return None;
    }
    (d_inx, d_iny) = (d_inx / l1, d_iny / l1);
    let (mut d_outx, mut d_outy) = (c.x - b.x, c.y - b.y);
    let l2 = (d_outx * d_outx + d_outy * d_outy).sqrt();
    if l2 < ZERO_LENGTH_EPSILON {
        return None;
    }
    (d_outx, d_outy) = (d_outx / l2, d_outy / l2);

    let cross = d_inx * d_outy - d_iny * d_outx;
    if cross.abs() < 1e-6 {
        return None;
    }
    // The outer (convex) side normal of a direction `d` is `sign(cross) * (d.y, -d.x)`.
    let s = if cross > 0.0 { 1.0 } else { -1.0 };
    let (n1x, n1y) = (s * d_iny, -s * d_inx);
    let (n2x, n2y) = (s * d_outy, -s * d_outx);
    let p1 = LyonPoint::new(b.x + half_width * n1x, b.y + half_width * n1y);
    let p2 = LyonPoint::new(b.x + half_width * n2x, b.y + half_width * n2y);

    match join {
        LineJoin::Bevel => Some(vec![b, p1, p2]),
        LineJoin::Miter | LineJoin::MiterClip => {
            let m = line_intersection(p1, (d_inx, d_iny), p2, (d_outx, d_outy))?;
            let miter_len = ((m.x - b.x).powi(2) + (m.y - b.y).powi(2)).sqrt();
            let width = 2.0 * half_width;
            if width > 0.0 && miter_len / width > miter_limit.max(1.0) {
                Some(vec![b, p1, p2]) // miter limit exceeded → bevel
            } else {
                Some(vec![b, p1, m, p2])
            }
        },
        LineJoin::Round | LineJoin::Arcs => {
            let ang1 = n1y.atan2(n1x);
            let ang2 = n2y.atan2(n2x);
            // Sweep from `ang1` to `ang2` along the short outer arc.
            let mut sweep = ang2 - ang1;
            while sweep > std::f32::consts::PI {
                sweep -= std::f32::consts::TAU;
            }
            while sweep <= -std::f32::consts::PI {
                sweep += std::f32::consts::TAU;
            }
            let steps = ((sweep.abs() / std::f32::consts::FRAC_PI_6).ceil() as usize).clamp(1, 32);
            let mut wedge = Vec::with_capacity(steps + 2);
            wedge.push(b);
            for k in 0..=steps {
                let ang = ang1 + sweep * (k as f32 / steps as f32);
                wedge.push(LyonPoint::new(
                    b.x + half_width * ang.cos(),
                    b.y + half_width * ang.sin(),
                ));
            }
            Some(wedge)
        },
    }
}

/// Intersection of the two infinite lines `p + t*r` and `q + u*s`.
fn line_intersection(
    p: LyonPoint,
    r: (f32, f32),
    q: LyonPoint,
    s: (f32, f32),
) -> Option<LyonPoint> {
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = ((q.x - p.x) * s.1 - (q.y - p.y) * s.0) / denom;
    Some(LyonPoint::new(p.x + t * r.0, p.y + t * r.1))
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
    grad_def: &GradientDef,
) {
    let Some(stroke) = &ctx.style.stroke else {
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

    // Pre-compute radial focal point, outer radii, and focal radius in absolute
    // space for later use.  The outer radii resolve against the bbox width/height
    // respectively (an ellipse under §13.2.2's non-uniform bounding-box scale).
    let rad_fx_fy_rx_ry = match grad_def {
        GradientDef::Radial(rg) => {
            let (fx, fy, rx, ry, fr) = match rg.units {
                GradientUnits::ObjectBoundingBox => {
                    let scale = gradient::normalized_diagonal(bbox_w, bbox_h);
                    (
                        ctx.svg_origin.x + min_x + rg.fx.to_object_bbox() * bbox_w,
                        ctx.svg_origin.y + min_y + rg.fy.to_object_bbox() * bbox_h,
                        (rg.r.to_object_bbox() * bbox_w).max(1.0),
                        (rg.r.to_object_bbox() * bbox_h).max(1.0),
                        (rg.fr.to_object_bbox() * scale).max(0.0),
                    )
                },
                GradientUnits::UserSpaceOnUse => {
                    // User space is uniform, so a radial gradient is a circle here.
                    let scale = gradient::normalized_diagonal(bbox_w, bbox_h);
                    let r = rg.r.to_user_space(scale).max(1.0);
                    (
                        ctx.svg_origin.x + rg.fx.to_user_space(bbox_w),
                        ctx.svg_origin.y + rg.fy.to_user_space(bbox_h),
                        r,
                        r,
                        rg.fr.to_user_space(scale).max(0.0),
                    )
                },
            };
            Some((fx, fy, rx, ry, fr))
        },
        GradientDef::Linear(_) => None,
    };

    let opacity = stroke.opacity.get() * ctx.style.opacity.get();
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
                    // `rad_fx_fy_rx_ry` is `Some` whenever `grad_def` is radial;
                    // this guard is defensive only.
                    let Some((fx, fy, rx, ry, fr)) = rad_fx_fy_rx_ry else {
                        continue;
                    };
                    let dx = mx - fx;
                    let dy = my - fy;
                    // Normalized elliptical distance: `1.0` on the outer ellipse.
                    let d = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
                    let fr_norm = fr / rx.min(ry);
                    let t = if d <= fr_norm {
                        0.0
                    } else if fr_norm >= 1.0 {
                        1.0
                    } else {
                        ((d - fr_norm) / (1.0 - fr_norm)).min(1.0)
                    };
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

// ======================= Crisp (aliased) stroke =======================

/// Arc segments per 90° corner. For a circle this yields 64 points around the
/// full circumference — a maximum chord deviation well under a pixel at the
/// radii used in typical SVG icons, so the polygon approximation is invisible
/// relative to the deliberate aliasing of `crispEdges`.
const CRISP_ARC_SEGMENTS: usize = 16;

/// Append a quarter-ellipse arc to `pts`, sweeping `sweep` radians from `a0`.
/// Points are `(cx + rx·cos a, cy + ry·sin a)` in screen space (y-down).
/// Consecutive duplicates (which arise where an arc meets a straight edge)
/// are skipped so the tessellator never sees a zero-length edge.
fn push_arc(pts: &mut Vec<LyonPoint>, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, sweep: f32) {
    for k in 0..=CRISP_ARC_SEGMENTS {
        let a = a0 + sweep * (k as f32 / CRISP_ARC_SEGMENTS as f32);
        push_unique(pts, LyonPoint::new(cx + rx * a.cos(), cy + ry * a.sin()));
    }
}

/// Append `p` unless it coincides with the last point already in `pts`.
fn push_unique(pts: &mut Vec<LyonPoint>, p: LyonPoint) {
    let dup = pts.last().is_some_and(|l| (l.x - p.x).abs() < 1e-4 && (l.y - p.y).abs() < 1e-4);
    if !dup {
        pts.push(p);
    }
}

/// Flatten a rounded rectangle (or full ellipse, when `rx`/`ry` are half the
/// size) into a closed polygon outline. Clockwise in screen space.
fn rounded_rect_outline(bounds: LayoutRect, rx: f32, ry: f32) -> Vec<LyonPoint> {
    let x0 = bounds.min.x;
    let y0 = bounds.min.y;
    let w = bounds.size().width;
    let h = bounds.size().height;
    let rx = rx.clamp(0.0, w * 0.5);
    let ry = ry.clamp(0.0, h * 0.5);

    let mut pts = Vec::new();
    if rx <= 0.0 || ry <= 0.0 {
        push_unique(&mut pts, LyonPoint::new(x0, y0));
        push_unique(&mut pts, LyonPoint::new(x0 + w, y0));
        push_unique(&mut pts, LyonPoint::new(x0 + w, y0 + h));
        push_unique(&mut pts, LyonPoint::new(x0, y0 + h));
        return pts;
    }

    // Top edge.
    push_unique(&mut pts, LyonPoint::new(x0 + rx, y0));
    push_unique(&mut pts, LyonPoint::new(x0 + w - rx, y0));
    // Top-right corner, from "up" (-90°) to "right" (0°).
    push_arc(
        &mut pts,
        x0 + w - rx,
        y0 + ry,
        rx,
        ry,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
    );
    // Right edge.
    push_unique(&mut pts, LyonPoint::new(x0 + w, y0 + h - ry));
    // Bottom-right corner, 0° → 90°.
    push_arc(
        &mut pts,
        x0 + w - rx,
        y0 + h - ry,
        rx,
        ry,
        0.0,
        std::f32::consts::FRAC_PI_2,
    );
    // Bottom edge.
    push_unique(&mut pts, LyonPoint::new(x0 + rx, y0 + h));
    // Bottom-left corner, 90° → 180°.
    push_arc(
        &mut pts,
        x0 + rx,
        y0 + h - ry,
        rx,
        ry,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2,
    );
    // Left edge.
    push_unique(&mut pts, LyonPoint::new(x0, y0 + ry));
    // Top-left corner, 180° → 270°.
    push_arc(
        &mut pts,
        x0 + rx,
        y0 + ry,
        rx,
        ry,
        std::f32::consts::PI,
        std::f32::consts::FRAC_PI_2,
    );
    pts
}

/// Stroke a rounded rectangle (circle/ellipse/rounded-rect) with a solid colour
/// using crisp, aliased edges. The ring is the band between the outer outline
/// and the outline inset by `stroke_width` — the same geometry WebRender's
/// `push_border` produces for a rounded border, so a `crispEdges` shape matches
/// its `geometricPrecision` twin in size and differs only in aliasing.
fn stroke_rounded_rect_crisp(
    bounds: LayoutRect,
    radii: BorderRadius,
    stroke_width: f32,
    color: ColorF,
    ctx: &mut RenderContext,
) {
    // `rect_bounds_and_radii` sets all four corners to the same (rx, ry).
    let rx = radii.top_left.width;
    let ry = radii.top_left.height;

    let outer = rounded_rect_outline(bounds, rx, ry);
    let inner_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(bounds.min.x + stroke_width, bounds.min.y + stroke_width),
        LayoutSize::new(
            (bounds.size().width - stroke_width * 2.0).max(0.0),
            (bounds.size().height - stroke_width * 2.0).max(0.0),
        ),
    );
    let inner = rounded_rect_outline(
        inner_bounds,
        (rx - stroke_width).max(0.0),
        (ry - stroke_width).max(0.0),
    );

    let fill = FillStyle::Solid(color);
    tessellate_contours(&[&outer, &inner], FillRule::EvenOdd, &fill, ctx);
}

/// Stroke a single line segment with a solid colour using crisp, aliased edges.
/// Builds the segment's outline (a rectangle, or a pill for round caps) in
/// absolute coordinates and fills it via the scanline tessellator.
fn stroke_line_segment_crisp(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    half_w: f32,
    color: ColorF,
    line_cap: LineCap,
    ctx: &mut RenderContext,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < ZERO_LENGTH_EPSILON {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    let pts: Vec<LyonPoint> = match line_cap {
        LineCap::Butt => vec![
            LyonPoint::new(x1 + nx * half_w, y1 + ny * half_w),
            LyonPoint::new(x1 - nx * half_w, y1 - ny * half_w),
            LyonPoint::new(x2 - nx * half_w, y2 - ny * half_w),
            LyonPoint::new(x2 + nx * half_w, y2 + ny * half_w),
        ],
        LineCap::Square => {
            let ex = ux * half_w;
            let ey = uy * half_w;
            vec![
                LyonPoint::new(x1 - ex + nx * half_w, y1 - ey + ny * half_w),
                LyonPoint::new(x1 - ex - nx * half_w, y1 - ey - ny * half_w),
                LyonPoint::new(x2 + ex - nx * half_w, y2 + ey - ny * half_w),
                LyonPoint::new(x2 + ex + nx * half_w, y2 + ey + ny * half_w),
            ]
        },
        LineCap::Round => {
            // A stadium (pill): semicircular cap around each endpoint joined by
            // the two straight sides (the sides are implied by the closed
            // contour). `a0` is the perpendicular (normal) direction.
            let a0 = uy.atan2(ux) + std::f32::consts::FRAC_PI_2;
            let mut v = Vec::with_capacity(2 * (CRISP_ARC_SEGMENTS + 1));
            for k in 0..=CRISP_ARC_SEGMENTS {
                let a = a0 + std::f32::consts::PI * (k as f32 / CRISP_ARC_SEGMENTS as f32);
                v.push(LyonPoint::new(x1 + half_w * a.cos(), y1 + half_w * a.sin()));
            }
            for k in 0..=CRISP_ARC_SEGMENTS {
                let a = a0 + std::f32::consts::PI +
                    std::f32::consts::PI * (k as f32 / CRISP_ARC_SEGMENTS as f32);
                v.push(LyonPoint::new(x2 + half_w * a.cos(), y2 + half_w * a.sin()));
            }
            v
        },
    };

    let fill = FillStyle::Solid(color);
    tessellate_contours(&[&pts], FillRule::NonZero, &fill, ctx);
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
