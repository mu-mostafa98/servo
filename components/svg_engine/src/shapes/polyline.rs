/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;
// TODO: will be restored in future PRs
// use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
// use crate::render_tree::ClipPathUnits;
// use crate::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE};

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

// TODO: clip_info() and clip_points() will be restored in future PRs
//
// impl Polyline {
//     pub(crate) fn clip_info(
//         &self,
//         svg_origin: &LayoutPoint,
//         units: ClipPathUnits,
//     ) -> Option<ClipGeometry> {
//         clip_points(&self.points, svg_origin, units)
//     }
// }
//
// pub(crate) fn clip_points(
//     pts: &[Point],
//     svg_origin: &LayoutPoint,
//     units: ClipPathUnits,
// ) -> Option<ClipGeometry> {
//     ...
// }
