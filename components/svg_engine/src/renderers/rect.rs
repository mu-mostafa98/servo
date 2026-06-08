/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, SpaceAndClipInfo, BorderSide, BorderStyle,
    BorderDetails, NormalBorder, BorderRadius,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutSideOffsets}
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
        if let Some(mut color) = fill.color {
            color.a *= fill.opacity;
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo{ spatial_id, clip_chain_id }
            );
            wr.push_rect(&common, bounds, color);
        }
    }

    if let Some(stroke) = &style.stroke {
        if let Some(mut color) = stroke.color {
            color.a *= stroke.opacity;
            let widths = LayoutSideOffsets::new_all_same(stroke.width);
            let border_side = BorderSide { color, style: BorderStyle::Solid };
            let details = BorderDetails::Normal(NormalBorder {
                left: border_side.clone(),
                right: border_side.clone(),
                top: border_side.clone(),
                bottom: border_side,
                radius: BorderRadius::zero(),
                do_aa: true,
            });
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo{ spatial_id, clip_chain_id }
            );
            wr.push_border(&common, bounds, widths, details);
        }
    }
}