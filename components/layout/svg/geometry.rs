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

use layout_api::LayoutNode;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::values::computed::LengthPercentage;
use style::values::generics::length::GenericLengthPercentageOrAuto;
use svg_engine::shapes::*;
use svg_engine::text::{TextAnchor, TextSpan};

use super::style::get_attr;

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

// ======================= Public API =======================

/// Build a [`Shape`] from a DOM element using computed values when available.
///
/// Geometry attributes that are CSS properties (`x`, `y`, `cx`, `cy`, `r`,
/// `rx`, `ry`) are read from the cascade via `computed.get_svg()`.
/// Attributes without CSS properties (`width`, `height`, `x1`, `y1`, `x2`,
/// `y2`, `points`, `d`) fall back to DOM attribute parsing.
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

/// Build a text span from a `<text>` or `<tspan>` DOM element.
///
/// Only the element's **own direct text** is collected — `<tspan>` children
/// are *not* recursed into. The builder assembles `<text>` as an ordered list
/// of runs (one per bare text node / `<tspan>`), so that each run keeps its
/// own style and font. This preserves per-tspan `fill` and `font-size`.
pub(crate) fn build_text(
    node: ServoLayoutNode,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
) -> Option<TextSpan> {
    let x = parse_length("x", get, fs).unwrap_or(0.0);
    let y = parse_length("y", get, fs).unwrap_or(0.0);
    let dx = parse_length_list("dx", get, fs);
    let dy = parse_length_list("dy", get, fs);
    let text_anchor = get("text-anchor")
        .as_deref()
        .map(|v| match v.trim() {
            "middle" => TextAnchor::Middle,
            "end" => TextAnchor::End,
            _ => TextAnchor::Start,
        })
        .unwrap_or(TextAnchor::Start);
    let text = extract_direct_text(node);
    if text.is_empty() {
        return None;
    }
    Some(TextSpan {
        text,
        x,
        y,
        dx,
        dy,
        text_anchor,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    })
}

/// Build a text span from a raw string, for bare text-node runs inside a
/// `<text>` that have no attributes of their own (they inherit the parent's
/// x/y/anchor). The run's style is applied by the caller via the parent node.
pub(crate) fn build_text_run(
    text: String,
    get: &dyn Fn(&str) -> Option<String>,
    fs: f32,
) -> Option<TextSpan> {
    if text.is_empty() {
        return None;
    }
    Some(TextSpan {
        text,
        x: parse_length("x", get, fs).unwrap_or(0.0),
        y: parse_length("y", get, fs).unwrap_or(0.0),
        dx: parse_length_list("dx", get, fs),
        dy: parse_length_list("dy", get, fs),
        text_anchor: get("text-anchor")
            .as_deref()
            .map(|v| match v.trim() {
                "middle" => TextAnchor::Middle,
                "end" => TextAnchor::End,
                _ => TextAnchor::Start,
            })
            .unwrap_or(TextAnchor::Start),
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
    })
}

/// Parse a space/comma-separated list of lengths from an attribute.
fn parse_length_list(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> Vec<f32> {
    let Some(val) = get(name) else { return vec![] };
    val.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter_map(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                parse_length_simple(t, fs)
            }
        })
        .collect()
}

/// Parse a single length value (number or number+unit).
fn parse_length_simple(val: &str, _fs: f32) -> Option<f32> {
    let val = val.trim();
    val.trim_end_matches("px").parse::<f32>().ok()
}

/// Extract the **direct** text content of a DOM node — the concatenated
/// text of its non-element children only. `<tspan>` (and other element)
/// children are intentionally excluded: the builder treats each `<tspan>` as
/// its own run with its own style. This prevents flattening tspans into a
/// single string, which would lose per-tspan `fill`/`font-size` and would
/// insert whitespace/newlines that render as missing-glyph boxes.
fn extract_direct_text(node: ServoLayoutNode) -> String {
    let mut text = String::new();
    for child in node.dom_children() {
        if child.as_element().is_none() {
            text.push_str(&child.text_content());
        }
    }
    text
}

// ======================= Shape Parsers =======================

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

// ======================= Helpers =======================

/// Convert a [`LengthPercentage`] to a pixel value.
fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

/// Parse a DOM length attribute as a fallback (for attributes not available
/// through the CSS cascade, like `width`, `height`, `x1`, `y1`).
fn dom_length(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> f32 {
    use svg_engine::attr_parsers::parse_length;
    parse_length(name, get, fs).unwrap_or(0.0)
}

/// Parse a length value using [`svg_engine::attr_parsers::parse_length`].
fn parse_length(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> Result<f32, ()> {
    use svg_engine::attr_parsers::parse_length;

    parse_length(name, get, fs).map_err(|_| ())
}
