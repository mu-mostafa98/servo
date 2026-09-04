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
use crate::RasterizedImage;

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
    /// The viewBox → viewport scale factor, used to rasterize paths at the
    /// correct resolution. `(1.0, 1.0)` when there is no viewBox (or it is
    /// identity).
    pub viewbox_scale: (f32, f32),
    /// The device pixel ratio (page zoom × hidpi), used to rasterize CPU
    /// shapes at device resolution so the compositor downscales rather than
    /// upscales the resulting bitmap.
    pub device_scale: f32,
    /// Accumulated translation of nested viewBox frames, in the root user
    /// space. CPU-rasterized shapes (vello_cpu) bypass reference frames, so
    /// they need this explicit offset folded into their raster position.
    pub raster_offset: LayoutPoint,
    /// When true, shapes are rendered via native WebRender primitives
    /// (respecting reference frames) rather than vello_cpu rasterization.
    /// Used for pattern content, which must be tiled correctly.
    pub native_rendering: bool,
    /// CPU-rasterized images collected during rendering. Shape `Render` impls
    /// that rasterize via vello_cpu push their output here; the layout layer
    /// uploads and pushes them as WebRender images after traversal.
    pub rasters: &'a mut Vec<RasterizedImage>,
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
