/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::render_tree::*;
use svg_engine::style::gradient::{GradientDef, parse_gradient_element};
use web_atoms::ns;

use super::geometry::build_shape;
use super::style::build_style_from_attrs;
use crate::context::LayoutContext;

pub(crate) trait DefinitionParser {
    type Definition;
    fn tag_names() -> &'static [&'static str];
    fn parse(node: ServoLayoutNode, context: &LayoutContext) -> Option<(String, Self::Definition)>;
}

pub(crate) struct DefinitionCollector;

impl DefinitionCollector {
    pub(crate) fn collect<T: DefinitionParser>(
        node: ServoLayoutNode,
        context: &LayoutContext,
    ) -> HashMap<String, T::Definition> {
        let mut result = HashMap::new();
        let mut candidates = Vec::new();
        for defs_child in node.dom_children() {
            if let Some(defs_elem) = defs_child.as_element() {
                if defs_elem.local_name() == &local_name!("defs") {
                    for tag in T::tag_names() {
                        find_elements_by_tag(defs_child, tag, &mut candidates);
                    }
                }
            }
        }
        for candidate_node in candidates {
            if candidate_node.as_element().is_some() {
                if let Some((id, def)) = T::parse(candidate_node, context) {
                    result.insert(id, def);
                }
            }
        }
        result
    }
}

fn find_elements_by_tag<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &str,
    result: &mut Vec<ServoLayoutNode<'dom>>,
) {
    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == tag {
                result.push(child);
            }
            let name = elem.local_name().as_ref();
            if name == "g" || name == "defs" || name == "svg" || name == "a" || name == "switch" {
                find_elements_by_tag(child, tag, result);
            }
        }
    }
}

pub(crate) struct GradientParser;

impl DefinitionParser for GradientParser {
    type Definition = GradientDef;
    fn tag_names() -> &'static [&'static str] {
        &["linearGradient", "radialGradient"]
    }

    fn parse(
        node: ServoLayoutNode,
        _context: &LayoutContext,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let grad_name = element.local_name().as_ref().to_owned();
        if grad_name != "linearGradient" && grad_name != "radialGradient" {
            return None;
        }
        let mut stop_attrs: Vec<Vec<(String, String)>> = Vec::new();
        for stop_node in node.dom_children() {
            if let Some(stop_elem) = stop_node.as_element() {
                if stop_elem.local_name() == &local_name!("stop") {
                    let mut attrs: Vec<(String, String)> = Vec::new();
                    if let Some(offset) = stop_elem.attribute_as_str(&ns!(), &local_name!("offset"))
                    {
                        attrs.push(("offset".to_owned(), offset.to_string()));
                    }
                    if let Some(color) =
                        stop_elem.attribute_as_str(&ns!(), &local_name!("stop-color"))
                    {
                        attrs.push(("stop-color".to_owned(), color.to_string()));
                    }
                    if let Some(op) =
                        stop_elem.attribute_as_str(&ns!(), &local_name!("stop-opacity"))
                    {
                        attrs.push(("stop-opacity".to_owned(), op.to_string()));
                    }
                    if !attrs.is_empty() {
                        stop_attrs.push(attrs);
                    }
                }
            }
        }
        let grad_get = |attr: &str| {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .map(|s| s.to_string())
        };
        if let Ok(def) = parse_gradient_element(&grad_name, &grad_get, &stop_attrs) {
            match &def {
                GradientDef::Linear(lg) => return Some((lg.id.clone(), def)),
                GradientDef::Radial(rg) => return Some((rg.id.clone(), def)),
            }
        }
        None
    }
}

pub(crate) struct ClipPathParser;

impl DefinitionParser for ClipPathParser {
    type Definition = ClipPathDef;
    fn tag_names() -> &'static [&'static str] {
        &["clipPath"]
    }

    fn parse(node: ServoLayoutNode, context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let units = element
            .attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(ClipPathUnits::UserSpaceOnUse);
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                let computed = child_elem
                    .style_data()
                    .is_some()
                    .then(|| child_node.style(&context.style_context));
                if let Some(shape) =
                    build_shape(&child_elem, &tag_name, computed.as_ref().map(|v| &**v))
                {
                    shapes.push(shape);
                }
            }
        }
        if !shapes.is_empty() {
            Some((
                id,
                ClipPathDef {
                    shapes,
                    clip_path_units: units,
                },
            ))
        } else {
            None
        }
    }
}

pub(crate) struct PatternParser;

