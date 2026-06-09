/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{ DisplayListBuilder, ClipChainId, SpatialId, units::LayoutPoint };

use crate::shapes::{ Ellipse, Rectangle };
use crate::styles::*;

use super::rect::render_rect;

pub fn render_ellipse(
    ellipse: &Ellipse,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    if ellipse.rx <= 0.0 || ellipse.ry <= 0.0 {
        return;
    }

    let rect = Rectangle {
        x: ellipse.cx - ellipse.rx,
        y: ellipse.cy - ellipse.ry,
        width: ellipse.rx * 2.0,
        height: ellipse.ry * 2.0,
        rx: Some(ellipse.rx),
        ry: Some(ellipse.ry),
    };
    render_rect(&rect, style, svg_origin, spatial_id, clip_chain_id, wr);
}
