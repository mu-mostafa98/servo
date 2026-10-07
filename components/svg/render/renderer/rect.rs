/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering for SVG `<rect>` and the shared rect/circle/ellipse bounds helper.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{BorderRadius, ClipMode, ComplexClipRegion};

use crate::model::element::shape::Rectangle;
use crate::render::renderer::{
    Render, RenderContext, clip_chain_option, fill, paint_order_stroke_before_fill, stroke,
};

/// Compute the layout-space bounds and corner radii for an axis-aligned
/// [`Rectangle`]. Returns `None` for a rect with non-positive width/height.
///
/// Callers with a `Shape` must convert it first via `Shape::to_rect`, which maps
/// circle/ellipse to their bounding rectangle and returns `None` for
/// non-rectangular shapes and degenerate circles/ellipses.
///
/// Shared by [`Rectangle::render`] and the top-level native-gradient routing in
/// `traversal.rs`, which needs the bounds to resolve a gradient's geometry
/// before deciding whether it can be rendered natively.
pub(crate) fn rect_bounds_and_radii(
    rect: &Rectangle,
    svg_origin: LayoutPoint,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    // A rect with non-positive width/height is not rendered (SVG 2
    // "basic shapes" error handling), and a negative size would also
    // make `clamp(0.0, size / 2.0)` panic (min > max).
    if rect.width.get() <= 0.0 || rect.height.get() <= 0.0 {
        return None;
    }
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + rect.x.get(), svg_origin.y + rect.y.get()),
        LayoutSize::new(rect.width.get(), rect.height.get()),
    );
    let rx = rect
        .rx
        .or(rect.ry)
        .map(|v| v.get())
        .unwrap_or(0.0)
        .clamp(0.0, rect.width.get() / 2.0);
    let ry = rect
        .ry
        .or(rect.rx)
        .map(|v| v.get())
        .unwrap_or(0.0)
        .clamp(0.0, rect.height.get() / 2.0);
    let has_radius = rx > 0.0 || ry > 0.0;
    let radii = has_radius.then(|| BorderRadius {
        top_left: LayoutSize::new(rx, ry),
        top_right: LayoutSize::new(rx, ry),
        bottom_left: LayoutSize::new(rx, ry),
        bottom_right: LayoutSize::new(rx, ry),
    });
    Some((bounds, radii))
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
        let Some((bounds, radii)) = rect_bounds_and_radii(self, ctx.svg_origin) else {
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
