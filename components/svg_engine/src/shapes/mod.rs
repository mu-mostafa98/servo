/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! This module defines SVG geometric shape structs based on the SVG 2 specification.
//! Each shape has its own file with its [`FromAttributes`] implementation.
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

// ----------------------- FromAttributes Trait -----------------------

/// Parse a shape from SVG element attributes.
///
/// Every shape type implements this trait. Individual shapes ignore the `name`
/// parameter, while [`Shape`] dispatches on it — matching the [`Render`](crate::renderer::Render) pattern.
pub trait FromAttributes: Sized {
    fn from_attributes(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self>;
}

// ----------------------- Shape Dispatch -----------------------

impl FromAttributes for Shape {
    fn from_attributes(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Self> {
        match name {
            "rect" => Rectangle::from_attributes(name, get_attr).map(Shape::Rect),
            "circle" => Circle::from_attributes(name, get_attr).map(Shape::Circle),
            "ellipse" => Ellipse::from_attributes(name, get_attr).map(Shape::Ellipse),
            "line" => Line::from_attributes(name, get_attr).map(Shape::Line),
            "polyline" => Polyline::from_attributes(name, get_attr).map(Shape::Polyline),
            "polygon" => Polygon::from_attributes(name, get_attr).map(Shape::Polygon),
            "path" => Path::from_attributes(name, get_attr).map(Shape::Path),
            _ => None,
        }
    }
}

// ----------------------- Attribute Parsing Helpers -----------------------

/// Parse a named SVG length attribute (e.g. `x="10"`, `width="50%px"`).
/// Strips trailing `px` suffix and returns the raw float value.
pub(crate) fn parse_length(attr: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<f32> {
    let value = get_attr(attr)?;
    value.trim_end_matches("px").trim().parse::<f32>().ok()
}

/// Shared parser for the `points` attribute used by both `<polyline>` and `<polygon>`.
pub(crate) fn parse_points(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Point>> {
    let value = get_attr("points")?;
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

    if points.len() < 2 { None } else { Some(points) }
}

/// Parse the SVG path `d` attribute string into a [`BezPath`].
pub(crate) fn parse_path(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<BezPath> {
    let value = get_attr("d")?;
    BezPath::from_svg(&value).ok()
}
