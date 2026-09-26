/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG shape geometry construction from DOM elements.
//!
//! Each function takes a DOM element, its tag name, and optional computed
//! values, and returns an [`svg_engine::shapes::Shape`] or `None` if the
//! element does not represent a valid shape.
//!
//! # Design
//!
//! Every `parse_*` function does exactly ONE job: extract attributes from
//! the DOM element and construct the corresponding shape struct.  There is
//! no shared mutable state and no side effects.

use script::layout_dom::ServoLayoutElement;
use style::values::computed::{Length as CssLength, LengthPercentage};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use svg_engine::geometry::{PathCommand, PathData, Point};
use svg_engine::shapes::*;
use svg_engine::units::Length;

use crate::svg::primitives::attrs::{get_attr, parse_length_resolved, parse_points};

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

// ======================= Public API =======================

/// Build a [`Shape`] from a DOM element using computed values when available.
///
/// Geometry attributes that are CSS properties (`x`, `y`, `cx`, `cy`, `r`,
/// `rx`, `ry`) are read from the cascade via `computed.get_svg()`.
/// Attributes without CSS properties (`width`, `height`, `x1`, `y1`, `x2`,
/// `y2`, `points`, `d`, `pathLength`) fall back to DOM attribute parsing.
///
/// `vw`/`vh` are the current viewport's percentage-resolution reference
/// dimensions (the `viewBox` extent when present, otherwise the viewport
/// `width`/`height` attributes).
pub(crate) fn build_shape(
    element: &ServoLayoutElement,
    tag_name: &str,
    computed: Option<&style::properties::ComputedValues>,
    vw: f32,
    vh: f32,
) -> Option<Shape> {
    let fs = SVG_DEFAULT_FONT_SIZE;
    let get = |name: &str| get_attr(element, name);

    match tag_name {
        "rect" => parse_rect(element, &get, fs, computed, vw, vh),
        "circle" => parse_circle(element, &get, fs, computed, vw, vh),
        "ellipse" => parse_ellipse(element, &get, fs, computed, vw, vh),
        "line" => parse_line(&get, fs, vw, vh),
        "polyline" => parse_polyline(&get),
        "polygon" => parse_polygon(&get),
        "path" => parse_path(&get),
        _ => None,
    }
}

// ======================= Shape Parsers =======================

fn parse_rect(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
    vw: f32,
    vh: f32,
) -> Option<Shape> {
    let w = dom_length("width", get, fs);
    let h = dom_length("height", get, fs);
    if w < 0.0 || h < 0.0 {
        return None;
    }
    let (x, y, rx, ry) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (
                lp_to_f32(&svg.clone_x(), vw),
                lp_to_f32(&svg.clone_y(), vh),
                match svg.clone_rx() {
                    GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                        resolve_radius(&nn_lp.0, w)
                    },
                    _ => None,
                },
                match svg.clone_ry() {
                    GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                        resolve_radius(&nn_lp.0, h)
                    },
                    _ => None,
                },
            )
        },
        None => (
            dom_length_resolved("x", get, fs, vw),
            dom_length_resolved("y", get, fs, vh),
            parse_radius("rx", get, fs, w),
            parse_radius("ry", get, fs, h),
        ),
    };
    Some(Shape::Rect(Rectangle {
        x: Length::new(x),
        y: Length::new(y),
        width: Length::new(w),
        height: Length::new(h),
        rx: rx.map(Length::new),
        ry: ry.map(Length::new),
        path_length: parse_path_length(get),
    }))
}

fn parse_circle(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
    vw: f32,
    vh: f32,
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
            (lp_to_f32(&svg.clone_cx(), vw), lp_to_f32(&svg.clone_cy(), vh))
        },
        None => (
            dom_length_resolved("cx", get, fs, vw),
            dom_length_resolved("cy", get, fs, vh),
        ),
    };
    Some(Shape::Circle(Circle {
        cx: Length::new(cx),
        cy: Length::new(cy),
        r: Length::new(r),
        path_length: parse_path_length(get),
    }))
}

fn parse_ellipse(
    _element: &ServoLayoutElement,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    computed: Option<&style::properties::ComputedValues>,
    vw: f32,
    vh: f32,
) -> Option<Shape> {
    // SVG 2: `rx`/`ry` accept `auto`. Both `auto` disables rendering, a single
    // `auto` derives from the other. `None` carries the `auto`/negative case to
    // the renderer, which performs the derivation.
    let rx = match computed {
        Some(cv) => match cv.get_svg().clone_rx() {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                resolve_radius(&nn_lp.0, vw)
            },
            _ => None,
        },
        None => parse_radius("rx", get, fs, vw),
    };
    let ry = match computed {
        Some(cv) => match cv.get_svg().clone_ry() {
            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                resolve_radius(&nn_lp.0, vh)
            },
            _ => None,
        },
        None => parse_radius("ry", get, fs, vh),
    };
    let (cx, cy) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (lp_to_f32(&svg.clone_cx(), vw), lp_to_f32(&svg.clone_cy(), vh))
        },
        None => (
            dom_length_resolved("cx", get, fs, vw),
            dom_length_resolved("cy", get, fs, vh),
        ),
    };
    Some(Shape::Ellipse(Ellipse {
        cx: Length::new(cx),
        cy: Length::new(cy),
        rx: rx.map(Length::new),
        ry: ry.map(Length::new),
        path_length: parse_path_length(get),
    }))
}

