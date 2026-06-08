/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, SpaceAndClipInfo,
    BorderRadius, ClipMode, ComplexClipRegion,
    units::{LayoutPoint, LayoutRect, LayoutSize}
};

use crate::shapes::Ellipse;
use crate::styles::*;

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

    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            svg_origin.x + ellipse.cx - ellipse.rx,
            svg_origin.y + ellipse.cy - ellipse.ry,
        ),
        LayoutSize::new(ellipse.rx * 2.0, ellipse.ry * 2.0),
    );

    if let Some(fill) = &style.fill {
        if let Some(color) = fill.color {
            let clip_id = wr.define_clip_rounded_rect(
                spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii: BorderRadius {
                        top_left: LayoutSize::new(ellipse.rx, ellipse.ry),
                        top_right: LayoutSize::new(ellipse.rx, ellipse.ry),
                        bottom_left: LayoutSize::new(ellipse.rx, ellipse.ry),
                        bottom_right: LayoutSize::new(ellipse.rx, ellipse.ry),
                    },
                    mode: ClipMode::Clip,
                },
            );

            let parent = match clip_chain_id {
                ClipChainId::INVALID => None,
                id => Some(id),
            };

            let ellipse_clip_chain_id = wr.define_clip_chain(
                parent,
                [clip_id],
            );     

            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo { spatial_id, clip_chain_id: ellipse_clip_chain_id },
            );
            wr.push_rect(&common, bounds, color);
        }
    }
}
