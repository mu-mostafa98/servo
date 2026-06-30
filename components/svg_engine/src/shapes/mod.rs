/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! This module defines SVG geometric shape structs based on the SVG 2 specification.
//! Each shape has its own file with its [`Build`](crate::builder::Build) implementation.
//! Shared attribute-parsing helpers live in this module.

pub(crate) mod rectangle;
pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod polyline;
pub(crate) mod polygon;
pub(crate) mod path;

pub use self::rectangle::Rectangle;
pub use self::circle::Circle;
pub use self::ellipse::Ellipse;
pub use self::line::Line;
pub use self::polyline::Polyline;
pub use self::polygon::Polygon;
pub use self::path::Path;

use kurbo::{BezPath, Point};

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

// ======================= Attribute Parsing Helpers =======================

/// Parse a named SVG length attribute (e.g. `x="10"`, `width="50"`).
/// Strips trailing `px` suffix and returns the raw float value.
pub(crate) fn parse_length(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> SvgResult<f32> {
    let value =
        get_attr(attr).ok_or_else(|| SvgEngineError::MissingAttribute(attr.to_owned()))?;
    value
        .trim_end_matches("px")
        .trim()
        .parse::<f32>()
        .map_err(|e| SvgEngineError::ParseError(format!("{attr}: {e}")))
}

/// Shared parser for the `points` attribute used by both `<polyline>` and `<polygon>`.
pub(crate) fn parse_points(
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> SvgResult<Vec<Point>> {
    let value =
        get_attr("points").ok_or_else(|| SvgEngineError::MissingAttribute("points".to_owned()))?;
    let coords: Vec<f64> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    let points: Vec<Point> = coords
        .chunks(2)
        .filter_map(|chunk| {
            let x = *chunk.first()?;
            let y = *chunk.get(1)?;
            Some(Point::new(x, y))
        })
        .collect();

    if points.len() < 2 {
        return Err(SvgEngineError::ParseError(
            "points attribute requires at least 2 coordinate pairs".to_owned(),
        ));
    }
    Ok(points)
}

/// Parse the SVG path `d` attribute string into a [`BezPath`].
pub(crate) fn parse_path(
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> SvgResult<BezPath> {
    let value =
        get_attr("d").ok_or_else(|| SvgEngineError::MissingAttribute("d".to_owned()))?;
    BezPath::from_svg(&value)
        .map_err(|e| SvgEngineError::ParseError(format!("path: {e}")))
}