impl DefinitionParser for PatternParser {
    type Definition = PatternDef;
    fn tag_names() -> &'static [&'static str] {
        &["pattern"]
    }

    fn parse(node: ServoLayoutNode, context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let parse_attr = |attr: &str, default: f32| -> f32 {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(default)
        };
        let width = parse_attr("width", 0.0);
        let height = parse_attr("height", 0.0);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let x = parse_attr("x", 0.0);
        let y = parse_attr("y", 0.0);
        let pattern_units = element
            .attribute_as_str(&ns!(), &local_name!("patternUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(PatternUnits::UserSpaceOnUse);
        let pattern_content_units = element
            .attribute_as_str(&ns!(), &local_name!("patternContentUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternContentUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(PatternContentUnits::UserSpaceOnUse);
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                let computed = child_elem
                    .style_data()
                    .is_some()
                    .then(|| child_node.style(&context.style_context));
                if let Some(shape) =
                    build_shape(&child_elem, &tag_name, computed.as_ref().map(|v| &**v))
                {
                    let style = build_style_from_attrs(child_node, context);
                    shapes.push((shape, style));
                }
            }
        }
        if !shapes.is_empty() {
            Some((
                id,
                PatternDef {
                    width,
                    height,
                    x,
                    y,
                    pattern_units,
                    pattern_content_units,
                    shapes,
                },
            ))
        } else {
            None
        }
    }
}

pub(crate) struct MaskParser;

impl DefinitionParser for MaskParser {
    type Definition = MaskDef;
    fn tag_names() -> &'static [&'static str] {
        &["mask"]
    }

    fn parse(node: ServoLayoutNode, context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                let computed = child_elem
                    .style_data()
                    .is_some()
                    .then(|| child_node.style(&context.style_context));
                if let Some(shape) =
                    build_shape(&child_elem, &tag_name, computed.as_ref().map(|v| &**v))
                {
                    let style = build_style_from_attrs(child_node, context);
                    shapes.push((shape, style));
                }
            }
        }
        if !shapes.is_empty() {
            Some((id, MaskDef { shapes }))
        } else {
            None
        }
    }
}

pub(crate) struct FilterParser;

impl DefinitionParser for FilterParser {
    type Definition = FilterDef;
    fn tag_names() -> &'static [&'static str] {
        &["filter"]
    }

    fn parse(
        node: ServoLayoutNode,
        _context: &LayoutContext,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let get = |attr: &str| {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .map(|s| s.to_string())
        };
        let get_float = |attr: &str, default: f32| -> f32 {
            get(attr)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(default)
        };
        let x = get_float("x", -0.1);
        let y = get_float("y", -0.1);
        let width = get_float("width", 1.2);
        let height = get_float("height", 1.2);

        let mut primitives = Vec::new();
        for prim_child in node.dom_children() {
            if let Some(prim_elem) = prim_child.as_element() {
                let pname = prim_elem.local_name().as_ref().to_owned();
                let prim_get = |attr: &str| {
                    prim_elem
                        .attribute_as_str(&ns!(), &LocalName::from(attr))
                        .map(|s| s.to_string())
                };
                let prim_get_float = |attr: &str, default: f32| -> f32 {
                    prim_get(attr)
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(default)
                };
                match pname.as_str() {
                    "feGaussianBlur" => {
                        let std_dev = prim_get_float("stdDeviation", 0.0);
                        primitives.push(FilterPrimitive::GaussianBlur(std_dev, std_dev));
                    },
                    "feDropShadow" => {
                        let dx = prim_get_float("dx", 2.0);
                        let dy = prim_get_float("dy", 2.0);
                        let std_dev = prim_get_float("stdDeviation", 2.0);
                        primitives.push(FilterPrimitive::DropShadow(
                            dx, dy, std_dev, 0.0, 0.0, 0.0, 0.5,
                        ));
                    },
                    "feColorMatrix" => {
                        let type_attr = prim_get("type").unwrap_or_else(|| "matrix".to_owned());
                        match type_attr.trim() {
                            "saturate" => {
                                let s = prim_get_float("values", 1.0);
                                primitives.push(FilterPrimitive::Saturate(s));
                            },
                            "hueRotate" => {
                                primitives.push(FilterPrimitive::ColorMatrix([
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                                ]));
                            },
                            "luminanceToAlpha" => {
                                primitives.push(FilterPrimitive::LuminanceToAlpha);
                            },
                            _ => {
                                let values_str = prim_get("values").unwrap_or_default();
                                let vals: Vec<f32> = values_str
                                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                                    .filter_map(|s| {
                                        let t = s.trim();
                                        if t.is_empty() {
                                            None
                                        } else {
                                            t.parse::<f32>().ok()
                                        }
                                    })
                                    .collect();
                                let mut matrix = [
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                                ];
                                for (i, v) in vals.iter().enumerate().take(20) {
                                    matrix[i] = *v;
                                }
                                primitives.push(FilterPrimitive::ColorMatrix(matrix));
                            },
                        }
                    },
                    "feOffset" => {
                        let dx = prim_get_float("dx", 0.0);
                        let dy = prim_get_float("dy", 0.0);
                        primitives.push(FilterPrimitive::Offset(dx, dy));
                    },
                    "feFlood" => {
                        let flood_color_str =
                            prim_get("flood-color").unwrap_or_else(|| "black".to_owned());
                        let (r, g, b, a) = parse_color(&flood_color_str);
                        let flood_opacity = prim_get_float("flood-opacity", 1.0);
                        primitives.push(FilterPrimitive::Flood(r, g, b, a * flood_opacity));
                    },
                    "feComposite" => {
                        let operator = prim_get("operator").unwrap_or_else(|| "over".to_owned());
                        let composite = match operator.trim() {
                            "arithmetic" => {
                                let k1 = prim_get_float("k1", 0.0);
                                let k2 = prim_get_float("k2", 0.0);
                                let k3 = prim_get_float("k3", 0.0);
                                let k4 = prim_get_float("k4", 0.0);
                                FeCompositeKind::Arithmetic { k1, k2, k3, k4 }
                            },
                            "in" => FeCompositeKind::In,
                            "out" => FeCompositeKind::Out,
                            "atop" => FeCompositeKind::Atop,
                            "xor" => FeCompositeKind::Xor,
                            "lighter" => FeCompositeKind::Lighter,
                            _ => FeCompositeKind::Over,
                        };
                        primitives.push(FilterPrimitive::Composite(composite));
                    },
                    "feTile" => {
                        primitives.push(FilterPrimitive::Tile);
                    },
                    _ => {},
                }
            }
        }
        if !primitives.is_empty() {
            Some((
                id,
                FilterDef {
                    primitives,
                    x,
                    y,
                    width,
                    height,
                },
            ))
        } else {
            None
        }
    }
}

