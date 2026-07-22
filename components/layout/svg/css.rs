/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use layout_api::{LayoutElement, LayoutNode, LayoutNodeType};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::style::gradient::PaintServer;
use svg_engine::style::*;

use super::style::get_attr;

pub(crate) type CssClassRules = HashMap<String, HashMap<String, String>>;

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

fn apply_css_property(style: &mut NodeStyle, prop: &str, value: &str) {
    match prop {
        "fill" | "fill-color" => {
            if let Some(ps) = PaintServer::from_attr(value) {
                match ps {
                    PaintServer::Solid(c) => {
                        style.fill = Some(FillParams {
                            color: Some(c),
                            paint_server: None,
                            opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
                            fill_rule: style
                                .fill
                                .as_ref()
                                .map(|f| f.fill_rule)
                                .unwrap_or(FillRule::NonZero),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.fill = Some(FillParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
                            fill_rule: style
                                .fill
                                .as_ref()
                                .map(|f| f.fill_rule)
                                .unwrap_or(FillRule::NonZero),
                        });
                    },
                    PaintServer::Pattern(_) => {},
                }
            } else if value.eq_ignore_ascii_case("none") {
                style.fill = None;
            }
        },
        "fill-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut fill) = style.fill {
                    fill.opacity = op.clamp(0.0, 1.0);
                }
            }
        },
        "stroke" | "stroke-color" => {
            if let Some(ps) = PaintServer::from_attr(value) {
                match ps {
                    PaintServer::Solid(c) => {
                        style.stroke = Some(StrokeParams {
                            color: Some(c),
                            paint_server: None,
                            opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                            width: style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0),
                            line_cap: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_cap)
                                .unwrap_or(LineCap::Butt),
                            line_join: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_join)
                                .unwrap_or(LineJoin::Miter),
                            miter_limit: style
                                .stroke
                                .as_ref()
                                .map(|s| s.miter_limit)
                                .unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style
                                .stroke
                                .as_ref()
                                .map(|s| s.dash_offset)
                                .unwrap_or(0.0),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.stroke = Some(StrokeParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                            width: style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0),
                            line_cap: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_cap)
                                .unwrap_or(LineCap::Butt),
                            line_join: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_join)
                                .unwrap_or(LineJoin::Miter),
                            miter_limit: style
                                .stroke
                                .as_ref()
                                .map(|s| s.miter_limit)
                                .unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style
                                .stroke
                                .as_ref()
                                .map(|s| s.dash_offset)
                                .unwrap_or(0.0),
                        });
                    },
                    PaintServer::Pattern(_) => {},
                }
            } else if value.eq_ignore_ascii_case("none") {
                style.stroke = None;
            }
        },
        "stroke-width" => {
            if let Ok(w) = value.trim_end_matches("px").parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.width = w.max(0.0);
                }
            }
        },
        "stroke-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.opacity = op.clamp(0.0, 1.0);
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
                "round" => LineJoin::Round,
                "bevel" => LineJoin::Bevel,
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
                style.opacity = op.clamp(0.0, 1.0);
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
