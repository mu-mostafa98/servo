/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;
use style::values::computed::svg::{SVGOpacity, SVGStrokeDashArray};
use style::values::generics::svg::SVGLength;
use webrender_api::ColorF;

use crate::style::color::{resolve_svg_paint, ResolvedPaint};
use crate::style::FromComputedValues;
use super::gradient::PaintServer;

/// SVG stroke properties.
#[derive(Debug, Clone)]
pub struct StrokeParams {
    pub color: Option<ColorF>,
    /// Paint server reference (gradient url). When set, takes priority over `color`.
    pub paint_server: Option<PaintServer>,
    pub opacity: f32,
    pub width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Option<Vec<f32>>,
    pub dash_offset: f32,
}

/// SVG line cap style — how the ends of open paths are rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style — how corners are rendered in a polyline/polygon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

// ----------------------- FromComputedValues -----------------------

impl FromComputedValues for StrokeParams {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.stroke, values);
        let opacity = match inherited_svg.stroke_opacity {
            SVGOpacity::Opacity(opacity) => opacity,
            _ => 1.0,
        };

        let width = match &inherited_svg.stroke_width {
            SVGLength::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0)
            },
            _ => 1.0,
        };

        let line_cap = match inherited_svg.stroke_linecap {
            style::computed_values::stroke_linecap::T::Butt => LineCap::Butt,
            style::computed_values::stroke_linecap::T::Round => LineCap::Round,
            style::computed_values::stroke_linecap::T::Square => LineCap::Square,
        };

        let line_join = match inherited_svg.stroke_linejoin {
            style::computed_values::stroke_linejoin::T::Miter => LineJoin::Miter,
            style::computed_values::stroke_linejoin::T::Round => LineJoin::Round,
            style::computed_values::stroke_linejoin::T::Bevel => LineJoin::Bevel,
        };

        let miter_limit = inherited_svg.stroke_miterlimit.0;

        let dash_array = match &inherited_svg.stroke_dasharray {
            SVGStrokeDashArray::Values(values) => {
                if values.is_empty() {
                    None
                } else {
                    Some(
                        values
                            .iter()
                            .map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0))
                            .collect(),
                    )
                }
            },
            _ => None,
        };

        let dash_offset = match &inherited_svg.stroke_dashoffset {
            SVGLength::LengthPercentage(lp) => {
                lp.to_length().map(|l| l.px()).unwrap_or(0.0)
            },
            _ => 0.0,
        };

        if width <= 0.0 {
            return None;
        }

        match paint {
            ResolvedPaint::Color(color) => {
                Some(StrokeParams {
                    color: Some(color),
                    paint_server: None,
                    opacity,
                    width,
                    line_cap,
                    line_join,
                    miter_limit,
                    dash_array,
                    dash_offset,
                })
            },
            ResolvedPaint::PaintServer(id) => {
                Some(StrokeParams {
                    color: None,
                    paint_server: Some(PaintServer::Gradient(id)),
                    opacity,
                    width,
                    line_cap,
                    line_join,
                    miter_limit,
                    dash_array,
                    dash_offset,
                })
            },
            ResolvedPaint::None => None,
        }
    }
}
