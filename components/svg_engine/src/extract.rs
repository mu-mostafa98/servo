/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::{BezPath, Point};
use style::properties::ComputedValues;
use style::values::computed::svg::{SVGPaint, SVGOpacity, SVGPaintKind, SVGStrokeDashArray };
use style::values::generics::svg::SVGLength;
use style::color::ColorSpace;
use webrender_api::ColorF;

use crate::render_tree::*;
use crate::styles::*;
use crate::shapes::*;

pub fn extract_node_style(computed_values: &ComputedValues) -> NodeStyle {
    NodeStyle{
        fill: extract_fill_params(computed_values),
        stroke: extract_stroke_params(computed_values),
    }

}

pub fn extract_fill_params(computed_values: &ComputedValues) -> Option<FillParams> {

    let inhirited_svg = computed_values.get_inherited_svg();
    let color = resolve_svg_paint(&inhirited_svg.fill, computed_values);
    let opacity = match inhirited_svg.fill_opacity {
        SVGOpacity::Opacity(opacity) => opacity,
        _ => 1.0,
    };
    let fill_rule = match inhirited_svg.fill_rule {
        style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
        style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
    };

    if color.is_none() {
        return None;
    }

    Some(FillParams {
        color,
        opacity,
        fill_rule,
    })
}

fn resolve_svg_paint(svg_paint: &SVGPaint, computed_values: &ComputedValues) -> Option<ColorF> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
            ))
        }
        SVGPaintKind::None => None,
        _ => None,
    }
}

pub fn extract_stroke_params(computed_values: &ComputedValues) -> Option<StrokeParams> {
    let inhirited_svg = computed_values.get_inherited_svg();
    let color = resolve_svg_paint(&inhirited_svg.stroke, computed_values);
    let opacity = match inhirited_svg.stroke_opacity {
        SVGOpacity::Opacity(opacity) => opacity,
        _ => 1.0,
    };

    let width = match &inhirited_svg.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => {
            nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0)
        },
        _ => 1.0,
    };

    let line_cap = match inhirited_svg.stroke_linecap {
        style::computed_values::stroke_linecap::T::Butt => LineCap::Butt,
        style::computed_values::stroke_linecap::T::Round => LineCap::Round,
        style::computed_values::stroke_linecap::T::Square => LineCap::Square,
    };

    let line_join = match inhirited_svg.stroke_linejoin {
        style::computed_values::stroke_linejoin::T::Miter => LineJoin::Miter,
        style::computed_values::stroke_linejoin::T::Round => LineJoin::Round,
        style::computed_values::stroke_linejoin::T::Bevel => LineJoin::Bevel,
    };

    let miter_limit = inhirited_svg.stroke_miterlimit.0;

    let dash_array = match &inhirited_svg.stroke_dasharray {
        SVGStrokeDashArray::Values(values) => {
            if values.is_empty() {
                None
            } else {
                Some(values.iter().map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0)).collect())
            }
        },
        _ => None,
    };

    let dash_offset = match &inhirited_svg.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => {
            lp.to_length().map(|l| l.px()).unwrap_or(0.0)
        },
        _ => 0.0,
    };

    if color.is_none() || width <= 0.0 {
        return None;
    }

    Some(StrokeParams {
        color,
        opacity,
        width,
        line_cap,
        line_join,
        miter_limit,
        dash_array,
        dash_offset,
    })
}

pub fn extract_tag(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<SvgTag> {
    match name {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "rect" => extract_rect(get_attr).map(|s| SvgTag::Shape(Shape::Rect(s))),
        "ellipse" => extract_ellipse(get_attr).map(|s| SvgTag::Shape(Shape::Ellipse(s))),
        "circle" => extract_circle(get_attr).map(|s| SvgTag::Shape(Shape::Circle(s))),
        "line" => extract_line(get_attr).map(|s| SvgTag::Shape(Shape::Line(s))),
        "polyline" => parse_points(get_attr).map(|pts| SvgTag::Shape(Shape::Polyline(Polyline { points: pts }))),
        "polygon" => parse_points(get_attr).map(|pts| SvgTag::Shape(Shape::Polygon(Polygon { points: pts }))),
        "path" => parse_path(get_attr).map(|path| SvgTag::Shape(Shape::Path(Path { path }))),
        _ => None,
    }
}

fn extract_rect(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Rectangle> {
    Some(Rectangle {
        x: parse_length("x", get_attr)?,
        y: parse_length("y", get_attr)?,
        width: parse_length("width", get_attr)?,
        height: parse_length("height", get_attr)?,
        rx: parse_length("rx", get_attr),
        ry: parse_length("ry", get_attr),
    })
}

fn parse_length(attr: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<f32> {
    let value = get_attr(attr)?;
    value.trim_end_matches("px").trim().parse::<f32>().ok()
}

fn extract_ellipse(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Ellipse> {
    Some(Ellipse {
        cx: parse_length("cx", get_attr)?,
        cy: parse_length("cy", get_attr)?,
        rx: parse_length("rx", get_attr)?,
        ry: parse_length("ry", get_attr)?,
    })
}

fn extract_circle(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Circle> {
    Some(Circle {
        cx: parse_length("cx", get_attr)?,
        cy: parse_length("cy", get_attr)?,
        r: parse_length("r", get_attr)?,
    })
}

fn extract_line(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Line> {
    Some(Line {
        x1: parse_length("x1", get_attr)?,
        y1: parse_length("y1", get_attr)?,
        x2: parse_length("x2", get_attr)?,
        y2: parse_length("y2", get_attr)?,
    })
}

/// Shared parser for the `points` attribute used by both `<polyline>` and `<polygon>`.
///
/// Accepts formats like:
/// - `"10,20 30,40 50,60"`
/// - `"10 20 30 40 50 60"`
/// - `"10.5,20.3 30.7,40.1"`
/// Returns `None` if fewer than 2 points are found.
fn parse_points(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Point>> {
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

fn parse_path(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<BezPath> {
    let value = get_attr("d")?;
    BezPath::from_svg(&value).ok()
}
