/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Entry point and per-element converter facades.
//!
//! [`build_usvg_tree`] orchestrates the whole build; [`convert_node`] dispatches
//! each DOM node to the facade that assembles its [`usvg::Node`]. Every facade
//! takes a single [`SvgContext`] — the shared `LayoutContext`/paint-server/defs/
//! font state that used to be threaded as seven separate parameters — plus the
//! per-element scalars (`element`, `parent_abs_transform`, `host`).

mod group;
mod image;
mod shape;
mod svg;
pub(crate) mod text;
mod use_;

use std::collections::HashMap;
use std::sync::Arc;

use servo_arc::Arc as ServoArc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::dom::TNode;
use style::properties::ComputedValues;

use crate::context::LayoutContext;
use crate::svg::effects::paint::{build_pattern, collect_paint_servers, Gradients};
use crate::svg::primitives::attrs::{
    element_id, element_layout_type, parse_length_attr, parse_transform, parse_view_box,
};
use crate::svg::primitives::text::SvgFonts;

use group::convert_group;
use image::convert_image;
use shape::build_shape_node;
use svg::convert_svg;
use text::convert_text;
use use_::convert_use;

/// The shared state every converter needs to build its [`usvg::Node`].
///
/// `'a` borrows the surrounding build scope (`LayoutContext`, the collected
/// paint servers, the id→element def map and the font resolver); `'dom` is the
/// DOM lifetime of the elements in `defs`.
pub(crate) struct SvgContext<'a, 'dom> {
    pub(crate) context: &'a LayoutContext<'a>,
    pub(crate) gradients: &'a Gradients,
    pub(crate) defs: &'a HashMap<String, ServoLayoutElement<'dom>>,
    pub(crate) diagonal: f32,
    pub(crate) fonts: &'a SvgFonts,
}

impl SvgContext<'_, '_> {
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

    /// The element's `transform` attribute as a [`usvg::Transform`], or identity
    /// when absent/unparseable.
    pub(crate) fn transform_attr(&self, element: &ServoLayoutElement<'_>) -> usvg::Transform {
        element
            .attribute_as_str(&ns!(), &LocalName::from("transform"))
            .map(parse_transform)
            .unwrap_or_else(usvg::Transform::identity)
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
/// Returns the tree together with the parsed [`usvg::ViewBox`] (if any). The
/// viewBox transform is deliberately *not* baked into the tree: content stays in
/// viewBox coordinates, and the viewBox→device mapping is applied at raster time
/// (see [`crate::svg::rasterize_svg_tree`]) so `preserveAspectRatio` is honoured
/// uniformly rather than being distorted by a non-uniform CSS-box stretch.
///
/// Returns `None` when `node` is not an `<svg>` element or the tree would be
/// empty/invalid.
#[expect(unsafe_code)]
pub(crate) fn build_usvg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<(usvg::Tree, Option<usvg::ViewBox>)> {
    let element = node.as_element()?;
    if element_layout_type(&element) != LayoutElementType::SVGSVGElement {
        return None;
    }

    let (size, view_box) = resolve_size_and_view_box(&element)?;

    // Reference length for `<percentage>` stroke-width/dash values: the SVG
    // "normalized diagonal" of the viewport, √(w² + h²) / √2.
    let diagonal = normalized_diagonal(size);

    // Font database + resolver shared by every `<text>` subtree.
    let fonts = SvgFonts::new(context);

    // Collect every `id`'d element in the document so `<use href="#id">` can be
    // resolved to its referenced element (which may live in a sibling subtree).
    let document = unsafe { node.dangerous_style_node() }.owner_doc();
    let mut defs = HashMap::new();
    if let Some(root_element) = document.root_element() {
        collect_element_ids(root_element.as_node(), &mut defs);
    }

    // Collect paint-server definitions up front so that `fill`/`stroke`
    // referencing them can be resolved during the main walk. Gradients are
    // collected first; patterns are built second so their content can reference
    // the already-collected gradients.
    let mut gradients = Gradients::default();
    let mut pattern_elements = Vec::new();
    for child in node.dom_children() {
        collect_paint_servers(child, &mut gradients, &mut pattern_elements);
    }

    // Build each pattern and insert it immediately, so that a pattern whose
    // content references a *previously* built pattern (nested `<pattern>`) can
    // resolve it. `ctx` is scoped per element so its `&gradients` borrow is
    // dropped before the insertion mutates `gradients`.
    for element in &pattern_elements {
        if let Some(id) = element_id(element) {
            let pattern = {
                let ctx = SvgContext {
                    context,
                    gradients: &gradients,
                    defs: &defs,
                    diagonal,
                    fonts: &fonts,
                };
                build_pattern(element, &ctx)
            };
            if let Some(pattern) = pattern {
                gradients.pattern.insert(id, Arc::new(pattern));
            }
        }
    }

    let ctx = SvgContext {
        context,
        gradients: &gradients,
        defs: &defs,
        diagonal,
        fonts: &fonts,
    };

    // Children are built in viewBox (user) coordinates; the viewBox transform is
    // applied later, at raster time.
    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        for child_node in convert_node(child, &ctx, usvg::Transform::identity(), None) {
            root.push_child(child_node);
        }
    }

    let mut tree = usvg::Tree::new(size, root);
    tree.finalize();
    Some((tree, view_box))
}

/// Determines the image size and view box from the root `<svg>` element.
fn resolve_size_and_view_box(
    element: &ServoLayoutElement<'_>,
) -> Option<(usvg::Size, Option<usvg::ViewBox>)> {
    let view_box = parse_view_box(element);

    let width = element
        .attribute_as_str(&ns!(), &LocalName::from("width"))
        .and_then(parse_length_attr);
    let height = element
        .attribute_as_str(&ns!(), &LocalName::from("height"))
        .and_then(parse_length_attr);

    let size = match (width, height, view_box.map(|vb| vb.rect)) {
        (Some(w), Some(h), _) => usvg::Size::from_wh(w, h),
        (Some(w), None, Some(vb)) => usvg::Size::from_wh(w, vb.height() * w / vb.width()),
        (None, Some(h), Some(vb)) => usvg::Size::from_wh(vb.width() * h / vb.height(), h),
        (None, None, Some(vb)) => usvg::Size::from_wh(vb.width(), vb.height()),
        _ => usvg::Size::from_wh(100.0, 100.0),
    }?;

    Some((size, view_box))
}

/// The SVG "normalized diagonal" of a viewport, used as the reference length for
/// `<percentage>` values of `stroke-width`, `stroke-dasharray` and `stroke-dashoffset`.
fn normalized_diagonal(size: usvg::Size) -> f32 {
    (size.width() * size.width() + size.height() * size.height()).sqrt()
        / std::f32::consts::SQRT_2
}

/// Recursively collects every element with a non-empty `id` attribute into `map`,
/// keyed by that id. Used to resolve `<use href="#id">` references anywhere in the
/// document, not just within the current `<svg>` subtree.
fn collect_element_ids<'a>(
    node: ServoLayoutNode<'a>,
    map: &mut HashMap<String, ServoLayoutElement<'a>>,
) {
    if let Some(element) = node.as_element() &&
        let Some(id) = element_id(&element)
    {
        map.entry(id).or_insert(element);
    }
    for child in node.dom_children() {
        collect_element_ids(child, map);
    }
}

