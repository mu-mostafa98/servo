/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{DisplayListBuilder, ClipChainId, SpatialId, units::LayoutPoint};

use crate::shapes::{Ellipse, Rectangle};
use crate::style::*;
use crate::renderer::Render;

impl Render for Ellipse {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        if self.rx <= 0.0 || self.ry <= 0.0 {
            return;
        }

        // An ellipse is rendered as a rounded rectangle with 100% corner radii.
        let rect = Rectangle {
            x: self.cx - self.rx,
            y: self.cy - self.ry,
            width: self.rx * 2.0,
            height: self.ry * 2.0,
            rx: Some(self.rx),
            ry: Some(self.ry),
        };
        rect.render(style, svg_origin, spatial_id, clip_chain_id, wr);
    }
}
