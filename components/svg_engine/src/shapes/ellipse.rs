/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;
use style::values::generics::length::GenericLengthPercentageOrAuto;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::attr_parsers::parse_length;
use crate::style::FromComputedValues;

/// SVG `<ellipse>` element.
#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

impl FromComputedValues for Ellipse {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        let svg = values.get_svg();
        let cx = svg.cx.to_length().map(|l| l.px()).unwrap_or(0.0);
        let cy = svg.cy.to_length().map(|l| l.px()).unwrap_or(0.0);
        let rx = match &svg.rx {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px())
            },
            _ => None,
        }.unwrap_or(0.0);
        let ry = match &svg.ry {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px())
            },
            _ => None,
        }.unwrap_or(0.0);
        if rx <= 0.0 || ry <= 0.0 {
            return None;
        }
        Some(Ellipse { cx, cy, rx, ry })
    }
}

impl Build for Ellipse {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        // Prefer CSS computed values (SVG 2 spec).
        if let Some(cv) = input.computed_values {
            if let Some(ellipse) = Self::from_computed_values(cv) {
                return Ok(ellipse);
            }
        }
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
