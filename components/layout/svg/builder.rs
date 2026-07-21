/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::sync::Arc;
use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use svg_engine::shapes::Shape;
use web_atoms::ns;

use crate::context::LayoutContext;

pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    _context: &'a LayoutContext<'a>,
    // TODO: css_rules
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        SvgRenderTreeBuilder {
            root_node: node,
            _context: context,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new())?;
        // TODO: extract viewport info, gradients, clip_paths, patterns, masks, filters

        let tree = SvgRenderTree {
            root,
            // TODO: viewport, gradients, clip_paths, patterns, masks, filters
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
            return build_text_node(node);
        }
        
        let tag = build_tag(node)?;
        let id = extract_id(&element);
        let children = resolve_children(node, &tag, root_node, self, resolving);
        
        // TODO: implement style from computed values + presentation attributes

        Some(SvgRenderNode {
            id,
            tag,
            // TODO: style, transforms
            children,
        })
    }
}

fn build_text_node(node: ServoLayoutNode) -> Option<SvgRenderNode> {
    use svg_engine::TextSpan;
    let element = node.as_element()?;
    let text = extract_text_content(node);
    if text.is_empty() {
        return None;
    }
    let tag = SvgTag::Text(TextSpan { text });
    let id = extract_id(&element);
    Some(SvgRenderNode {
        id,
        tag,
        // TODO: style, transforms
        children: vec![],
    })
}

fn extract_text_content(node: ServoLayoutNode) -> String {
    let mut text = String::new();
    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == "tspan" {
                text.push_str(&extract_text_content(child));
            }
        } else {
            text.push_str(&child.text_content());
        }
    }
    text
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

fn resolve_use_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgRenderTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    let element = node.as_element().unwrap();

    let ref_id = element
        .attribute_as_str(&ns!(), &local_name!("href"))
        .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
        .and_then(|h| {
            let t = h.trim_start_matches('#');
            if t.is_empty() { None } else { Some(t.to_owned()) }
        });

    let Some(ref_id) = ref_id else { return vec![] };
    if resolving.contains(&ref_id) {
        return vec![];
    }
    resolving.insert(ref_id.clone());

    // TODO: parse x/y offset, apply translate transform

    let result = find_element_by_id(root_node, &ref_id)
        .and_then(|target| builder.build_render_node(target, root_node, resolving))
        .map(|target_node| {
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

// ======================= Tag Dispatch =======================

fn build_tag(node: ServoLayoutNode) -> Option<SvgTag> {
    let element = node.as_element()?;
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "image" => build_image_tag(&element).map(SvgTag::Image),
        _ => build_shape(tag).map(SvgTag::Shape),
    }
}

fn build_image_tag(element: &ServoLayoutElement) -> Option<SvgImage> {
    use svg_engine::SvgImage;
    let get = |attr: &str| {
        element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string())
    };
    let get_xlink = |attr: &str| {
        element.attribute_as_str(&ns!(xlink), &LocalName::from(attr)).map(|s| s.to_string())
    };

    // TODO: Parse x, y, width, height attributes

    Some(SvgImage {
        href: get("href").or_else(|| get_xlink("href")),
    })
}

fn build_shape(tag_name: &str) -> Option<Shape> {
    use svg_engine::shapes::*;
    match tag_name {
        // TODO: Parse Geometry for each shape type in separate functions
        "rect" => Some(Shape::Rect(Rectangle {})),
        "circle" => Some(Shape::Circle(Circle {})),
        "ellipse" => Some(Shape::Ellipse(Ellipse {})),
        "line" => Some(Shape::Line(Line {})),
        "polyline" => Some(Shape::Polyline(Polyline {})),
        "polygon" => Some(Shape::Polygon(Polygon {})),
        "path" => Some(Shape::Path(Path {})),
        _ => None,
    }
}

// ======================= Helpers =======================

fn extract_id(element: &ServoLayoutElement) -> Option<String> {
    element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string())
}

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
