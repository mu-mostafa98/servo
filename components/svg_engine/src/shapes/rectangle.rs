/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::error::SvgResult;
use crate::extract::{Extract, SvgExtractInput};
use crate::shapes::parse_length;

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

impl Extract for Rectangle {
    fn extract(input: &SvgExtractInput) -> SvgResult<Self> {
        let width = parse_length("width", &input.get_attr)?;
        if width < 0.0 {
            return Err(crate::error::SvgEngineError::ParseError(
                "negative width on <rect>".to_owned(),
            ));
        }
        let height = parse_length("height", &input.get_attr)?;
        if height < 0.0 {
            return Err(crate::error::SvgEngineError::ParseError(
                "negative height on <rect>".to_owned(),
            ));
        }

        Ok(Rectangle {
            x: parse_length("x", &input.get_attr).unwrap_or(0.0),
            y: parse_length("y", &input.get_attr).unwrap_or(0.0),
            width,
            height,
            rx: parse_length("rx", &input.get_attr).ok(),
            ry: parse_length("ry", &input.get_attr).ok(),
        })
    }
}
