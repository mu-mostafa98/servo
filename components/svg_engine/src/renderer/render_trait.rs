/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The [`Render`] trait and [`RenderContext`] — the core rendering
//! interface for SVG shapes.

use webrender_api::units::LayoutPoint;
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};

use crate::renderer::providers::PaintResourceProvider;
use crate::shapes::Shape;
use crate::style::NodeStyle;

/// Bundled rendering parameters passed to every [`Render::render`] call.
pub(crate) struct RenderContext<'a> {
    pub style: &'a NodeStyle,
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
    /// Paint resource provider, used internally by fill/stroke helpers.
    /// Shape `Render` impls should NOT access this field directly.
    /// Instead, call `fill::fill_rect(…)` or `stroke::stroke_rect(…)`
    /// which internally use this field to look up paint servers.
    pub paints: &'a dyn PaintResourceProvider,
    /// Accumulated transform scale from all ancestor transforms.
    /// Used by `vector-effect: non-scaling-stroke` to compensate stroke width.
    pub accumulated_scale: f32,
}

/// Convert an SVG shape into WebRender display list commands.
///
/// Every SVG shape type implements this trait so that traversal
/// code can call `shape.render(...)` without a central match.
pub(crate) trait Render {
    /// Emit WebRender display list commands for this shape.
    fn render(&self, ctx: &mut RenderContext);
}

// ----------------------- Shape Dispatch -----------------------

impl Render for Shape {
    fn render(&self, ctx: &mut RenderContext) {
        match self {
            Shape::Rect(r) => r.render(ctx),
            Shape::Circle(c) => c.render(ctx),
            Shape::Ellipse(e) => e.render(ctx),
            Shape::Line(l) => l.render(ctx),
            Shape::Polyline(p) => p.render(ctx),
            Shape::Polygon(p) => p.render(ctx),
            Shape::Path(p) => p.render(ctx),
        }
    }
}
