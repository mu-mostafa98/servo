/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! This module defines SVG geometric shape structs based on the SVG 2 specification.
//! Each shape has its own file with its [`Build`](crate::builder::Build) implementation.
//! Shared attribute-parsing helpers live in [`attr_parsers`].

pub(crate) mod rectangle;
pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod polyline;
pub(crate) mod polygon;
pub(crate) mod path;
pub(crate) mod attr_parsers;

pub use self::rectangle::Rectangle;
pub use self::circle::Circle;
pub use self::ellipse::Ellipse;
pub use self::line::Line;
pub use self::polyline::Polyline;
pub use self::polygon::Polygon;
pub use self::path::Path;

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};
use crate::error::SvgEngineError;

/// An SVG geometric shape.
#[derive(Debug, Clone)]
pub enum Shape {
    Rect(Rectangle),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polyline(Polyline),
    Polygon(Polygon),
    Path(Path),
}

// ======================= Build dispatch =======================

impl Build for Shape {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        match input.element_name {
            "rect" => Rectangle::build(input).map(Shape::Rect),
            "circle" => Circle::build(input).map(Shape::Circle),
            "ellipse" => Ellipse::build(input).map(Shape::Ellipse),
            "line" => Line::build(input).map(Shape::Line),
            "polyline" => Polyline::build(input).map(Shape::Polyline),
            "polygon" => Polygon::build(input).map(Shape::Polygon),
            "path" => Path::build(input).map(Shape::Path),
            other => Err(SvgEngineError::UnsupportedFeature(
                format!("unknown shape element: {other}"),
            )),
        }
    }
}
