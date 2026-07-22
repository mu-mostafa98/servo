/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use svg_engine::style::gradient::GradientDef;
use svg_engine::visitor::PaintServerFixupVisitor;
use web_atoms::ns;

use super::css::collect_svg_css_rules;
use super::defines::{
    ClipPathParser, DefinitionCollector, FilterParser, GradientParser, MaskParser, PatternParser,
};
use super::geometry::build_shape;
use super::style::build_style;
use super::viewport::extract_viewport_info;
use crate::context::LayoutContext;

pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    css_rules: HashMap<String, HashMap<String, String>>,
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        let css_rules = collect_svg_css_rules(node);
        SvgRenderTreeBuilder {
            root_node: node,
            context,
            css_rules,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new())?;
        let viewport = extract_viewport_info(self.root_node);
        let definitions = collect_definitions(self.root_node, self.context);

        let mut tree = SvgRenderTree {
            root,
            viewport,
            gradients: definitions.gradients,
            clip_paths: definitions.clip_paths,
            patterns: definitions.patterns,
            masks: definitions.masks,
            filters: definitions.filters,
        };

        let patterns = tree.patterns.clone();
        let mut visitor = PaintServerFixupVisitor {
            pattern_ids: &patterns,
        };
        tree.visit_mut(&mut visitor);

        Some(Arc::new(tree))
    }

    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        resolving: &mut HashSet<String>,
    ) -> Option<SvgRenderNode> {
        let element = node.as_element()?;

        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));
        let tag = build_tag(&element, computed.as_ref().map(|v| &**v))?;
        let (style, transforms) = build_style(node, self.context, &self.css_rules);
        let id = extract_id(&element);
        let children = resolve_children(node, &tag, root_node, self, resolving);

        Some(SvgRenderNode {
            id,
            tag,
            style,
            transforms,
            children,
        })
    }
}

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

    let parse_coord = |attr: &str| -> Option<f32> {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
    };
    let offset = (parse_coord("x"), parse_coord("y"));

    let result = find_element_by_id(root_node, &ref_id)
        .and_then(|target| builder.build_render_node(target, root_node, resolving))
        .map(|target_node| {
            let apply_offset = |node: &mut SvgRenderNode| {
                if let (Some(dx), Some(dy)) = offset {
                    if dx != 0.0 || dy != 0.0 {
                        node.transforms.insert(
                            0,
                            svg_engine::style::transform_ops::TransformOp::Translate(dx, dy),
                        );
                    }
                }
            };

            if let SvgTag::Container(Container::Symbol) = &target_node.tag {
                let mut children = target_node.children;
                for child in &mut children {
                    apply_offset(child);
                }
                return children;
            }

            let mut cloned = target_node;
            apply_offset(&mut cloned);
            vec![cloned]
        })
        .unwrap_or_default();

    resolving.remove(&ref_id);
    result
}

struct DefinitionMaps {
    gradients: HashMap<String, GradientDef>,
    clip_paths: HashMap<String, ClipPathDef>,
    patterns: HashMap<String, PatternDef>,
    masks: HashMap<String, MaskDef>,
    filters: HashMap<String, FilterDef>,
}

fn collect_definitions<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> DefinitionMaps {
    DefinitionMaps {
        gradients: DefinitionCollector::collect::<GradientParser>(node, context),
        clip_paths: DefinitionCollector::collect::<ClipPathParser>(node, context),
        patterns: DefinitionCollector::collect::<PatternParser>(node, context),
        masks: DefinitionCollector::collect::<MaskParser>(node, context),
        filters: DefinitionCollector::collect::<FilterParser>(node, context),
    }
}

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
        _ => build_shape(element, tag, computed).map(SvgTag::Shape),
    }
}

fn extract_id(element: &ServoLayoutElement) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &local_name!("id"))
        .map(|s| s.to_string())
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
