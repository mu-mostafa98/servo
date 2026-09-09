/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{BorderRadius, ClipMode, ComplexClipRegion};

use crate::renderer::{
    Render, RenderContext, clip_chain_option, fill, paint_order_stroke_before_fill, stroke,
};
use crate::shapes::{Ellipse, Rectangle, Shape};

/// Compute the layout-space bounds and corner radii for an axis-aligned
/// rect/circle/ellipse. Returns `None` for shapes that are not one of those
/// three, or for a degenerate circle/ellipse (non-positive radius).
///
/// Shared by [`Rectangle::render`] and the top-level native-gradient routing in
/// `traversal.rs`, which needs the bounds to resolve a gradient's geometry
/// before deciding whether it can be rendered natively.
pub(crate) fn rect_bounds_and_radii(
    shape: &Shape,
    svg_origin: LayoutPoint,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    match shape {
        Shape::Rect(r) => {
            let bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(svg_origin.x + r.x, svg_origin.y + r.y),
                LayoutSize::new(r.width, r.height),
            );
            let rx = r.rx.or(r.ry).unwrap_or(0.0).clamp(0.0, r.width / 2.0);
            let ry = r.ry.or(r.rx).unwrap_or(0.0).clamp(0.0, r.height / 2.0);
            let has_radius = rx > 0.0 || ry > 0.0;
            let radii = has_radius.then(|| BorderRadius {
                top_left: LayoutSize::new(rx, ry),
                top_right: LayoutSize::new(rx, ry),
                bottom_left: LayoutSize::new(rx, ry),
                bottom_right: LayoutSize::new(rx, ry),
            });
            Some((bounds, radii))
        },
        Shape::Circle(c) => rect_bounds_and_radii(
            &Shape::Ellipse(Ellipse {
                cx: c.cx,
                cy: c.cy,
                rx: c.r,
                ry: c.r,
            }),
            svg_origin,
        ),
        Shape::Ellipse(e) => {
            if e.rx <= 0.0 || e.ry <= 0.0 {
                return None;
            }
            rect_bounds_and_radii(
                &Shape::Rect(Rectangle {
                    x: e.cx - e.rx,
                    y: e.cy - e.ry,
                    width: e.rx * 2.0,
                    height: e.ry * 2.0,
                    rx: Some(e.rx),
                    ry: Some(e.ry),
                }),
                svg_origin,
            )
        },
        _ => None,
    }
}

/// Renders an SVG `<rect>`.
///
/// LSP contract:
/// - Fills if `ctx.style.fill` is `Some` (via [`fill::fill_rect`]).
/// - Strokes if `ctx.style.stroke` is `Some` (via [`stroke::stroke_rect`]).
/// - Respects `rx`/`ry` corner radii for both fill clip and stroke border.
/// - Delegates gradient/pattern paint server lookup to paint helpers.
impl Render for Rectangle {
    fn render(&self, ctx: &mut RenderContext) {
        let Some((bounds, radii)) =
            rect_bounds_and_radii(&Shape::Rect(*self), ctx.svg_origin)
        else {
            return;
        };

        // Build a clip chain for rounded corners.
        let clip = if let Some(r) = radii {
            let clip_id = ctx.wr.define_clip_rounded_rect(
                ctx.spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii: r,
                    mode: ClipMode::Clip,
                },
            );
            let parent = clip_chain_option(ctx.clip_chain_id);
            ctx.wr.define_clip_chain(parent, [clip_id])
        } else {
            ctx.clip_chain_id
        };

        if paint_order_stroke_before_fill(ctx) {
            stroke::stroke_rect(bounds, radii, ctx);
            fill::fill_rect(bounds, clip, ctx);
        } else {
            fill::fill_rect(bounds, clip, ctx);
            stroke::stroke_rect(bounds, radii, ctx);
        }
    }
}
