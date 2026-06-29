/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
};

use crate::shapes::{Polygon, Polyline};
use crate::styles::NodeStyle;
use crate::renderer::Render;

impl Render for Polygon {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        // A polygon is a closed shape: append the first point to the end so the
        // stroke renders an edge from the last point back to the first.
        // The fill is unaffected — the tessellator already treats vertices as
        // a closed polygon regardless of duplication.
        let mut closed_points = self.points.clone();
        if let Some(first) = self.points.first() {
            closed_points.push(*first);
        }

        let polyline = Polyline {
            points: closed_points,
        };
        polyline.render(style, svg_origin, spatial_id, clip_chain_id, wr);
    }
}
