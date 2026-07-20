/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Unified fill pipeline — dispatches solid / gradient / pattern fills
//! for both axis-aligned rectangles and arbitrary polygons.
//!
//! **Single responsibility:** given a fill style from [`RenderContext`],
//! fill a region of the display list.  Every shape renderer delegates
//! its fill work here, eliminating the duplicated match-on-`PaintServer`
//! pattern that previously lived in each shape's `Render` impl.

use lyon::math::Point as LyonPoint;
use webrender_api::units::LayoutRect;
use webrender_api::{ClipChainId, CommonItemProperties, SpaceAndClipInfo};

use crate::render_tree::PatternUnits;
use crate::renderer::{RenderContext, gradient, pattern, to_colorf};
use crate::style::gradient::{GradientDef, GradientUnits, PaintServer};
use crate::style::hints::ColorInterpolation;
use crate::tessellator;
use crate::tessellator::FillStyle;

// ======================= Rect fill =======================

/// Fill an axis-aligned rectangle using the current style's fill properties.
///
/// `clip` is the clip chain to apply (may differ from `ctx.clip_chain_id`
/// when the caller has a rounded-rect clip for corner radii).
pub(crate) fn fill_rect(bounds: LayoutRect, clip: ClipChainId, ctx: &mut RenderContext) {
    let Some(fill) = &ctx.style.fill else { return };
    let opacity = fill.opacity * ctx.style.opacity;

    // Use the shape's clip chain so gradient/pattern fills respect
    // rounded-rect clips (needed for circles/ellipses with corners).
    let orig_clip = ctx.clip_chain_id;
    ctx.clip_chain_id = clip;

    match &fill.paint_server {
        Some(PaintServer::Gradient(id)) => {
            // Guard: sometimes a pattern ID overlaps a gradient ID in the maps.
            if ctx.paints.has_pattern(id) {
                pattern::fill_rect_with_pattern_by_id(id, bounds, ctx, opacity);
            } else {
                gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, opacity);
            }
        },
        Some(PaintServer::Pattern(id)) => {
            pattern::fill_rect_with_pattern_by_id(id, bounds, ctx, opacity);
        },
        Some(PaintServer::Solid(_)) => {
            // Solid paint server — handled via fill.color below.
        },
        None => {
            if let Some(svg_color) = fill.color {
                let mut color = to_colorf(&svg_color);
                color.a *= opacity;
                let common = CommonItemProperties::new(
                    bounds,
                    SpaceAndClipInfo {
                        spatial_id: ctx.spatial_id,
                        clip_chain_id: clip,
                    },
                );
                ctx.wr.push_rect(&common, bounds, color);
            }
        },
    }

    ctx.clip_chain_id = orig_clip;
}

// ======================= Polygon fill =======================

/// Fill a polygon (in layout-space coordinates) using the current style's
/// fill properties.
///
/// `pts` must already be shifted into the WebRender layout coordinate
/// space (i.e. origin-adjusted).  `bounds` is the axis-aligned bounding
/// box of `pts`, used for gradient/pattern fills that need a bounding
/// region.
pub(crate) fn fill_polygon(
    pts: &[LyonPoint],
    bounds: LayoutRect,
    fill_rule: crate::style::FillRule,
    ctx: &mut RenderContext,
) {
    let Some(fill) = &ctx.style.fill else { return };
    let opacity = fill.opacity * ctx.style.opacity;
    let bx = bounds.min.x;
    let by = bounds.min.y;
    let bw = bounds.size().width.max(1.0);
    let bh = bounds.size().height.max(1.0);

    match &fill.paint_server {
        Some(PaintServer::Gradient(id)) => {
            if ctx.paints.has_pattern(id) {
                handle_pattern_fill(id, pts, bounds, fill_rule, ctx, opacity);
                return;
            }
            if let Some(grad_def) = ctx.paints.gradient(id) {
                match grad_def {
                    GradientDef::Linear(lg) => {
                        let (gx1, gy1, gx2, gy2) =
                            resolve_linear_gradient_coords(lg, bx, by, bw, bh, ctx);
                        let fill_style =
                            build_linear_fill_style(lg, gx1, gy1, gx2, gy2, opacity, ctx);
                        tessellator::tessellate_polygon(pts, fill_rule, &fill_style, ctx);
                    },
                    GradientDef::Radial(rg) => {
                        let (fx, fy, r2) = resolve_radial_gradient_coords(rg, bx, by, bw, bh, ctx);
                        let fill_style = build_radial_fill_style(rg, fx, fy, r2, opacity, ctx);
                        tessellator::tessellate_polygon(pts, fill_rule, &fill_style, ctx);
                    },
                }
            }
        },
        Some(PaintServer::Pattern(id)) => {
            handle_pattern_fill(id, pts, bounds, fill_rule, ctx, opacity);
        },
        _ => {
            if let Some(svg_color) = fill.color {
                let mut color = to_colorf(&svg_color);
                color.a *= opacity;
                let fill_style = FillStyle::Solid(color);
                tessellator::tessellate_polygon(pts, fill_rule, &fill_style, ctx);
            }
        },
    }
}

// ======================= Gradient Coordinate Resolution =======================

