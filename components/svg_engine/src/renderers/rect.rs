/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, SpaceAndClipInfo,
    units::{LayoutPoint, LayoutRect, LayoutSize}
};

use crate::shapes::Rectangle;
use crate::styles::*;

pub fn render_rect(
    rect: &Rectangle,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + rect.x, svg_origin.y + rect.y),
        LayoutSize::new(rect.width, rect.height),
    );

    if let Some(fill) = &style.fill {
        if let Some(color) = fill.color {
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo{ spatial_id, clip_chain_id }
            );
            wr.push_rect(&common, bounds, color);
        }
    }
}