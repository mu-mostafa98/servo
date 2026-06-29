/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::{FromAttributes, parse_length};

/// SVG `<circle>` element.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl FromAttributes for Circle {
    fn from_attributes(_name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        Some(Circle {
            cx: parse_length("cx", get_attr)?,
            cy: parse_length("cy", get_attr)?,
            r: parse_length("r", get_attr)?,
        })
    }
}
