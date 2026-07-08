/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — facade module.
//!
//! This module is the public API surface for building SVG render trees.
//! The actual construction logic is split across:
//!
//! - [`svg_style_builder`]: CSS rule parsing, style construction
//! - [`svg_definition_collector`]: Definition collection (gradients, clip-paths, etc.)
//! - [`svg_tree_builder`]: Render tree assembly (Builder pattern)
//!
//! Keeping this file as a thin re-export avoids breaking callers that
//! use `crate::svg_builder::build_svg_render_tree`.

use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;

use svg_engine::render_tree::SvgRenderTree;

use crate::context::LayoutContext;
use crate::svg_tree_builder::SvgRenderTreeBuilder;

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
///
/// Delegates to [`SvgRenderTreeBuilder`] which handles CSS collection,
/// definition collection, shape construction, and post-processing via
/// the Visitor pattern (PaintServerFixupVisitor).
pub(crate) fn build_svg_render_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgRenderTree>> {
    SvgRenderTreeBuilder::new(node, context).build()
}
