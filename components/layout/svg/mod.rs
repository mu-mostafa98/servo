/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — public API surface.
//!
//! Bridges Servo's DOM and style system with the SVG engine's render tree types.

pub(crate) mod builder;
pub(crate) mod geometry;
pub(crate) mod viewport;

// TODO: css, defines, style, transforms

use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;
use svg_engine::render_tree::SvgRenderTree;

use crate::context::LayoutContext;

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgRenderTree>> {
    builder::SvgRenderTreeBuilder::new(node, context).build()
}
