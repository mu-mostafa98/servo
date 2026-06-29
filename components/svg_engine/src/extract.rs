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
use crate::transform::TransformOp;

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

/// Parse the full `transform` attribute into an ordered list of transform operations.
///
/// Supports: `translate(tx,ty)`, `scale(s)`, `scale(sx,sy)`, `rotate(a)`, `rotate(a,cx,cy)`.
/// Multiple functions can be chained: `"translate(30,20) rotate(45)"` → `[Translate, Rotate]`.
pub fn extract_transforms(get_attr: &dyn Fn(&str) -> Option<String>) -> Vec<TransformOp> {
    let attr = match get_attr("transform") {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut remaining = attr.trim().to_string();
    let mut ops = Vec::new();

    while !remaining.is_empty() {
        let paren_open = match remaining.find('(') {
            Some(i) => i,
            None => break,
        };
        let paren_close = match remaining.find(')') {
            Some(i) => i,
            None => break,
        };

        let name = remaining[..paren_open].trim().to_string();
        let args_str = &remaining[paren_open + 1..paren_close];
        let args: Vec<f32> = args_str
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.trim().parse::<f32>().ok())
            .collect();

        match name.as_str() {
            "translate" if args.len() == 2 => {
                ops.push(TransformOp::Translate(args[0], args[1]));
            },
            "scale" if args.len() == 1 => {
                ops.push(TransformOp::Scale(args[0], args[0]));
            },
            "scale" if args.len() == 2 => {
                ops.push(TransformOp::Scale(args[0], args[1]));
            },
            "rotate" if args.len() == 1 => {
                ops.push(TransformOp::Rotate(args[0], 0.0, 0.0));
            },
            "rotate" if args.len() == 3 => {
                ops.push(TransformOp::Rotate(args[0], args[1], args[2]));
            },
            _ => {},
        }

        remaining = remaining[paren_close + 1..].trim().to_string();
        remaining = remaining.trim_start_matches(|c: char| c == ';' || c == ',').to_string();
    }

    ops
}

/// Parse a raw CSS `style` attribute string into a NodeStyle.
///
/// Used as a fallback when `ComputedValues` aren't available
/// (e.g. for SVG child elements inside `<g>`).
/// Supports: `fill`, `stroke`, `stroke-width`, `fill-opacity`,
/// `stroke-opacity`, `fill-rule`, `opacity`.
pub fn extract_node_style_from_css(style_str: &str) -> NodeStyle {
    use crate::styles::*;
    use webrender_api::ColorF;

    let mut fill_color: Option<ColorF> = None;
    let mut fill_opacity: f32 = 1.0;
    let mut fill_rule = FillRule::NonZero;
    let mut stroke_color: Option<ColorF> = None;
    let mut stroke_opacity: f32 = 1.0;
    let mut stroke_width: f32 = 1.0;
    let mut has_stroke_width = false;

    for decl in style_str.split(';') {
        let decl = decl.trim();
        if decl.is_empty() { continue; }
        let parts: Vec<&str> = decl.splitn(2, ':').collect();
        if parts.len() != 2 { continue; }
        let prop = parts[0].trim();
        let val = parts[1].trim();

        match prop {
            "fill" => { fill_color = parse_css_color(val); },
            "fill-opacity" => {
                if let Ok(v) = val.parse::<f32>() { fill_opacity = v.clamp(0.0, 1.0); }
            },
            "fill-rule" => {
                fill_rule = if val == "evenodd" { FillRule::EvenOdd } else { FillRule::NonZero };
            },
            "stroke" => { stroke_color = parse_css_color(val); },
            "stroke-width" => {
                let v = val.trim_end_matches("px").trim();
                if let Ok(w) = v.parse::<f32>() { stroke_width = w.max(0.0); has_stroke_width = true; }
            },
            "stroke-opacity" => {
                if let Ok(v) = val.parse::<f32>() { stroke_opacity = v.clamp(0.0, 1.0); }
            },
            "opacity" => {
                if let Ok(v) = val.parse::<f32>() { fill_opacity *= v; stroke_opacity *= v; }
            },
            _ => {},
        }
    }

    NodeStyle {
        fill: fill_color.map(|c| FillParams { color: Some(c), opacity: fill_opacity, fill_rule }),
        stroke: stroke_color.map(|c| StrokeParams {
            color: Some(c), opacity: stroke_opacity,
            width: if has_stroke_width { stroke_width } else { 1.0 },
            line_cap: LineCap::Butt, line_join: LineJoin::Miter,
            miter_limit: 4.0, dash_array: None, dash_offset: 0.0,
        }),
    }
}

/// Parse a CSS color value (hex `#rrggbb` or named color).
fn parse_css_color(val: &str) -> Option<ColorF> {
    let val = val.trim();
    if val == "none" || val == "transparent" { return None; }
    if val.starts_with('#') {
        let hex = &val[1..];
        // #rgb → expand each digit
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(ColorF::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0));
        }
        // #rrggbb
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                return Some(ColorF::new(
                    ((rgb >> 16) & 0xFF) as f32 / 255.0,
                    ((rgb >> 8) & 0xFF) as f32 / 255.0,
                    (rgb & 0xFF) as f32 / 255.0,
                    1.0,
                ));
            }
        }
    }
    match val {
        "red" => Some(ColorF::new(1.0, 0.0, 0.0, 1.0)),
        "green" => Some(ColorF::new(0.0, 0.502, 0.0, 1.0)),
        "blue" => Some(ColorF::new(0.0, 0.0, 1.0, 1.0)),
        "white" => Some(ColorF::new(1.0, 1.0, 1.0, 1.0)),
        "black" => Some(ColorF::new(0.0, 0.0, 0.0, 1.0)),
        "yellow" => Some(ColorF::new(1.0, 1.0, 0.0, 1.0)),
        "orange" => Some(ColorF::new(1.0, 0.647, 0.0, 1.0)),
        "purple" => Some(ColorF::new(0.502, 0.0, 0.502, 1.0)),
        "gray" | "grey" => Some(ColorF::new(0.5, 0.5, 0.5, 1.0)),
        _ => None,
    }
}

