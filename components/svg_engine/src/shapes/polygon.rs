/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::attr_parsers::parse_points;

/// SVG `<polygon>` element — a closed shape formed by connected line segments.
#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point>,
}

impl Build for Polygon {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        parse_points(&input.get_attr).map(|points| Polygon { points })
    }
}