/// Convert a linear gradient's endpoints to absolute layout coordinates.
fn resolve_linear_gradient_coords(
    lg: &crate::style::gradient::LinearGradient,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    ctx: &RenderContext,
) -> (f32, f32, f32, f32) {
    match lg.units {
        GradientUnits::ObjectBoundingBox => (
            bx + lg.x1.to_object_bbox() * bw,
            by + lg.y1.to_object_bbox() * bh,
            bx + lg.x2.to_object_bbox() * bw,
            by + lg.y2.to_object_bbox() * bh,
        ),
        GradientUnits::UserSpaceOnUse => (
            ctx.svg_origin.x + lg.x1.to_user_space(bw),
            ctx.svg_origin.y + lg.y1.to_user_space(bh),
            ctx.svg_origin.x + lg.x2.to_user_space(bw),
            ctx.svg_origin.y + lg.y2.to_user_space(bh),
        ),
    }
}

/// Convert a radial gradient's focal point and radius to absolute layout coordinates.
/// Returns `(fx, fy, radius)`.  The caller must square the radius for the tessellator.
fn resolve_radial_gradient_coords(
    rg: &crate::style::gradient::RadialGradient,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    ctx: &RenderContext,
) -> (f32, f32, f32) {
    let scale = bw.max(bh);
    match rg.units {
        GradientUnits::ObjectBoundingBox => (
            bx + rg.fx.to_object_bbox() * bw,
            by + rg.fy.to_object_bbox() * bh,
            (rg.r.to_object_bbox() * scale).max(1.0),
        ),
        GradientUnits::UserSpaceOnUse => (
            ctx.svg_origin.x + rg.fx.to_user_space(bw),
            ctx.svg_origin.y + rg.fy.to_user_space(bh),
            rg.r.to_user_space(scale).max(1.0),
        ),
    }
}

/// Extract the `color-interpolation` hint from the render context.
fn color_interpolation_hint(ctx: &RenderContext) -> ColorInterpolation {
    ctx.style
        .render_hints
        .as_ref()
        .and_then(|h| h.color_interpolation)
        .unwrap_or(ColorInterpolation::Srgb)
}

/// Build a [`FillStyle::LinearGradient`] from resolved coordinates.
fn build_linear_fill_style<'a>(
    lg: &'a crate::style::gradient::LinearGradient,
    gx1: f32,
    gy1: f32,
    gx2: f32,
    gy2: f32,
    opacity: f32,
    ctx: &RenderContext,
) -> FillStyle<'a> {
    FillStyle::LinearGradient {
        stops: &lg.stops,
        gx1,
        gy1,
        gx2,
        gy2,
        opacity,
        color_interpolation: color_interpolation_hint(ctx),
        spread_method: lg.spread_method,
    }
}

/// Build a [`FillStyle::RadialGradient`] from resolved coordinates.
fn build_radial_fill_style<'a>(
    rg: &'a crate::style::gradient::RadialGradient,
    fx: f32,
    fy: f32,
    radius: f32,
    opacity: f32,
    ctx: &RenderContext,
) -> FillStyle<'a> {
    FillStyle::RadialGradient {
        stops: &rg.stops,
        fx,
        fy,
        r2: radius * radius,
        opacity,
        color_interpolation: color_interpolation_hint(ctx),
        spread_method: rg.spread_method,
    }
}

/// Helper: fill a polygon with a pattern paint server.
fn handle_pattern_fill(
    id: &str,
    pts: &[LyonPoint],
    bounds: LayoutRect,
    fill_rule: crate::style::FillRule,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    let def = match ctx.paints.pattern(id) {
        Some(d) => d,
        None => {
            log::warn!("SVG pattern \"{}\" not found in definitions", id);
            return;
        },
    };

    if def.shapes.is_empty() {
        return;
    }

    let bw = bounds.size().width.max(1.0);
    let bh = bounds.size().height.max(1.0);
    let bx = bounds.min.x;
    let by = bounds.min.y;

    let (tile_w, tile_h) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (def.width * bw, def.height * bh),
        PatternUnits::UserSpaceOnUse => (def.width, def.height),
    };

    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }

    let (ox, oy) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (bx + def.x * bw, by + def.y * bh),
        PatternUnits::UserSpaceOnUse => {
            // Per SVG spec, pattern x/y are in user space (the SVG viewport
            // coordinate system), not relative to the element being filled.
            // Convert from SVG user space to document layout space by adding
            // the SVG viewport origin.
            (ctx.svg_origin.x + def.x, ctx.svg_origin.y + def.y)
        },
    };

    let fill_style = FillStyle::Pattern {
        shapes: &def.shapes,
        tile_w,
        tile_h,
        ox,
        oy,
        opacity,
    };
    tessellator::tessellate_polygon(pts, fill_rule, &fill_style, ctx);
}

// ======================= Utility =======================

/// Compute the axis-aligned bounding box of a set of lyon [`Point`]s.
///
/// Returns `(min_x, min_y, width, height)`.
pub(crate) fn points_bounds(pts: &[LyonPoint]) -> (f32, f32, f32, f32) {
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
    (min_x, min_y, max_x - min_x, max_y - min_y)
}