fn parse_line(
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    vw: f32,
    vh: f32,
) -> Option<Shape> {
    Some(Shape::Line(Line {
        x1: Length::new(dom_length_resolved("x1", get, fs, vw)),
        y1: Length::new(dom_length_resolved("y1", get, fs, vh)),
        x2: Length::new(dom_length_resolved("x2", get, fs, vw)),
        y2: Length::new(dom_length_resolved("y2", get, fs, vh)),
        path_length: parse_path_length(get),
    }))
}

fn parse_polyline(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    Some(Shape::Polyline(Polyline {
        points: parse_points(get),
        path_length: parse_path_length(get),
    }))
}

fn parse_polygon(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    Some(Shape::Polygon(Polygon {
        points: parse_points(get),
        path_length: parse_path_length(get),
    }))
}

fn parse_path(get: &dyn Fn(&str) -> Option<String>) -> Option<Shape> {
    let d = get("d")?;
    kurbo::BezPath::from_svg(&d).ok().map(|bez| {
        Shape::Path(Path {
            path: bez_to_path_data(&bez),
            path_length: parse_path_length(get),
        })
    })
}

/// Convert a parsed [`kurbo::BezPath`] into the model's pure [`PathData`].
///
/// The engine's model stores paths without any `kurbo` dependency, so this is
/// the DOM → model boundary: `kurbo::BezPath::from_svg` parses the `d`
/// attribute, and here we copy the flattened command list (coordinates as f32)
/// into the model representation.
fn bez_to_path_data(bez: &kurbo::BezPath) -> PathData {
    let commands = bez
        .elements()
        .iter()
        .map(|el| match el {
            kurbo::PathEl::MoveTo(p) => PathCommand::MoveTo(Point::new(p.x as f32, p.y as f32)),
            kurbo::PathEl::LineTo(p) => PathCommand::LineTo(Point::new(p.x as f32, p.y as f32)),
            kurbo::PathEl::QuadTo(c, p) => PathCommand::QuadTo(
                Point::new(c.x as f32, c.y as f32),
                Point::new(p.x as f32, p.y as f32),
            ),
            kurbo::PathEl::CurveTo(c1, c2, p) => PathCommand::CurveTo(
                Point::new(c1.x as f32, c1.y as f32),
                Point::new(c2.x as f32, c2.y as f32),
                Point::new(p.x as f32, p.y as f32),
            ),
            kurbo::PathEl::ClosePath => PathCommand::Close,
        })
        .collect();
    PathData { commands }
}

// ======================= Helpers =======================

/// Convert a [`LengthPercentage`] to a pixel value, resolving percentages
/// against `reference`.
fn lp_to_f32(lp: &LengthPercentage, reference: f32) -> f32 {
    lp.resolve(CssLength::new(reference)).px()
}

/// Resolve a radius [`LengthPercentage`] against `reference`, mapping a
/// negative result to `None` (SVG 2 treats a negative radius as `auto`).
fn resolve_radius(lp: &LengthPercentage, reference: f32) -> Option<f32> {
    let v = lp_to_f32(lp, reference);
    (v >= 0.0).then_some(v)
}

/// Parse a DOM length attribute with no percentage resolution (used for
/// `width`/`height`/`r`, whose percentage references differ or are out of
/// scope for this layer).
fn dom_length(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> f32 {
    use crate::svg::primitives::attrs::parse_length;
    parse_length(name, get, fs).unwrap_or(0.0)
}

/// Parse a DOM length attribute, resolving percentages against `reference`.
fn dom_length_resolved(
    name: &str,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    reference: f32,
) -> f32 {
    parse_length_resolved(name, get, fs, reference).unwrap_or(0.0)
}

/// Parse a DOM radius attribute (`rx`/`ry`), resolving percentages against
/// `reference` and mapping a negative value to `None` (SVG 2: `auto`).
fn parse_radius(
    name: &str,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
    reference: f32,
) -> Option<f32> {
    parse_length_resolved(name, get, fs, reference)
        .ok()
        .and_then(|v| (v >= 0.0).then_some(v))
}

/// Parse the `pathLength` presentation attribute (a positive number).
fn parse_path_length(get: &dyn Fn(&str) -> Option<String>) -> Option<f32> {
    get("pathLength")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|&n| n.is_finite() && n > 0.0)
}
