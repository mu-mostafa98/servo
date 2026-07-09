/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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

impl crate::shapes::BuildFromElement for Rectangle {
    fn from_attrs(font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_length;
        let w = parse_length("width", &|a| attrs.get_attr(a), font_size).ok()?;
        let h = parse_length("height", &|a| attrs.get_attr(a), font_size).ok()?;
        if w < 0.0 || h < 0.0 { return None; }
        Some(Rectangle {
            x: parse_length("x", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            y: parse_length("y", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            width: w,
            height: h,
            rx: parse_length("rx", &|a| attrs.get_attr(a), font_size).ok(),
            ry: parse_length("ry", &|a| attrs.get_attr(a), font_size).ok(),
        })
    }
}
