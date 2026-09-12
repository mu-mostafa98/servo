/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The recursive `<pattern>` paint-server builder.
//!
//! Unlike gradients and `fill`/`stroke` resolution (leaf functions in
//! [`crate::svg::primitives::paint`]), a `<pattern>` recurses through its
//! children via [`crate::svg::usvg_builder::build_usvg_node`], so it lives here
//! alongside the other recursive effect builders (`clip`, `mask`, `filter`).

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::attrs::{
    element_id, length_or_percentage_attr, parse_transform, parse_view_box,
};
use crate::svg::usvg_builder::{SvgContext, build_usvg_node};

/// Builds a [`usvg::Pattern`] from a `<pattern>` element.
pub(crate) fn build_pattern<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
) -> Option<usvg::Pattern> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;

    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("patternUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let content_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("patternContentUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("patternTransform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);

    let x = length_or_percentage_attr(element, "x", 0.0);
    let y = length_or_percentage_attr(element, "y", 0.0);
    let width = length_or_percentage_attr(element, "width", 0.0);
    let height = length_or_percentage_attr(element, "height", 0.0);
    let rect = usvg::NonZeroRect::from_xywh(x, y, width, height)?;

    let view_box = parse_view_box(element);

    let mut root = usvg::Group::empty();
    for child in element.as_node().dom_children() {
        for child_node in build_usvg_node(child, ctx, usvg::Transform::identity(), None) {
            root.push_child(child_node);
        }
    }

    Some(usvg::Pattern::new(
        id,
        units,
        content_units,
        transform,
        rect,
        view_box,
        root,
    ))
}
