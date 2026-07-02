/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;
use style::values::computed::svg::SVGOpacity;
use webrender_api::ColorF;

use crate::style::color::{resolve_svg_paint, ResolvedPaint};
use crate::style::FromComputedValues;
use super::gradient::PaintServer;

/// SVG fill properties.
#[derive(Debug, Clone)]
pub struct FillParams {
    pub color: Option<ColorF>,
    /// Paint server reference (gradient url). When set, takes priority over `color`.
    pub paint_server: Option<PaintServer>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

/// SVG fill rule: determines how overlapping regions are filled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

// ----------------------- FromComputedValues -----------------------

impl FromComputedValues for FillParams {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.fill, values);
        let opacity = match inherited_svg.fill_opacity {
            SVGOpacity::Opacity(opacity) => opacity,
            _ => 1.0,
        };
        let fill_rule = match inherited_svg.fill_rule {
            style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
            style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
        };

        match paint {
            ResolvedPaint::Color(color) => {
                Some(FillParams {
                    color: Some(color),
                    paint_server: None,
                    opacity,
                    fill_rule,
                })
            },
            ResolvedPaint::PaintServer(id) => {
                Some(FillParams {
                    color: None,
                    paint_server: Some(PaintServer::Gradient(id)),
                    opacity,
                    fill_rule,
                })
            },
            ResolvedPaint::None => None,
        }
    }
}
