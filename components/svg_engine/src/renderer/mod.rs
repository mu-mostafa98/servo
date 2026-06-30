/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape renderers — convert SVG shapes into WebRender display list commands.
//!
//! Each shape in [`crate::shapes`] implements the [`Render`] trait, which
//! produces the corresponding [`webrender_api::DisplayListBuilder`] commands.
//! The [`crate::traversal`] module calls [`Render::render`] during SVG tree
//! traversal — there is no central dispatch match to maintain.

pub(crate) mod rect;
pub(crate) mod ellipse;
pub(crate) mod circle;
pub(crate) mod line;
pub(crate) mod polyline;
pub(crate) mod polygon;
pub(crate) mod path;
pub(crate) mod transform;

use webrender_api::{
    ClipChainId, CommonItemProperties, DisplayListBuilder, SpaceAndClipInfo, SpatialId,
    units::LayoutPoint, units::LayoutRect,
};

use crate::shapes::*;
use crate::style::NodeStyle;

// ----------------------- Render Context -----------------------

/// Bundled rendering parameters passed to every [`Render::render`] call.
///
/// Using a single context struct avoids repeatedly threading 5+ parameters
/// through the rendering pipeline and makes it easy to add new context
/// (e.g. an accumulated transform for hit-testing).
pub(crate) struct RenderContext<'a> {
    pub style: &'a NodeStyle,
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
}

// ----------------------- Render Trait -----------------------

/// Convert an SVG shape into WebRender display list commands.
///
/// Every SVG shape type implements this trait so that traversal
/// code can call `shape.render(...)` without a central match.
pub(crate) trait Render {
    /// Emit WebRender display list commands for this shape.
    fn render(&self, ctx: &mut RenderContext);
}

// ----------------------- Delegation -----------------------

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

// ----------------------- Shared Helpers -----------------------

/// Construct a [`CommonItemProperties`] from an origin-space rect and clip info.
///
/// This is a thin convenience wrapper used by multiple renderers to
/// avoid repeating the same `SpaceAndClipInfo` construction.
pub(crate) fn make_common_props(
    bounds: LayoutRect,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) -> CommonItemProperties {
    CommonItemProperties::new(bounds, SpaceAndClipInfo { spatial_id, clip_chain_id })
}
