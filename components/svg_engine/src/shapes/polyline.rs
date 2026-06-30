/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::parse_points;

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

impl Build for Polyline {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        parse_points(&input.get_attr).map(|points| Polyline { points })
    }
}
