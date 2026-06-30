/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::parse_length;

/// SVG `<ellipse>` element.
#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

impl Build for Ellipse {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let rx = parse_length("rx", &input.get_attr)?;
        if rx < 0.0 {
            return Err(crate::error::SvgEngineError::ParseError(
                "negative rx on <ellipse>".to_owned(),
            ));
        }
        let ry = parse_length("ry", &input.get_attr)?;
        if ry < 0.0 {
            return Err(crate::error::SvgEngineError::ParseError(
                "negative ry on <ellipse>".to_owned(),
            ));
        }

        Ok(Ellipse {
            cx: parse_length("cx", &input.get_attr).unwrap_or(0.0),
            cy: parse_length("cy", &input.get_attr).unwrap_or(0.0),
            rx,
            ry,
        })
    }
}
