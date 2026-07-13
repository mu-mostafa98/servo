pub(crate) mod builder;
pub(crate) mod style;

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
