/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod group;
mod path;

use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_arc::Arc as ServoArc;
use style::properties::ComputedValues;

use self::group::build_group;
use self::path::build_shape;
use crate::context::LayoutContext;
use crate::svg::primitives::attrs::{element_layout_type, is_group_element, resolve_size};
use crate::svg::primitives::geometry::normalized_diagonal;

pub(crate) struct SvgContext<'a> {
    pub(crate) context: &'a LayoutContext<'a>,
    pub(crate) diagonal: f32,
}

impl SvgContext<'_> {
    pub(crate) fn computed_style(
        &self,
        element: &ServoLayoutElement<'_>,
    ) -> Option<ServoArc<ComputedValues>> {
        element
            .style_data()
            .is_some()
            .then(|| element.as_node().style(&self.context.style_context))
    }

    pub(crate) fn opacity(&self, computed: Option<&ComputedValues>) -> usvg::Opacity {
        computed
            .and_then(|c| usvg::Opacity::new(c.get_effects().opacity))
            .unwrap_or(usvg::Opacity::ONE)
    }
}

pub(crate) fn build_usvg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<usvg::Tree> {
    let element = node.as_element()?;
    if element_layout_type(&element) != LayoutElementType::SVGSVGElement {
        return None;
    }

    let size = resolve_size(&element)?;
    let diagonal = normalized_diagonal(size);
    let ctx = SvgContext { context, diagonal };

    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        if let Some(child_node) = build_usvg_node(child, &ctx) {
            root.push_child(child_node);
        }
    }

    let tree = usvg::Tree::new(size, root);
    Some(tree)
}

pub(crate) fn build_usvg_node<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a>,
) -> Option<usvg::Node> {
    let Some(element) = node.as_element() else {
        return None;
    };
    let ty = element_layout_type(&element);

    if is_group_element(ty) {
        return build_group(node, ctx);
    }

    let computed = ctx.computed_style(&element);

    build_shape(&element, ty, computed.as_deref(), ctx)
}
