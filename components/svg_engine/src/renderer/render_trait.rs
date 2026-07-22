/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::LayoutPoint;
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};

use crate::renderer::providers::PaintResourceProvider;
use crate::shapes::Shape;
use crate::style::NodeStyle;

pub(crate) struct RenderContext<'a> {
    pub style: &'a NodeStyle,
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
    pub paints: &'a dyn PaintResourceProvider,
    pub accumulated_scale: f32,
}

pub(crate) trait Render {
    fn render(&self, ctx: &mut RenderContext);
}

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
