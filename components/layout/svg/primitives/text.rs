/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG text span construction from DOM elements.
//!
//! Each function parses the attributes of a `<text>` or `<tspan>` element into
//! an [`svg_engine::element::text::TextSpan`] — the text content plus its
//! per-character offsets and typography flags. These are leaf parsers with no
//! dependency on the builder/defines layers.

use layout_api::LayoutNode;
use script::layout_dom::ServoLayoutNode;
use svg_engine::element::text::{DominantBaseline, TextAnchor, TextSpan};

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
    let mut x = parse_length_list("x", get, fs);
    let mut y = parse_length_list("y", get, fs);
    let mut dx = parse_length_list("dx", get, fs);
    let mut dy = parse_length_list("dy", get, fs);
    let mut rotate = parse_rotate_list(get);
    let text_anchor = parse_text_anchor(get);
    let dominant_baseline = parse_dominant_baseline(get);
    let mut text = extract_direct_text(node);
    if text.is_empty() {
        return None;
    }
    let rtl = apply_rtl_direction(&mut text, &mut x, &mut y, &mut dx, &mut dy, &mut rotate, get);
    Some(TextSpan {
        text,
        x,
        y,
        dx,
        dy,
        rotate,
        text_anchor,
        rtl,
        dominant_baseline,
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
        font_size: fs,
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
    let mut text = text;
    let mut x = parse_length_list("x", get, fs);
    let mut y = parse_length_list("y", get, fs);
    let mut dx = parse_length_list("dx", get, fs);
    let mut dy = parse_length_list("dy", get, fs);
    let mut rotate = parse_rotate_list(get);
    let rtl = apply_rtl_direction(&mut text, &mut x, &mut y, &mut dx, &mut dy, &mut rotate, get);
    Some(TextSpan {
        text,
        x,
        y,
        dx,
        dy,
        rotate,
        text_anchor: parse_text_anchor(get),
        rtl,
        dominant_baseline: parse_dominant_baseline(get),
        glyphs: vec![],
        font_instance_key: None,
        advance_offset: 0.0,
        font_size: fs,
    })
}

fn parse_text_anchor(get: &dyn Fn(&str) -> Option<String>) -> TextAnchor {
    get("text-anchor")
        .as_deref()
        .map(|v| match v.trim() {
            "middle" => TextAnchor::Middle,
            "end" => TextAnchor::End,
            _ => TextAnchor::Start,
        })
        .unwrap_or(TextAnchor::Start)
}

fn parse_dominant_baseline(get: &dyn Fn(&str) -> Option<String>) -> DominantBaseline {
    get("dominant-baseline")
        .as_deref()
        .map(|v| match v.trim() {
            "text-before-edge" => DominantBaseline::TextBeforeEdge,
            "text-after-edge" => DominantBaseline::TextAfterEdge,
            "hanging" => DominantBaseline::Hanging,
            "middle" => DominantBaseline::Middle,
            "central" => DominantBaseline::Central,
            "ideographic" => DominantBaseline::Ideographic,
            "alphabetic" => DominantBaseline::Alphabetic,
            "mathematical" => DominantBaseline::Mathematical,
            _ => DominantBaseline::Auto,
        })
        .unwrap_or(DominantBaseline::Auto)
}

/// If the element is `direction="rtl"`, reverse the per-character position lists
/// (`x`/`y`/`dx`/`dy`/`rotate`) so they line up with the visual glyph order
/// produced by RTL shaping. The text itself is left in logical order — the
/// shaper produces the reversed glyph order for RTL.
fn apply_rtl_direction(
    _text: &mut String,
    x: &mut Vec<f32>,
    y: &mut Vec<f32>,
    dx: &mut Vec<f32>,
    dy: &mut Vec<f32>,
    rotate: &mut Vec<f32>,
    get: &dyn Fn(&str) -> Option<String>,
) -> bool {
    let is_rtl = get("direction")
        .as_deref()
        .map(|d| d.trim().eq_ignore_ascii_case("rtl"))
        .unwrap_or(false);
    if is_rtl {
        x.reverse();
        y.reverse();
        dx.reverse();
        dy.reverse();
        rotate.reverse();
    }
    is_rtl
}

/// Parse the `rotate` attribute into a list of per-character angles (degrees).
fn parse_rotate_list(get: &dyn Fn(&str) -> Option<String>) -> Vec<f32> {
    let Some(val) = get("rotate") else { return vec![] };
    val.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect()
}

/// Parse a space/comma-separated list of lengths from an attribute.
pub(crate) fn parse_length_list(name: &str, get: &dyn Fn(&str) -> Option<String>, fs: f32) -> Vec<f32> {
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

/// Parse a single length value (number or number + any SVG/CSS unit).
fn parse_length_simple(val: &str, fs: f32) -> Option<f32> {
    crate::svg::primitives::attrs::parse_length_token(val, fs)
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
