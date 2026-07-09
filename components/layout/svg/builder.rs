/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — assembles an [`SvgRenderTree`] from DOM nodes.
//!
//! Uses the **Builder pattern**: [`SvgRenderTreeBuilder`] accumulates state
//! (CSS rules, definition maps) through chained methods, then produces the
//! final tree via [`build`](SvgRenderTreeBuilder::build).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};

use svg_engine::render_tree::*;
use svg_engine::visitor::PaintServerFixupVisitor;

use web_atoms::ns;

use crate::context::LayoutContext;

use super::style::{
    collect_svg_css_rules,
    build_style,
};
use super::collects::{
    DefinitionCollector,
    GradientParser, ClipPathParser, PatternParser, MaskParser, FilterParser,
    build_shape_core,
    extract_viewport_info,
};

// ======================= Builder =======================

/// Builds an [`SvgRenderTree`] from a DOM SVG element.
///
/// Usage:
/// ```ignore
/// let tree = SvgRenderTreeBuilder::new(node, context)
///     .build();
/// ```
pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    css_rules: HashMap<String, HashMap<String, String>>,
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    /// Start building from an SVG DOM element node.
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        // Phase 1: Collect CSS rules from <style> elements (needed by build_style).
        let css_rules = collect_svg_css_rules(node);
        SvgRenderTreeBuilder { root_node: node, context, css_rules }
    }

    /// Build the complete [`SvgRenderTree`].
    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new())?;
        let viewport = extract_viewport_info(self.root_node, self.context);

        // Collect definitions using the Strategy pattern.
        let gradients = DefinitionCollector::collect::<GradientParser>(self.root_node, self.context);
        let clip_paths = DefinitionCollector::collect::<ClipPathParser>(self.root_node, self.context);
        let patterns = DefinitionCollector::collect::<PatternParser>(self.root_node, self.context);
        let masks = DefinitionCollector::collect::<MaskParser>(self.root_node, self.context);
        let filters = DefinitionCollector::collect::<FilterParser>(self.root_node, self.context);

        let mut tree = SvgRenderTree { root, viewport, gradients, clip_paths, patterns, masks, filters };

        // Post-process: convert PaintServer::Gradient → PaintServer::Pattern
        // when the referenced ID is actually a pattern definition.
        {
            let patterns = tree.patterns.clone();
            let mut visitor = PaintServerFixupVisitor { pattern_ids: &patterns };
            tree.visit_mut(&mut visitor);
        }

        Some(Arc::new(tree))
    }

    /// Recursively build a render node from a DOM node.
    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        resolving: &mut HashSet<String>,
    ) -> Option<SvgRenderNode> {
        let element = node.as_element()?;
        let tag = build_tag(&element)?;
        let style = build_style(node, self.context, &self.css_rules);
        let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string());

        // Resolve children, handling <use> element references.
        let children = match &tag {
            SvgTag::Container(Container::Use) => {
                let ref_id = element.attribute_as_str(&ns!(), &local_name!("href"))
                    .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
                    .and_then(|href| {
                        let trimmed = href.trim_start_matches('#');
                        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
                    });
                match ref_id {
                    Some(ref_id) if !resolving.contains(&ref_id) => {
                        resolving.insert(ref_id.clone());
                        let result = find_element_by_id(root_node, &ref_id)
                            .and_then(|target| self.build_render_node(target, root_node, resolving))
                            .map(|target_node| target_node.children)
                            .unwrap_or_default();
                        resolving.remove(&ref_id);
                        result
                    },
                    _ => vec![],
                }
            },
            _ => {
                node.dom_children()
                    .filter_map(|child| self.build_render_node(child, root_node, resolving))
                    .collect()
            },
        };

        Some(SvgRenderNode { id, tag, style, children })
    }
}

// ======================= Tag Dispatch =======================

fn build_tag<'dom>(element: &ServoLayoutElement<'dom>) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        _ => build_shape_core(element, tag).map(SvgTag::Shape),
    }
}

// ======================= Element Lookup =======================

/// Recursively search the SVG DOM subtree for an element by its `id`.
fn find_element_by_id<'dom>(node: ServoLayoutNode<'dom>, target_id: &str) -> Option<ServoLayoutNode<'dom>> {
    if let Some(element) = node.as_element() {
        if let Some(id) = element.attribute_as_str(&ns!(), &local_name!("id")) {
            if id == target_id {
                return Some(node);
            }
        }
    }
    for child in node.dom_children() {
        if let Some(found) = find_element_by_id(child, target_id) {
            return Some(found);
        }
    }
    None
}
