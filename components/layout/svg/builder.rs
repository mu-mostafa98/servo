/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;
use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use web_atoms::ns;

use super::collects::build_shape_core;
use super::style::build_style;
use crate::context::LayoutContext;

pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        SvgRenderTreeBuilder {
            root_node: node,
            context,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node)?;
        Some(Arc::new(SvgRenderTree { root }))
    }

    fn build_render_node(&self, node: ServoLayoutNode<'dom>) -> Option<SvgRenderNode> {
        let element = node.as_element()?;
        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));
        let tag = build_tag(&element, computed.as_ref().map(|v| &**v))?;
        let style = build_style(node, self.context);
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string());
        let children = node
            .dom_children()
            .filter_map(|child| self.build_render_node(child))
            .collect();

        Some(SvgRenderNode {
            id,
            tag,
            style,
            children,
        })
    }
}

fn build_tag<'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        _ => build_shape_core(element, tag, computed).map(SvgTag::Shape),
    }
}
