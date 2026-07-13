/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — public API surface.
//!
//! This module bridges Servo's DOM and style system with the SVG engine's
//! render tree types.  Internal concerns are split into three submodules:
//!
//! | Module | Role |
//! |--------|------|
//! | [`style`] | CSS rule parsing, style construction, presentation attributes |
//! | [`collects`] | Definition collection (gradients, clip-paths, …) — Strategy pattern |
//! | [`builder`] | Render tree assembly — Builder pattern |
//!
//! The main entry point is [`build_svg_render_tree`], called from
//! [`crate::replaced`].

pub(crate) mod builder;
pub(crate) mod collects;
pub(crate) mod style;

use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;
use svg_engine::render_tree::SvgRenderTree;

use crate::context::LayoutContext;

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
///
/// Delegates to [`builder::SvgRenderTreeBuilder`] which handles CSS collection,
/// definition collection, shape construction, and post-processing via the
/// Visitor pattern (PaintServerFixupVisitor).
pub(crate) fn build_svg_render_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgRenderTree>> {
    builder::SvgRenderTreeBuilder::new(node, context).build()
}
