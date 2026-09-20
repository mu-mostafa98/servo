/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Thin per-element builders that assemble a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! Each `build_*` function reads the small bit of per-element state it needs and
//! delegates the parsing to [`crate::svg::primitives`], then constructs the
//! resulting [`usvg::Node`](s). This phase covers basic shapes inside
//! `<svg>`/`<g>` containers only.

mod group;
mod path;

use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_arc::Arc as ServoArc;
use style::dom::TNode;
use style::properties::ComputedValues;

use self::group::build_group;
use self::path::build_shape;
use crate::context::LayoutContext;
use crate::svg::primitives::attrs::{element_layout_type, is_group_element, resolve_size};
use crate::svg::primitives::geometry::normalized_diagonal;

/// The shared state every builder needs to build its [`usvg::Node`].
///
/// `'a` borrows the surrounding build scope (`LayoutContext`).
pub(crate) struct SvgContext<'a> {
    pub(crate) context: &'a LayoutContext<'a>,
    pub(crate) diagonal: f32,
}

impl SvgContext<'_> {
    /// The computed style of `element`, or `None` when the element is unstyled.
    pub(crate) fn computed_style(
        &self,
        element: &ServoLayoutElement<'_>,
    ) -> Option<ServoArc<ComputedValues>> {
        element
            .style_data()
            .is_some()
            .then(|| element.as_node().style(&self.context.style_context))
    }

    /// The element's group opacity, defaulting to fully opaque when `computed` is
    /// `None` (unstyled) or the computed opacity is not a valid normalised value.
    pub(crate) fn opacity(&self, computed: Option<&ComputedValues>) -> usvg::Opacity {
        computed
            .and_then(|c| usvg::Opacity::new(c.get_effects().opacity))
            .unwrap_or(usvg::Opacity::ONE)
    }
}

/// Builds a [`usvg::Tree`] from the `<svg>` element at `node`.
///
/// Returns `None` when `node` is not an `<svg>` element or the tree would be
/// empty/invalid.
#[expect(unsafe_code)]
pub(crate) fn build_usvg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<usvg::Tree> {
    let element = node.as_element()?;
    if element_layout_type(&element) != LayoutElementType::SVGSVGElement {
        return None;
    }

    let size = resolve_size(&element)?;

    // Reference length for `<percentage>` stroke-width/dash values: the SVG
    // "normalized diagonal" of the viewport, √(w² + h²) / √2.
    let diagonal = normalized_diagonal(size);

    let ctx = SvgContext { context, diagonal };

    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        if let Some(child_node) = build_usvg_node(child, &ctx) {
            root.push_child(child_node);
        }
    }

    let mut tree = usvg::Tree::new(size, root);
    tree.finalize();
    Some(tree)
}

/// Converts a DOM node (and its subtree) into a single [`usvg::Node`].
///
/// A shape produces a single node (or a carrying group when it has an opacity
/// below 1.0), so a single [`Option`] suffices in this phase.
pub(crate) fn build_usvg_node<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a>,
) -> Option<usvg::Node> {
    let Some(element) = node.as_element() else {
        return None;
    };
    let ty = element_layout_type(&element);

    // A nested `<svg>` is treated as a plain group in this phase: its children
    // render with the container's opacity, but its viewport
    // (`viewBox`/`x`/`y`/`width`/`height`) semantics are not yet modelled.
    if is_group_element(ty) {
        return build_group(node, ctx);
    }

    let computed = ctx.computed_style(&element);

    build_shape(&element, ty, computed.as_deref(), ctx)
}
