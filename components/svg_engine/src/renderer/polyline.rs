/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Transform2D;
use kurbo::{BezPath, Point as KurboPoint};
use lyon::math::Point as LyonPoint;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::renderer::{Render, RenderContext, fill, paint_order_stroke_before_fill, stroke};
use crate::renderer::path::rasterize_bez;
use crate::shapes::Polyline;
use crate::style::FillRule;

/// Renders an SVG `<polyline>`.
///
/// - Default: vello_cpu rasterization (solid or gradient).
/// - `native_rendering` (pattern content): WebRender primitives so the
///   shape respects reference frames and is tiled correctly.
impl Render for Polyline {
    fn render(&self, ctx: &mut RenderContext) {
        let points = &self.points;
        if points.len() < 2 {
            return;
        }

        if ctx.native_rendering {
            // SVG polylines are implicitly closed for filling.
            render_native_polyline(points, ctx, true);
            return;
        }

        let bez = points_to_bez(points, false);
        rasterize_bez(
            &bez,
            ctx.style.fill.as_ref(),
            ctx.style.stroke.as_ref(),
            ctx.style.opacity,
            &ctx.svg_origin,
            ctx.viewbox_scale,
            Transform2D::identity(),
            None,
            ctx.paints,
            ctx.rasters,
        );
    }
}

/// Native polyline rendering via WebRender primitives (tessellated fill +
/// per-segment stroke), respecting reference frames.
///
/// `fill_enabled` controls whether the polygon is filled (open paths with no
/// closed subpath are stroke-only).
pub(crate) fn render_native_polyline(
    points: &[KurboPoint],
    ctx: &mut RenderContext,
    fill_enabled: bool,
) {
    let fill_rule = ctx
        .style
        .fill
        .as_ref()
        .map(|f| f.fill_rule)
        .unwrap_or(FillRule::NonZero);
    let stroke_before_fill = paint_order_stroke_before_fill(ctx);
    let has_stroke = ctx.style.stroke.is_some();
    let has_fill = fill_enabled && ctx.style.fill.is_some();

    if stroke_before_fill {
        if has_stroke {
            render_native_stroke(points, ctx);
        }
        if has_fill {
            render_native_fill(points, ctx, fill_rule);
        }
    } else {
        if has_fill {
            render_native_fill(points, ctx, fill_rule);
        }
        if has_stroke {
            render_native_stroke(points, ctx);
        }
    }
}

/// Stroke a single polyline via WebRender primitives (per-segment).
pub(crate) fn render_native_stroke(points: &[KurboPoint], ctx: &mut RenderContext) {
    let stroke_pts: Vec<LyonPoint> = points
        .iter()
        .map(|p| LyonPoint::new(p.x as f32, p.y as f32))
        .collect();
    stroke::stroke_polyline(&stroke_pts, ctx);
}

/// Fill a single polygon via WebRender primitives (tessellated).
pub(crate) fn render_native_fill(
    points: &[KurboPoint],
    ctx: &mut RenderContext,
    fill_rule: FillRule,
) {
    let shifted_pts: Vec<LyonPoint> = points
        .iter()
        .map(|p| LyonPoint::new(ctx.svg_origin.x + p.x as f32, ctx.svg_origin.y + p.y as f32))
        .collect();
    if shifted_pts.len() >= 3 {
        let (bx, by, bw, bh) = fill::points_bounds(&shifted_pts);
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bx, by),
            LayoutSize::new(bw, bh),
        );
        fill::fill_polygon(&shifted_pts, bounds, fill_rule, ctx);
    }
}

/// Build an open or closed [`BezPath`] from a list of points.
pub(crate) fn points_to_bez(points: &[KurboPoint], close: bool) -> BezPath {
    let mut bez = BezPath::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            bez.move_to((p.x, p.y));
        } else {
            bez.line_to((p.x, p.y));
        }
    }
    if close {
        bez.close_path();
    }
    bez
}
