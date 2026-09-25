/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Inline CSS parsing for SVG `<style>` elements.
//!
//! Parses class-based CSS rules from `<style>` elements inside SVG subtrees
//! and applies them to shape styles.  This is a simple best-effort parser
//! (not a full CSS engine) — it handles `.class { prop: value; }` syntax.

use std::collections::HashMap;

use layout_api::{LayoutElement, LayoutNode, LayoutNodeType};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::style::*;
use svg_engine::units::{Length, Opacity};

use crate::svg::primitives::attrs::get_attr;
use crate::svg::primitives::paint::parse_paint_server;

/// A simple mapping from class name to (property → value) parsed from
/// `<style>` elements inside an SVG subtree.
pub(crate) type CssClassRules = HashMap<String, HashMap<String, String>>;

/// Collect CSS class rules from all `<style>` elements inside the SVG DOM subtree.
pub(crate) fn collect_svg_css_rules<'dom>(root_node: ServoLayoutNode<'dom>) -> CssClassRules {
    let mut all_rules: CssClassRules = HashMap::new();
    let mut stack: Vec<ServoLayoutNode<'dom>> = vec![root_node];
    while let Some(node) = stack.pop() {
        if let Some(element) = node.as_element() {
            if element.local_name().as_ref() == "style" {
                if let Some(css_text) = extract_style_text_content(node) {
                    let rules = parse_svg_class_rules(&css_text);
                    for (cls, props) in rules {
                        all_rules.entry(cls).or_default().extend(props);
                    }
                }
            }
        }
        for child in node.dom_children() {
            stack.push(child);
        }
    }
    all_rules
}

/// Extract the text content of a `<style>` element.
fn extract_style_text_content<'dom>(node: ServoLayoutNode<'dom>) -> Option<String> {
    let mut text = String::new();
    for child in node.dom_children() {
        if let Some(LayoutNodeType::Text) = child.type_id() {
            text.push_str(&child.text_content());
        }
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Parse CSS text into a map of class name → (property → value).
fn parse_svg_class_rules(css_text: &str) -> CssClassRules {
    let mut rules: CssClassRules = HashMap::new();
    for block in css_text.split('}') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut parts = block.splitn(2, '{');
        let selector = parts.next().unwrap_or("").trim();
        let declarations = parts.next().unwrap_or("").trim();
        if selector.is_empty() || declarations.is_empty() {
            continue;
        }
        if !selector.starts_with('.') {
            continue;
        }
        let class_name = selector[1..].trim();
        if class_name.is_empty() || class_name.contains(' ') {
            continue;
        }
        let props = parse_svg_declarations(declarations);
        rules.insert(class_name.to_owned(), props);
    }
    rules
}

/// Parse CSS declaration block into property → value pairs.
fn parse_svg_declarations(block: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for decl in block.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let mut parts = decl.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim().to_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }
        props.insert(name, value.to_owned());
    }
    props
}

/// Apply CSS class rules to a style, based on the element's `class` attribute.
pub(crate) fn apply_css_class_rules(
    element: &ServoLayoutElement,
    css_rules: &CssClassRules,
    style: &mut NodeStyle,
) {
    let Some(class_attr) = get_attr(element, "class") else {
        return;
    };
    for class_name in class_attr.split_whitespace() {
        let Some(props) = css_rules.get(class_name) else {
            continue;
        };
        for (prop, value) in props {
            apply_css_property(style, prop, value);
        }
    }
}

/// Apply a single CSS property to a [`NodeStyle`].
fn apply_css_property(style: &mut NodeStyle, prop: &str, value: &str) {
    match prop {
        "fill" | "fill-color" => {
            if let Some(ps) = parse_paint_server(value) {
                let fill = style.fill.get_or_insert_with(|| FillParams {
                    paint_server: None,
                    opacity: Opacity::ONE,
                    fill_rule: FillRule::NonZero,
                });
                fill.paint_server = Some(ps);
            } else if value.eq_ignore_ascii_case("none") {
                style.fill = None;
            }
        },
        "fill-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut fill) = style.fill {
                    fill.opacity = Opacity::new(op);
                }
            }
        },
        "stroke" | "stroke-color" => {
            if let Some(ps) = parse_paint_server(value) {
                let stroke = style.stroke.get_or_insert_with(|| StrokeParams {
                    paint_server: None,
                    opacity: Opacity::ONE,
                    width: Length::new(1.0),
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    miter_limit: 4.0,
                    dash_array: None,
                    dash_offset: 0.0,
                });
                stroke.paint_server = Some(ps);
            } else if value.eq_ignore_ascii_case("none") {
                style.stroke = None;
            }
        },
        "stroke-width" => {
            if let Ok(w) = value.trim_end_matches("px").parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.width = Length::new(w.max(0.0));
                }
            }
        },
        "stroke-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.opacity = Opacity::new(op);
                }
            }
        },
        "stroke-linecap" => {
            let lc = match value {
                "round" => LineCap::Round,
                "square" => LineCap::Square,
                _ => LineCap::Butt,
            };
            if let Some(ref mut s) = style.stroke {
                s.line_cap = lc;
            }
        },
        "stroke-linejoin" => {
            let lj = match value {
                "miter-clip" => LineJoin::MiterClip,
                "round" => LineJoin::Round,
                "bevel" => LineJoin::Bevel,
                "arcs" => LineJoin::Arcs,
                _ => LineJoin::Miter,
            };
            if let Some(ref mut s) = style.stroke {
                s.line_join = lj;
            }
        },
        "stroke-dasharray" => {
            if value != "none" {
                let dashes: Vec<f32> = value
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                if !dashes.is_empty() {
                    if let Some(ref mut s) = style.stroke {
                        s.dash_array = Some(dashes);
                    }
                }
            } else if let Some(ref mut s) = style.stroke {
                s.dash_array = None;
            }
        },
        "stroke-dashoffset" => {
            if let Ok(off) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.dash_offset = off;
                }
            }
        },
        "opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                style.opacity = Opacity::new(op);
            }
        },
        "visibility" => {
            style.visibility = match value {
                "hidden" | "collapse" => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        },
        _ => {},
    }
}
