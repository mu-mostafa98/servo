/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{DisplayListBuilder, ClipChainId, SpatialId, units::LayoutPoint};

use crate::shapes::{Circle, Ellipse};
use crate::styles::*;
use crate::renderer::Render;

impl Render for Circle {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        // A circle is an ellipse with equal rx and ry.
        let ellipse = Ellipse {
            cx: self.cx,
            cy: self.cy,
            rx: self.r,
            ry: self.r,
        };
        ellipse.render(style, svg_origin, spatial_id, clip_chain_id, wr);
    }
}
