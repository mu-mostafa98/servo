/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::{FromAttributes, parse_length};

/// SVG `<line>` element.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl FromAttributes for Line {
    fn from_attributes(_name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        Some(Line {
            x1: parse_length("x1", get_attr)?,
            y1: parse_length("y1", get_attr)?,
            x2: parse_length("x2", get_attr)?,
            y2: parse_length("y2", get_attr)?,
        })
    }
}