fn parse_color(input: &str) -> (f32, f32, f32, f32) {
    let s = input.trim().to_lowercase();
    if let Some((r, g, b)) = parse_named_color(&s) {
        return (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
    }
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if s.starts_with("rgb") {
        return parse_rgb_color(&s);
    }
    (0.0, 0.0, 0.0, 1.0)
}

fn parse_hex_color(hex: &str) -> (f32, f32, f32, f32) {
    let hex: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0);
        (r as f32 / 15.0, g as f32 / 15.0, b as f32 / 15.0, 1.0)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    }
}

fn parse_rgb_color(input: &str) -> (f32, f32, f32, f32) {
    let start = input.find('(').unwrap_or(0);
    let end = input.find(')').unwrap_or(input.len());
    let inner = &input[start + 1..end];
    let parts: Vec<f32> = inner
        .split(|c: char| c == ',' || c == '/' || c.is_ascii_whitespace())
        .filter_map(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<f32>().ok()
            }
        })
        .collect();
    let r = *parts.first().unwrap_or(&0.0) /
        if input.starts_with("rgba") {
            255.0
        } else {
            1.0
        };
    let g = *parts.get(1).unwrap_or(&0.0) /
        if input.starts_with("rgba") {
            255.0
        } else {
            1.0
        };
    let b = *parts.get(2).unwrap_or(&0.0) /
        if input.starts_with("rgba") {
            255.0
        } else {
            1.0
        };
    let a = *parts.get(3).unwrap_or(&1.0);
    let max_rgb = if input.starts_with("rgba") {
        255.0
    } else {
        1.0
    };
    (r / max_rgb, g / max_rgb, b / max_rgb, a.clamp(0.0, 1.0))
}

fn parse_named_color(name: &str) -> Option<(u8, u8, u8)> {
    match name {
        "black" => Some((0, 0, 0)),
        "white" => Some((255, 255, 255)),
        "red" => Some((255, 0, 0)),
        "green" => Some((0, 128, 0)),
        "blue" => Some((0, 0, 255)),
        "yellow" => Some((255, 255, 0)),
        "cyan" | "aqua" => Some((0, 255, 255)),
        "magenta" | "fuchsia" => Some((255, 0, 255)),
        "gray" | "grey" => Some((128, 128, 128)),
        "silver" => Some((192, 192, 192)),
        "maroon" => Some((128, 0, 0)),
        "purple" => Some((128, 0, 128)),
        "teal" => Some((0, 128, 128)),
        "navy" => Some((0, 0, 128)),
        "lime" => Some((0, 255, 0)),
        "orange" => Some((255, 165, 0)),
        "pink" => Some((255, 192, 203)),
        "transparent" => Some((0, 0, 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::GradientParser;

    #[test]
    fn gradient_parser_collects_both_linear_and_radial_tags() {
        let tags = GradientParser::tag_names();
        assert!(tags.contains(&"linearGradient"));
        assert!(tags.contains(&"radialGradient"));
    }
}
