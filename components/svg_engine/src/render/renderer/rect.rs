/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{BorderRadius, ClipMode, ComplexClipRegion};

use crate::render::renderer::{
    Render, RenderContext, clip_chain_option, fill, paint_order_stroke_before_fill, stroke,
};
use crate::model::shapes::{Ellipse, Rectangle, Shape};
use crate::model::units::Length;

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
                LayoutPoint::new(svg_origin.x + r.x.get(), svg_origin.y + r.y.get()),
                LayoutSize::new(r.width.get(), r.height.get()),
            );
            let rx = r
                .rx
                .or(r.ry)
                .map(|v| v.get())
                .unwrap_or(0.0)
                .clamp(0.0, r.width.get() / 2.0);
            let ry = r
                .ry
                .or(r.rx)
                .map(|v| v.get())
                .unwrap_or(0.0)
                .clamp(0.0, r.height.get() / 2.0);
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
                rx: Some(c.r),
                ry: Some(c.r),
                path_length: c.path_length,
            }),
            svg_origin,
        ),
        Shape::Ellipse(e) => {
            let (rx, ry) = e.resolved_radii()?;
            if rx.get() <= 0.0 || ry.get() <= 0.0 {
                return None;
            }
            rect_bounds_and_radii(
                &Shape::Rect(Rectangle {
                    x: Length::new(e.cx.get() - rx.get()),
                    y: Length::new(e.cy.get() - ry.get()),
                    width: Length::new(rx.get() * 2.0),
                    height: Length::new(ry.get() * 2.0),
                    rx: Some(rx),
                    ry: Some(ry),
                    path_length: None,
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
