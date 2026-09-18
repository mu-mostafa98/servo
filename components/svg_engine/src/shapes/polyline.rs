/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;
use webrender_api::units::LayoutPoint;

use crate::render_tree::ClipPathUnits;
use crate::shapes::{ClipGeometry, clip_path_geometry, points_to_bez};

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

impl Polyline {
    /// Clip geometry for this polyline.  A clip path fills the polyline as if
    /// it were closed (per SVG 2), so the mask path is closed for filling.
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        if self.points.len() < 3 {
            return None;
        }
        Some(clip_path_geometry(
            &points_to_bez(&self.points, true),
            svg_origin,
            units,
        ))
    }
}
