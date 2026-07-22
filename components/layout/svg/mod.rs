/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod builder;
pub(crate) mod css;
pub(crate) mod defines;
pub(crate) mod geometry;
pub(crate) mod style;
pub(crate) mod transforms;
pub(crate) mod viewport;

use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;
use svg_engine::render_tree::SvgRenderTree;

use crate::context::LayoutContext;

pub(crate) fn build_svg_render_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgRenderTree>> {
    builder::SvgRenderTreeBuilder::new(node, context).build()
}
