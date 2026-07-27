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
//! | [`geometry`] | Shape construction — DOM elements → [`Shape`] |
//! | [`style`] | Style construction — [`ComputedValues`] → [`NodeStyle`] |
//! | [`css`] | Inline `<style>` CSS rule parsing |
//! | [`defines`] | Definition collection — gradients, clip-paths, etc. (Strategy pattern) |
//! | [`viewport`] | Viewport/viewBox/aspectRatio extraction |
//! | [`transforms`] | CSS/SVG transform conversion |
//!
//! The main entry point is [`build_svg_render_tree`], called from
//! [`crate::replaced`].

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

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<SvgRenderTree>> {
    builder::SvgRenderTreeBuilder::new(node, context).build()
}
