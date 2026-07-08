/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

impl crate::shapes::BuildFromElement for Polyline {
    fn from_attrs(_font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_points;
        parse_points(&|a| attrs.get_attr(a)).ok().map(|pts| Polyline { points: pts })
    }
}
