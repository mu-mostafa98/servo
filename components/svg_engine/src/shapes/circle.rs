/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::attr_parsers::parse_length;
use crate::style::FromComputedValues;

/// SVG `<circle>` element.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl FromComputedValues for Circle {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        let svg = values.get_svg();
        let cx = svg.cx.to_length().map(|l| l.px()).unwrap_or(0.0);
        let cy = svg.cy.to_length().map(|l| l.px()).unwrap_or(0.0);
        let r = svg.r.0.to_length().map(|l| l.px()).unwrap_or(0.0);
        if r <= 0.0 {
            return None;
        }
        Some(Circle { cx, cy, r })
    }
}

impl Build for Circle {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        // Prefer CSS computed values (SVG 2 spec).
        // Fall back to raw attribute parsing when unavailable.
        if let Some(cv) = input.computed_values {
            if let Some(circle) = Self::from_computed_values(cv) {
                return Ok(circle);
            }
        }
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
