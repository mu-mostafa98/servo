/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — assembles an [`SvgRenderTree`] from DOM nodes.
//!
//! Uses the **Builder pattern**: [`SvgRenderTreeBuilder`] accumulates state
//! through chained methods, then produces the final tree via
//! [`build`](SvgRenderTreeBuilder::build).

use std::collections::HashSet;
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use web_atoms::ns;

use super::geometry::{build_shape, build_text, get_attr};
use super::viewport::extract_viewport_info;
use crate::context::LayoutContext;

// ======================= Builder =======================

/// Builds an [`SvgRenderTree`] from a DOM SVG element.
pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    // TODO: css_rules
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        // TODO: collect inline <style> CSS rules
        SvgRenderTreeBuilder {
            root_node: node,
            context,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new())?;
        let viewport = extract_viewport_info(self.root_node);

        // TODO: collect definition maps (gradients, clip-paths, patterns, masks, filters)
        // and run PaintServerFixupVisitor

        let tree = SvgRenderTree {
            root,
            viewport,
            // TODO: gradients, clip_paths, patterns, masks, filters
        };

        Some(Arc::new(tree))
    }

    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        resolving: &mut HashSet<String>,
    ) -> Option<SvgRenderNode> {
        let element = node.as_element()?;
        let tag_name = element.local_name().as_ref().to_owned();

        // Text / tspan — extract text content from DOM children.
        if tag_name == "text" || tag_name == "tspan" {
            return build_text_node(node, self.context);
        }

        // Resolve computed values for CSS-cascaded geometry properties
        // (x, y, cx, cy, r, rx, ry). Style and transforms are not built yet.
        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));

        let tag = build_tag(&element, computed.as_ref().map(|v| &**v))?;
        let id = extract_id(&element);
        let children = resolve_children(node, &tag, root_node, self, resolving);

        // TODO: build style from computed values + presentation attributes

        Some(SvgRenderNode {
            id,
            tag,
            // TODO: style, transforms
            children,
        })
    }
}

// ======================= Text / Tspan Node =======================

/// Build a [`SvgRenderNode`] for `<text>` or `<tspan>`.
///
/// TODO: implement text shaping via font subsystem for accurate glyph positioning
fn build_text_node(
    node: ServoLayoutNode,
    _context: &LayoutContext,
) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let fs: f32 = 16.0;
    let get = |name: &str| get_attr(&element, name);
    let span = build_text(node, &get, fs)?;

    // TODO: Shape text with the font subsystem (shape_text_span)

    let tag = SvgTag::Text(span);
    let id = extract_id(&element);

    Some(SvgRenderNode {
        id,
        tag,
        // TODO: style, transforms
        children: vec![],
    })
}

// ======================= Children Resolution =======================

fn resolve_children<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &SvgTag,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgRenderTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    if let SvgTag::Container(Container::Use) = tag {
        resolve_use_children(node, root_node, builder, resolving)
    } else {
        node.dom_children()
            .filter_map(|child| builder.build_render_node(child, root_node, resolving))
            .collect()
    }
}

/// Resolve children for a `<use>` element.
fn resolve_use_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgRenderTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    let element = node.as_element().unwrap();

    // Extract href reference.
    let ref_id = element
        .attribute_as_str(&ns!(), &local_name!("href"))
        .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
        .and_then(|h| {
            let t = h.trim_start_matches('#');
            if t.is_empty() {
                None
            } else {
                Some(t.to_owned())
            }
        });

    let Some(ref_id) = ref_id else { return vec![] };
    if resolving.contains(&ref_id) {
        return vec![];
    }
    resolving.insert(ref_id.clone());

    // Parse x/y offset.
    let parse_coord = |attr: &str| -> Option<f32> {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
    };
    let _offset = (parse_coord("x"), parse_coord("y"));

    // Build target and clone.
    let result = find_element_by_id(root_node, &ref_id)
        .and_then(|target| builder.build_render_node(target, root_node, resolving))
        .map(|target_node| {
            // TODO: implement <use> x/y offset as translate transform
            // (requires transforms + TransformOp)

            // <symbol> is never rendered directly — unwrap its children
            // so they render when referenced via <use>.
            if let SvgTag::Container(Container::Symbol) = &target_node.tag {
                // TODO: apply x/y translation offset to symbol children
                return target_node.children;
            }
            vec![target_node]
        })
        .unwrap_or_default();

    resolving.remove(&ref_id);
    result
}

// TODO: DefinitionMaps, collect_definitions, shape_text_span

// ======================= Tag Dispatch =======================

fn build_tag<'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "image" => build_image_tag(element).map(SvgTag::Image),
        _ => build_shape(element, tag, computed).map(SvgTag::Shape),
    }
}

fn build_image_tag(element: &ServoLayoutElement) -> Option<SvgImage> {
    use svg_engine::attr_parsers::parse_length;
    let fs = 16.0;
    let get = |name: &str| get_attr(element, name);
    let x = parse_length("x", &get, fs).unwrap_or(0.0);
    let y = parse_length("y", &get, fs).unwrap_or(0.0);
    let w = parse_length("width", &get, fs).unwrap_or(0.0).max(0.0);
    let h = parse_length("height", &get, fs).unwrap_or(0.0).max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let get_xlink = |name: &str| {
        element.attribute_as_str(&ns!(xlink), &LocalName::from(name)).map(|s| s.to_string())
    };
    let href = get("href").or_else(|| get_xlink("href"));
    Some(SvgImage {
        x,
        y,
        width: w,
        height: h,
        href,
    })
}

// ======================= Helpers =======================

fn extract_id(element: &ServoLayoutElement) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &local_name!("id"))
        .map(|s| s.to_string())
}

/// Recursively search the SVG DOM subtree for an element by its `id`.
fn find_element_by_id<'dom>(
    node: ServoLayoutNode<'dom>,
    target_id: &str,
) -> Option<ServoLayoutNode<'dom>> {
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
