/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — public API surface.
//!
//! This module bridges Servo's DOM and style system with the SVG engine's
//! render tree types.
//!
//! # Module Map
//!
//! | Module | Role |
//! |--------|------|
//! | [`builder`] | Orchestrator — assembles the render tree (Builder pattern) |
//! | [`defines`] | Definition collection — gradients, clip-paths, etc. (Strategy pattern) |
//! | [`primitives`] | Leaf parsers — attributes, geometry, text, paint, CSS, viewport, transforms |
//! | [`style`] | Style construction — [`ComputedValues`] → [`NodeStyle`] |
//!
//! The main entry point is [`build_svg_tree`], called from
//! [`crate::replaced`].

pub(crate) mod builder;
pub(crate) mod defines;
pub(crate) mod primitives;
pub(crate) mod style;

use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;
use svg_engine::document::SvgTree;

use crate::context::LayoutContext;

/// Main entry point — builds a complete `SvgTree` from an SVG DOM element.
pub(crate) fn build_svg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgTree>> {
    builder::SvgTreeBuilder::new(node, context).build()
}
