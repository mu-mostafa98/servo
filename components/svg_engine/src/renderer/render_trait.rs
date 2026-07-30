/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The [`Render`] trait and [`RenderContext`] — the core rendering
//! interface for SVG shapes.

use webrender_api::units::LayoutPoint;
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};

/// Bundled rendering parameters passed to every [`Render::render`] call.
pub(crate) struct RenderContext<'a> {
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
    /// Accumulated transform scale from all ancestor transforms.
    /// Used by `vector-effect: non-scaling-stroke` to compensate stroke width.
    #[allow(dead_code)]
    pub accumulated_scale: f32,
}

/// Convert an SVG shape into WebRender display list commands.
pub(crate) trait Render {
    /// Emit WebRender display list commands for this shape.
    fn render(&self, ctx: &mut RenderContext);
}

// ----------------------- SimpleShape Dispatch -----------------------

impl Render for usvg::SimpleShape {
    fn render(&self, ctx: &mut RenderContext) {
        use usvg::SimpleShapeKind;
        match self.kind() {
            SimpleShapeKind::Rect { x, y, width, height, rx, ry } =>
                crate::renderer::rect::render(self, *x, *y, *width, *height, rx.as_ref().copied(), ry.as_ref().copied(), ctx),
            SimpleShapeKind::Circle { cx, cy, r } =>
                crate::renderer::circle::render(self, *cx, *cy, *r, ctx),
            SimpleShapeKind::Ellipse { cx, cy, rx, ry } =>
                crate::renderer::ellipse::render(self, *cx, *cy, *rx, *ry, ctx),
            SimpleShapeKind::Line { x1, y1, x2, y2 } =>
                crate::renderer::line::render(self, *x1, *y1, *x2, *y2, ctx),
        }
    }
}
