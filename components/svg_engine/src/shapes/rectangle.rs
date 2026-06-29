/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::{FromAttributes, parse_length};

/// SVG `<rect>` element.
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rx: Option<f32>,
    pub ry: Option<f32>,
}

impl FromAttributes for Rectangle {
    fn from_attributes(_name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        Some(Rectangle {
            x: parse_length("x", get_attr)?,
            y: parse_length("y", get_attr)?,
            width: parse_length("width", get_attr)?,
            height: parse_length("height", get_attr)?,
            rx: parse_length("rx", get_attr),
            ry: parse_length("ry", get_attr),
        })
    }
}
