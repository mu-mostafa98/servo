/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use layout_api::LayoutNode;
use resvg::usvg;
use script::layout_dom::ServoLayoutNode;

use super::{SvgContext, build_usvg_node};
use crate::svg::primitives::attrs::element_id;

pub(super) fn build_group<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = ctx.computed_style(&element);

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();
    group.opacity = ctx.opacity(computed.as_deref());

    for child in node.dom_children() {
        if let Some(child_node) = build_usvg_node(child, ctx) {
            group.push_child(child_node);
        }
    }

    Some(usvg::Node::Group(Box::new(group)))
}
