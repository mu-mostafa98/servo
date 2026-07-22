/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use script::layout_dom::ServoLayoutElement;
use style::values::computed::LengthPercentage;
use style::values::generics::length::GenericLengthPercentageOrAuto;
use svg_engine::shapes::*;

use super::style::get_attr;

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

pub(crate) fn build_shape(
    element: &ServoLayoutElement,
    tag_name: &str,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<Shape> {
    let fs = SVG_DEFAULT_FONT_SIZE;
    let get = |name: &str| get_attr(element, name);

    match tag_name {
        "rect" => parse_rect(element, &get, fs, computed),
        "circle" => parse_circle(element, &get, fs, computed),
        "ellipse" => parse_ellipse(element, &get, fs, computed),
        "line" => parse_line(&get, fs),
        "polyline" => parse_polyline(&get),
        "polygon" => parse_polygon(&get),
        "path" => parse_path(&get),
        _ => None,
    }
}

fn parse_rect(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<Shape> {
    let (x, y, rx, ry) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (
                lp_to_f32(&svg.clone_x()),
                lp_to_f32(&svg.clone_y()),
                match svg.clone_rx() {
                    GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                        Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
                    },
                    _ => None,
                },
                match svg.clone_ry() {
                    GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                        Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
                    },
                    _ => None,
                },
            )
        },
        None => (
            parse_length("x", get, fs).unwrap_or(0.0),
            parse_length("y", get, fs).unwrap_or(0.0),
            parse_length("rx", get, fs).ok(),
            parse_length("ry", get, fs).ok(),
        ),
    };
    let w = dom_length("width", get, fs);
    let h = dom_length("height", get, fs);
    if w < 0.0 || h < 0.0 {
        return None;
    }
    Some(Shape::Rect(Rectangle {
        x,
        y,
        width: w,
        height: h,
        rx,
        ry,
    }))
}

fn parse_circle(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<Shape> {
    let r = match computed {
        Some(cv) => cv
            .get_svg()
            .clone_r()
            .0
            .to_length()
            .map(|l| l.px())
            .unwrap_or(0.0)
            .max(0.0),
        None => dom_length("r", get, fs).max(0.0),
    };
    if r <= 0.0 {
        return None;
    }
    let (cx, cy) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (lp_to_f32(&svg.clone_cx()), lp_to_f32(&svg.clone_cy()))
        },
        None => (dom_length("cx", get, fs), dom_length("cy", get, fs)),
    };
    Some(Shape::Circle(Circle { cx, cy, r }))
}

fn parse_ellipse(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<Shape> {
    let rx = if let Some(cv) = computed {
        match cv.get_svg().clone_rx() {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
            },
            _ => None,
        }
    } else {
        Some(dom_length("rx", get, fs))
    }?;
    let ry = if let Some(cv) = computed {
        match cv.get_svg().clone_ry() {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
            },
            _ => None,
        }
    } else {
        Some(dom_length("ry", get, fs))
    }?;
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let (cx, cy) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (lp_to_f32(&svg.clone_cx()), lp_to_f32(&svg.clone_cy()))
        },
        None => (dom_length("cx", get, fs), dom_length("cy", get, fs)),
    };
    Some(Shape::Ellipse(Ellipse { cx, cy, rx, ry }))
}

fn parse_line(get: &dyn Fn(&str) -> Option<String>, fs: f32) -> Option<Shape> {
    Some(Shape::Line(Line {
        x1: parse_length("x1", get, fs).unwrap_or(0.0),
        y1: parse_length("y1", get, fs).unwrap_or(0.0),
        x2: parse_length("x2", get, fs).unwrap_or(0.0),
        y2: parse_length("y2", get, fs).unwrap_or(0.0),
    }))
}

fn parse_polyline(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    use svg_engine::attr_parsers::parse_points;
    parse_points(get)
        .ok()
        .map(|pts| Shape::Polyline(Polyline { points: pts }))
}

fn parse_polygon(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    use svg_engine::attr_parsers::parse_points;
    parse_points(get)
        .ok()
        .map(|pts| Shape::Polygon(Polygon { points: pts }))
}

fn parse_path(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    let d = get("d")?;
    kurbo::BezPath::from_svg(&d)
        .ok()
        .map(|path| Shape::Path(Path { path }))
}

fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

fn dom_length(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> f32 {
    use svg_engine::attr_parsers::parse_length;
    parse_length(name, get, fs).unwrap_or(0.0)
}

fn parse_length(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> Result<f32, ()> {
    use svg_engine::attr_parsers::parse_length;

    parse_length(name, get, fs).map_err(|_| ())
}
