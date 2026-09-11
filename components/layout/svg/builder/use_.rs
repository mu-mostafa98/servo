/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `<use>` element (and its `<symbol>`-referencing variant).

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::properties::ComputedValues;

use crate::svg::builder::{SvgContext, convert_node};
use crate::svg::effects::clip::rect_clip_path;
use crate::svg::primitives::attrs::{
    element_id, element_layout_type, length_attr_opt, parse_view_box,
};

/// Converts a `<use href="#id">` element into a group whose children are the
/// referenced element's nodes, translated by the `<use>`'s `x`/`y`.
pub(crate) fn convert_use<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;

    let href = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))?;
    let id = href.trim_start_matches('#');
    if id.is_empty() {
        return None;
    }

    let referenced = ctx.defs.get(id)?;
    if referenced.as_node().opaque() == node.opaque() {
        return None;
    }

    let computed = ctx.computed_style(&element);

    // A `<use>` referencing a `<symbol>` establishes a new viewport (the symbol's
    // `viewBox` mapped onto the `<use>`'s `width`×`height`), handled separately.
    if element_layout_type(referenced) == LayoutElementType::SVGSymbolElement {
        return convert_use_symbol(
            node,
            referenced,
            computed.as_deref(),
            ctx,
            parent_abs_transform,
        );
    }

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let mut transform = ctx.transform_attr(&element);
    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        transform = transform.pre_concat(usvg::Transform::from_translate(x, y));
    }
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    group.opacity = ctx.opacity(computed.as_deref());

    let referenced_node = referenced.as_node();
    for child_node in convert_node(referenced_node, ctx, abs_transform, computed.as_deref()) {
        group.push_child(child_node);
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Converts a `<use href="#symbol">` reference. Unlike `<use>` of a plain group, a
/// `<symbol>` establishes a new viewport: its `viewBox` is mapped onto the `<use>`'s
/// `x`/`y`/`width`/`height` rectangle (mirroring usvg's `use_node::convert`). The
/// structure matches [`convert_svg`]: an outer group carries the `transform`
/// attribute (plus an optional viewport clip), and an inner group carries the
/// viewport transform (`translate(x, y) · viewBox→viewport`).
fn convert_use_symbol<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    symbol: &ServoLayoutElement<'dom>,
    host: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;

    // The `transform` attribute operates in the parent user space, before the new
    // viewport is established, so it stays on the outer group.
    let orig_ts = ctx.transform_attr(&element);

    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);

    // Viewport transform: translate to (x, y), then map the symbol's `viewBox` onto
    // the `<use>`'s width×height rectangle (defaulting each to 100%, like usvg).
    let mut viewport_ts = usvg::Transform::from_translate(x, y);
    if let Some(vb) = parse_view_box(symbol) {
        let w = length_attr_opt(&element, "width").unwrap_or(100.0);
        let h = length_attr_opt(&element, "height").unwrap_or(100.0);
        if let Some(size) = usvg::Size::from_wh(w, h) {
            viewport_ts = viewport_ts.pre_concat(vb.to_transform(size));
        }
    }

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();
    group.transform = orig_ts;
    let abs_transform = parent_abs_transform.pre_concat(orig_ts);
    group.abs_transform = abs_transform;

    group.opacity = ctx.opacity(host);

    // A symbol establishes a viewport; clip to its rectangle unless `overflow` is
    // `visible` (or `auto`), mirroring the nested-`<svg>` handling in `convert_svg`.
    let overflow_visible = matches!(
        symbol.attribute_as_str(&ns!(), &LocalName::from("overflow")),
        Some("visible") | Some("auto")
    );
    if !overflow_visible {
        let w = length_attr_opt(&element, "width").unwrap_or(100.0);
        let h = length_attr_opt(&element, "height").unwrap_or(100.0);
        if let Some(clip_rect) = usvg::NonZeroRect::from_xywh(x, y, w, h) {
            group.clip_path = rect_clip_path(clip_rect);
        }
    }

    // Wrap the symbol's children in an inner group carrying the viewport transform.
    let mut inner = usvg::Group::empty();
    inner.transform = viewport_ts;
    inner.abs_transform = abs_transform.pre_concat(viewport_ts);

    let symbol_node = symbol.as_node();
    for child in symbol_node.dom_children() {
        for child_node in convert_node(child, ctx, inner.abs_transform, host) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}
