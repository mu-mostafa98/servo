/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::attr_parsers::parse_length;

/// SVG `<line>` element.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Build for Line {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        Ok(Line {
            x1: parse_length("x1", &input.get_attr).unwrap_or(0.0),
            y1: parse_length("y1", &input.get_attr).unwrap_or(0.0),
            x2: parse_length("x2", &input.get_attr).unwrap_or(0.0),
            y2: parse_length("y2", &input.get_attr).unwrap_or(0.0),
        })
    }
}
