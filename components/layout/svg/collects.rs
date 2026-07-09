/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG definition collection — extracts gradients, clip-paths, patterns,
//! masks, and filters from `<defs>` containers.
//!
//! Uses the **Strategy pattern** via [`DefinitionParser`] to eliminate
//! duplicated traversal code — each definition type implements the trait
//! and [`DefinitionCollector`] handles the common recursion.

use std::collections::HashMap;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};

use svg_engine::render_tree::*;
use svg_engine::shapes::*;
use svg_engine::shapes::BuildFromElement;
use svg_engine::style::gradient::{GradientDef, parse_gradient_element};

use web_atoms::ns;

use crate::context::LayoutContext;

use super::style::{get_attr, build_style_from_attrs};

// ======================= Strategy Pattern =======================

/// Trait implemented by each definition type to define how it's parsed
/// from a DOM element. The [`DefinitionCollector`] handles the common
/// traversal and collection logic.
pub(crate) trait DefinitionParser {
    /// The type of the parsed definition.
    type Definition;
    /// The SVG tag names to search for (e.g. `{"linearGradient", "radialGradient"}`).
    fn tag_names() -> &'static [&'static str];
    /// Parse a definition from a DOM element node. Returns `(id_attr_value, definition)`.
    fn parse(node: ServoLayoutNode, context: &LayoutContext) -> Option<(String, Self::Definition)>;
}

/// Generic collector that walks `<defs>` containers and collects definitions
/// using the provided [`DefinitionParser`].
pub(crate) struct DefinitionCollector;

impl DefinitionCollector {
    pub(crate) fn collect<T: DefinitionParser>(
        node: ServoLayoutNode,
        context: &LayoutContext,
    ) -> HashMap<String, T::Definition> {
        let mut result = HashMap::new();
        let mut candidates = Vec::new();
        // Walk direct children of `<svg>` looking for `<defs>`.
        for defs_child in node.dom_children() {
            if let Some(defs_elem) = defs_child.as_element() {
                if defs_elem.local_name() == &local_name!("defs") {
                    // Deep-recursive search inside `<defs>` for each target tag.
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

/// Recursively search a DOM subtree for SVG elements with the given local name.
/// Handles nested groups inside `<defs>`.
pub(crate) fn find_elements_by_tag<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &str,
    result: &mut Vec<ServoLayoutNode<'dom>>,
) {
    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == tag {
                result.push(child);
            }
            // Recurse into containers to handle nesting.
            let name = elem.local_name().as_ref();
            if name == "g" || name == "defs" || name == "svg" || name == "a" || name == "switch" {
                find_elements_by_tag(child, tag, result);
            }
        }
    }
}

// ======================= Gradient Parser =======================

pub(crate) struct GradientParser;

impl DefinitionParser for GradientParser {
    type Definition = GradientDef;
    fn tag_names() -> &'static [&'static str] { &["linearGradient", "radialGradient"] }

    fn parse(node: ServoLayoutNode, _context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let grad_name = element.local_name().as_ref().to_owned();
        if grad_name != "linearGradient" && grad_name != "radialGradient" {
            return None;
        }
        let mut stop_attrs: Vec<Vec<(String, String)>> = Vec::new();
        // Collect <stop> children.
        for stop_node in node.dom_children() {
            if let Some(stop_elem) = stop_node.as_element() {
                if stop_elem.local_name() == &local_name!("stop") {
                    let mut attrs: Vec<(String, String)> = Vec::new();
                    if let Some(offset) = stop_elem.attribute_as_str(&ns!(), &local_name!("offset")) {
                        attrs.push(("offset".to_owned(), offset.to_string()));
                    }
                    if let Some(color) = stop_elem.attribute_as_str(&ns!(), &local_name!("stop-color")) {
                        attrs.push(("stop-color".to_owned(), color.to_string()));
                    }
                    if let Some(op) = stop_elem.attribute_as_str(&ns!(), &local_name!("stop-opacity")) {
                        attrs.push(("stop-opacity".to_owned(), op.to_string()));
                    }
                    if !attrs.is_empty() { stop_attrs.push(attrs); }
                }
            }
        }
        let grad_get = |attr: &str| {
            element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string())
        };
        if let Ok(def) = parse_gradient_element(&grad_name, &grad_get, &stop_attrs) {
            match &def {
                GradientDef::Linear(lg) => { return Some((lg.id.clone(), def)); },
                GradientDef::Radial(rg) => { return Some((rg.id.clone(), def)); },
            }
        }
        None
    }
}

// ======================= ClipPath Parser =======================

pub(crate) struct ClipPathParser;

impl DefinitionParser for ClipPathParser {
    type Definition = ClipPathDef;
    fn tag_names() -> &'static [&'static str] { &["clipPath"] }

    fn parse(node: ServoLayoutNode, _context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string())?;
        let units = element.attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(ClipPathUnits::UserSpaceOnUse);
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                if let Some(shape) = build_shape_core(&child_elem, &tag_name) {
                    shapes.push(shape);
                }
            }
        }
        if !shapes.is_empty() {
            Some((id, ClipPathDef { shapes, clip_path_units: units }))
        } else {
            None
        }
    }
}

// ======================= Pattern Parser =======================

pub(crate) struct PatternParser;

