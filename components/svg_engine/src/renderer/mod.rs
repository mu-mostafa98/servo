/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod fill;
pub(crate) mod rect;

use svgtypes::Color as SvgColor;
use webrender_api::units::{LayoutPoint, LayoutRect};
use webrender_api::{
    ClipChainId, ColorF, CommonItemProperties, DisplayListBuilder, SpaceAndClipInfo, SpatialId,
};

use crate::shapes::*;
use crate::style::NodeStyle;

pub(crate) struct RenderContext<'a> {
    pub style: &'a NodeStyle,
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
}

pub(crate) trait Render {
    fn render(&self, ctx: &mut RenderContext);
}

impl Render for Shape {
    fn render(&self, ctx: &mut RenderContext) {
        match self {
            Shape::Rect(r) => r.render(ctx),
            // Shape::Circle(c) => c.render(ctx),
            // Shape::Ellipse(e) => e.render(ctx),
            // Shape::Line(l) => l.render(ctx),
            // Shape::Polyline(p) => p.render(ctx),
            // Shape::Polygon(p) => p.render(ctx),
            // Shape::Path(p) => p.render(ctx),
        }
    }
}

// ----------------------- Shared Helpers -----------------------

pub(crate) fn make_common_props(
    bounds: LayoutRect,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) -> CommonItemProperties {
    CommonItemProperties::new(
        bounds,
        SpaceAndClipInfo {
            spatial_id,
            clip_chain_id,
        },
    )
}

pub(crate) fn to_colorf(c: &SvgColor) -> ColorF {
    ColorF::new(
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
        c.alpha as f32 / 255.0,
    )
}
