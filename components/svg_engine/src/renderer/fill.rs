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

use webrender_api::{
    ClipChainId, CommonItemProperties, SpaceAndClipInfo,
    units::LayoutRect,
};

use lyon::math::Point as LyonPoint;

use crate::renderer::{RenderContext, to_colorf};
use crate::renderer::gradient;
use crate::renderer::pattern;
use crate::style::gradient::{GradientDef, PaintServer};
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
                    SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: clip },
                );
                ctx.wr.push_rect(&common, bounds, color);
            }
        },
    }
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

    match &fill.paint_server {
        Some(PaintServer::Gradient(id)) => {
            if ctx.paints.has_pattern(id) {
                pattern::fill_rect_with_pattern_by_id(id, bounds, ctx, opacity);
            } else if let Some(grad_def) = ctx.paints.gradient(id) {
                match grad_def {
                    GradientDef::Linear(lg) => {
                        let bx = bounds.min.x;
                        let by = bounds.min.y;
                        let bw = bounds.size().width.max(1.0);
                        let bh = bounds.size().height.max(1.0);
                        let fill_style = FillStyle::LinearGradient {
                            stops: &lg.stops,
                            x1: lg.x1.to_object_bbox(),
                            y1: lg.y1.to_object_bbox(),
                            x2: lg.x2.to_object_bbox(),
                            y2: lg.y2.to_object_bbox(),
                            units: lg.units,
                            bx, by, bw, bh,
                            opacity,
                        };
                        tessellator::tessellate_polygon(pts, fill_rule, &fill_style, ctx);
                    },
                    GradientDef::Radial(_rg) => {
                        gradient::fill_rect_with_gradient_by_id(id, bounds, ctx, opacity);
                    },
                }
            }
        },
        Some(PaintServer::Pattern(id)) => {
            pattern::fill_rect_with_pattern_by_id(id, bounds, ctx, opacity);
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
        if p.x < min_x { min_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.x > max_x { max_x = p.x; }
        if p.y > max_y { max_y = p.y; }
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}
