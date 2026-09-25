/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG definition collection — extracts gradients, clip-paths, patterns,
//! masks, filters, and markers from `<defs>` containers.
//!
//! Uses the **Strategy pattern**: [`DefinitionParser`] defines how each
//! definition type is parsed, and [`DefinitionCollector`] handles the
//! common recursion and collection logic. The per-type parsers live in the
//! sibling modules ([`gradient`], [`clip_path`], [`pattern`], [`mask`],
//! [`filter`], [`marker`]).

use std::collections::HashMap;
use std::sync::Arc;

use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::element::{Container, SvgNode, SvgTag};
use svg_engine::style::NodeStyle;

use crate::svg::builder::SvgTreeBuilder;

pub(crate) mod clip_path;
pub(crate) mod filter;
pub(crate) mod gradient;
pub(crate) mod marker;
pub(crate) mod mask;
pub(crate) mod pattern;

pub(crate) use clip_path::ClipPathParser;
pub(crate) use filter::FilterParser;
pub(crate) use gradient::{GradientParser, resolve_gradient_hrefs};
pub(crate) use marker::MarkerParser;
pub(crate) use mask::MaskParser;
pub(crate) use pattern::PatternParser;

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
    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)>;
}

/// Generic collector that walks `<defs>` containers and collects definitions
/// using the provided [`DefinitionParser`].
pub(crate) struct DefinitionCollector;

impl DefinitionCollector {
    pub(crate) fn collect<'dom, 'a, T: DefinitionParser>(
        node: ServoLayoutNode<'dom>,
        builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> HashMap<String, Arc<T::Definition>> {
        let mut result = HashMap::new();
        let mut candidates = Vec::new();
        // SVG definitions (gradients, clip paths, patterns, masks, filters,
        // markers) may appear anywhere in the document, not only inside `<defs>`.
        for tag in T::tag_names() {
            find_elements_by_tag(node, tag, &mut candidates);
        }
        for candidate_node in candidates {
            if candidate_node.as_element().is_some() {
                if let Some((id, def)) = T::parse(candidate_node, builder) {
                    result.insert(id, Arc::new(def));
                }
            }
        }
        result
    }
}

/// Recursively search a DOM subtree for SVG elements with the given local name.
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
            if name == "g"
                || name == "defs"
                || name == "svg"
                || name == "a"
                || name == "switch"
                || name == "symbol"
                || name == "marker"
                || name == "clipPath"
                || name == "mask"
                || name == "pattern"
            {
                find_elements_by_tag(child, tag, result);
            }
        }
    }
}

/// Build the child elements of a definition container (clip-path, pattern,
/// mask, marker) into full render nodes, recursively handling `<g>`, `<use>`,
/// `<text>` and nested `<defs>` content instead of flattening to shapes.
fn collect_def_content<'dom, 'a>(
    node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, 'a>,
) -> Vec<SvgNode> {
    node.dom_children()
        .filter_map(|child| builder.build_def_content(child))
        .collect()
}

/// Wrap definition children in a synthetic `<g>` root node.
fn def_content_root(children: Vec<SvgNode>) -> SvgNode {
    SvgNode {
        id: None,
        tag: SvgTag::Container(Container::Group),
        style: NodeStyle::default(),
        transforms: Vec::new(),
        viewport: None,
        children,
    }
}