impl DefinitionParser for PatternParser {
    type Definition = PatternDef;
    fn tag_names() -> &'static [&'static str] { &["pattern"] }

    fn parse(node: ServoLayoutNode, _context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string())?;
        let parse_attr = |attr: &str, default: f32| -> f32 {
            element.attribute_as_str(&ns!(), &LocalName::from(attr))
                .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(default)
        };
        let width = parse_attr("width", 0.0);
        let height = parse_attr("height", 0.0);
        if width <= 0.0 || height <= 0.0 { return None; }
        let x = parse_attr("x", 0.0);
        let y = parse_attr("y", 0.0);
        let pattern_units = element.attribute_as_str(&ns!(), &local_name!("patternUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(PatternUnits::UserSpaceOnUse);
        let pattern_content_units = element.attribute_as_str(&ns!(), &local_name!("patternContentUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternContentUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(PatternContentUnits::UserSpaceOnUse);
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                if let Some(shape) = build_shape_core(&child_elem, &tag_name) {
                    let style = build_style_from_attrs(&child_elem);
                    shapes.push((shape, style));
                }
            }
        }
        if !shapes.is_empty() {
            Some((id, PatternDef {
                width, height, x, y,
                pattern_units, pattern_content_units,
                shapes,
            }))
        } else {
            None
        }
    }
}

// ======================= Mask Parser =======================

pub(crate) struct MaskParser;

impl DefinitionParser for MaskParser {
    type Definition = MaskDef;
    fn tag_names() -> &'static [&'static str] { &["mask"] }

    fn parse(node: ServoLayoutNode, _context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string())?;
        let mut shapes = Vec::new();
        for child_node in node.dom_children() {
            if let Some(child_elem) = child_node.as_element() {
                let tag_name = child_elem.local_name().as_ref().to_owned();
                if let Some(shape) = build_shape_core(&child_elem, &tag_name) {
                    let style = build_style_from_attrs(&child_elem);
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

// ======================= Filter Parser =======================

pub(crate) struct FilterParser;

impl DefinitionParser for FilterParser {
    type Definition = FilterDef;
    fn tag_names() -> &'static [&'static str] { &["filter"] }

    fn parse(node: ServoLayoutNode, _context: &LayoutContext) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string())?;
        let get = |attr: &str| element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
        let get_float = |attr: &str, default: f32| -> f32 {
            get(attr).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default)
        };
        let x = get_float("x", -0.1);
        let y = get_float("y", -0.1);
        let width = get_float("width", 1.2);
        let height = get_float("height", 1.2);

        let mut primitives = Vec::new();
        for prim_child in node.dom_children() {
            if let Some(prim_elem) = prim_child.as_element() {
                let pname = prim_elem.local_name().as_ref().to_owned();
                // Read primitive-specific attributes from the child element, not the <filter> parent.
                let prim_get = |attr: &str| prim_elem.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
                let prim_get_float = |attr: &str, default: f32| -> f32 {
                    prim_get(attr).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default)
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
                        primitives.push(FilterPrimitive::DropShadow(dx, dy, std_dev, 0.0, 0.0, 0.0, 0.5));
                    },
                    "feColorMatrix" => {
                        let v = 1.0 / 3.0;
                        primitives.push(FilterPrimitive::ColorMatrix([
                            v, v, v, 0.0, 0.0,
                            v, v, v, 0.0, 0.0,
                            v, v, v, 0.0, 0.0,
                            0.0, 0.0, 0.0, 1.0, 0.0,
                        ]));
                    },
                    _ => {},
                }
            }
        }
        if !primitives.is_empty() {
            Some((id, FilterDef { primitives, x, y, width, height }))
        } else {
            None
        }
    }
}

// ======================= Viewport Extraction =======================

/// Extract viewport info from the `<svg>` element.
pub(crate) fn extract_viewport_info<'dom>(node: ServoLayoutNode<'dom>, _context: &LayoutContext) -> ViewportInfo {
    let element = node.as_element().unwrap();
    let get = |attr: &str| element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
    let svg_width = get("width").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(300.0);
    let svg_height = get("height").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(150.0);
    let view_box = get("viewBox").as_deref().and_then(extract_viewbox);

    let overflow_visible = get("overflow")
        .or_else(|| {
            get("style").and_then(|s| {
                for part in s.split(';') {
                    let mut kv = part.splitn(2, ':');
                    let key = kv.next()?.trim();
                    let val = kv.next()?.trim();
                    if key.eq_ignore_ascii_case("overflow") {
                        return Some(val.to_owned());
                    }
                }
                None
            })
        })
        .map_or(false, |v| v.trim().eq_ignore_ascii_case("visible"));

    let aspect_ratio = get("preserveAspectRatio")
        .map(|v| parse_aspect_ratio(&v));

    ViewportInfo { width: svg_width, height: svg_height, view_box, overflow_visible, aspect_ratio }
}

// ======================= Shared Shape Construction =======================

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

/// Build a [`Shape`] from a DOM element using the [`BuildFromElement`] factory trait.
/// Used both by definition parsers and the main render tree builder.
pub(crate) fn build_shape_core(element: &ServoLayoutElement, tag_name: &str) -> Option<Shape> {
    let fs = SVG_DEFAULT_FONT_SIZE;
    let attrs = |name: &str| get_attr(element, name);
    match tag_name {
        "rect" => Rectangle::from_attrs(fs, &attrs).map(Shape::Rect),
        "circle" => Circle::from_attrs(fs, &attrs).map(Shape::Circle),
        "ellipse" => Ellipse::from_attrs(fs, &attrs).map(Shape::Ellipse),
        "line" => Line::from_attrs(fs, &attrs).map(Shape::Line),
        "polyline" => Polyline::from_attrs(fs, &attrs).map(Shape::Polyline),
        "polygon" => Polygon::from_attrs(fs, &attrs).map(Shape::Polygon),
        "path" => Path::from_attrs(fs, &attrs).map(Shape::Path),
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

// The `from_element_for_layout` constructor on ServoLayoutNode is needed
// by the definition parsers. We declare it as an unsafe extension here
// since the parsers have access to a &ServoLayoutElement and need to
// reconstruct the parent node for child iteration.
//
// Safety: The returned node is valid only during the current layout pass.
