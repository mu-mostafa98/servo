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

use webrender_api::{
    ClipChainId, CommonItemProperties, DisplayListBuilder, SpaceAndClipInfo, SpatialId,
    units::LayoutPoint, units::LayoutRect,
};

use crate::shapes::*;
use crate::styles::NodeStyle;

// ----------------------- Render Trait -----------------------

/// Convert an SVG shape into WebRender display list commands.
///
/// Every SVG shape type implements this trait so that traversal
/// code can call `shape.render(...)` without a central match.
pub(crate) trait Render {
    /// Emit WebRender display list commands for this shape.
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    );
}

// ----------------------- Delegation -----------------------

impl Render for Shape {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        match self {
            Shape::Rect(r) => r.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Circle(c) => c.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Ellipse(e) => e.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Line(l) => l.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Polyline(p) => p.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Polygon(p) => p.render(style, svg_origin, spatial_id, clip_chain_id, wr),
            Shape::Path(p) => p.render(style, svg_origin, spatial_id, clip_chain_id, wr),
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
