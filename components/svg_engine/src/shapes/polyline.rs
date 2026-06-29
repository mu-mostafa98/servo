/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;

use crate::shapes::{FromAttributes, parse_points};

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

impl FromAttributes for Polyline {
    fn from_attributes(_name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        parse_points(get_attr).map(|points| Polyline { points })
    }
}
