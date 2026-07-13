/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;
use webrender_api::units::LayoutPoint;

use crate::render_tree::ClipPathUnits;
use crate::shapes::polyline::clip_points;
use crate::shapes::ClipGeometry;

/// SVG `<polygon>` element — a closed shape formed by connected line segments.
#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point>,
}

impl Polygon {
    /// Clip geometry for this polygon (bounding box around all points).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        clip_points(&self.points, svg_origin, units)
    }
}
