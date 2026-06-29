/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::{FromAttributes, parse_length};

/// SVG `<ellipse>` element.
#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

impl FromAttributes for Ellipse {
    fn from_attributes(_name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        Some(Ellipse {
            cx: parse_length("cx", get_attr)?,
            cy: parse_length("cy", get_attr)?,
            rx: parse_length("rx", get_attr)?,
            ry: parse_length("ry", get_attr)?,
        })
    }
}