/// Whether `ty` is a group-like element that is converted by [`convert_group`].
fn is_group_element(ty: LayoutElementType) -> bool {
    matches!(
        ty,
        LayoutElementType::SVGSVGElement
            | LayoutElementType::SVGGElement
            | LayoutElementType::SVGAElement
            | LayoutElementType::SVGClipPathElement
            | LayoutElementType::SVGMaskElement
    )
}

/// Converts a DOM node (and its subtree) into a list of [`usvg::Node`]s.
///
/// A shape normally produces a single node, but a shape carrying markers emits the
/// path plus one group per placed marker (markers are siblings of the path in usvg,
/// not a child node). Callers must therefore push every returned node.
///
/// `ctx.defs` is a document-wide `id`→element map used to resolve `<use>` references.
/// `host` is the computed style of the enclosing `<use>` element (if any): it is
/// used as the inheritance parent for paint, since `<use>` shadow content inherits
/// from the `<use>` host rather than from its location in the `<defs>`.
pub(crate) fn convert_node<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Vec<usvg::Node> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let ty = element_layout_type(&element);

    // A nested `<svg>` establishes a new viewport (x/y + viewBox transform), so it
    // needs dedicated handling rather than the plain `<g>` group path.
    if ty == LayoutElementType::SVGSVGElement {
        return convert_svg(node, ctx, parent_abs_transform, host)
            .into_iter()
            .collect();
    }

    // `<defs>` and `<symbol>` are definition-only containers: their content never
    // renders directly, only when instantiated via `<use href="#id">`. Skip them in
    // the normal walk (they remain resolvable through the `defs` id map).
    if matches!(
        ty,
        LayoutElementType::SVGDefsElement | LayoutElementType::SVGSymbolElement
    ) {
        return Vec::new();
    }

    if is_group_element(ty) {
        return convert_group(node, ctx, parent_abs_transform, host)
            .into_iter()
            .collect();
    }

    if ty == LayoutElementType::SVGUseElement {
        return convert_use(node, ctx, parent_abs_transform)
            .into_iter()
            .collect();
    }

    let computed = ctx.computed_style(&element);

    if ty == LayoutElementType::SVGTextElement {
        return convert_text(&element, computed.as_deref(), ctx, parent_abs_transform);
    }

    if ty == LayoutElementType::SVGImageElement {
        return convert_image(&element, computed.as_deref(), ctx, parent_abs_transform);
    }

    build_shape_node(&element, ty, computed.as_deref(), host, ctx, parent_abs_transform)
}
