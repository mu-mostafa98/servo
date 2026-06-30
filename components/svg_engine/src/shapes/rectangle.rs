/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;
use style::values::computed::length::Size;
use style::values::generics::length::GenericLengthPercentageOrAuto;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::shapes::attr_parsers::parse_length;
use crate::style::FromComputedValues;

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

impl FromComputedValues for Rectangle {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        let svg = values.get_svg();
        let pos = values.get_position();

        let x = svg.x.to_length().map(|l| l.px()).unwrap_or(0.0);
        let y = svg.y.to_length().map(|l| l.px()).unwrap_or(0.0);

        let width = match &pos.width {
            Size::LengthPercentage(w) => w.0.to_length().map(|l| l.px()).unwrap_or(0.0),
            _ => 0.0,
        };
        let height = match &pos.height {
            Size::LengthPercentage(h) => h.0.to_length().map(|l| l.px()).unwrap_or(0.0),
            _ => 0.0,
        };

        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        let rx = match &svg.rx {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px())
            },
            _ => None,
        };
        let ry = match &svg.ry {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px())
            },
            _ => None,
        };

        Some(Rectangle { x, y, width, height, rx, ry })
    }
}

impl Build for Rectangle {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        // Prefer CSS computed values (SVG 2 spec).
        if let Some(cv) = input.computed_values {
            if let Some(rect) = Self::from_computed_values(cv) {
                return Ok(rect);
            }
        }
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
