/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::error::SvgResult;
use crate::extract::{Build, SvgBuildInput};
use crate::shapes::parse_length;

/// SVG `<circle>` element.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl Build for Circle {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let r = parse_length("r", &input.get_attr)?;
        if r < 0.0 {
            return Err(crate::error::SvgEngineError::ParseError(
                "negative radius on <circle>".to_owned(),
            ));
        }

        Ok(Circle {
            cx: parse_length("cx", &input.get_attr).unwrap_or(0.0),
            cy: parse_length("cy", &input.get_attr).unwrap_or(0.0),
            r,
        })
    }
}
