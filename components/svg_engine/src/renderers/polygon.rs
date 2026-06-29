/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
};

use crate::shapes::Polyline;
use crate::styles::NodeStyle;

pub fn render_polygon(
    polygon: &crate::shapes::Polygon,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // A polygon is a closed shape: append the first point to the end so the
    // stroke renders an edge from the last point back to the first.
    // The fill is unaffected — define_clip_image_mask already treats the
    // vertices as a closed polygon regardless of duplication.
    let mut closed_points = polygon.points.clone();
    if let Some(first) = polygon.points.first() {
        closed_points.push(*first);
    }

    let polyline = Polyline { points: closed_points };
    super::render_polyline(&polyline, style, svg_origin, spatial_id, clip_chain_id, wr);
}
