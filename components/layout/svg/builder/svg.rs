/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The nested `<svg>` element, which establishes a new viewport.

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use resvg::usvg;
use script::layout_dom::ServoLayoutNode;
use style::properties::ComputedValues;

use crate::svg::builder::{convert_node, SvgContext};
use crate::svg::effects::clip::rect_clip_path;
use crate::svg::primitives::attrs::{element_id, length_attr_opt, parse_view_box};

/// Converts a nested `<svg>` element, which establishes a new viewport: its `x`/`y`
/// position plus a `viewBox`→viewport transform (`preserveAspectRatio`-aware). When
/// `overflow` is not `visible` (and explicit `width`/`height` form a rectangle), a
/// synthetic clip path limits rendering to the new viewport — mirroring the parser's
/// `use_node::convert_svg` in usvg.
///
/// The structure matches usvg: an outer group carries the `transform` attribute and
/// (optionally) the clip path, and an inner group carries the viewport transform
/// (`translate(x, y) · viewBox`) so that it participates correctly in bounding-box
/// and clipping calculations.
pub(crate) fn convert_svg<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = ctx.computed_style(&element);

    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);

    // The `transform` attribute operates in the parent user space, before the new
    // viewport is established, so it stays on the outer group.
    let orig_ts = ctx.transform_attr(&element);

    // Viewport transform: translate to (x, y), then map the `viewBox` onto the
    // viewport rectangle.
    let mut viewport_ts = usvg::Transform::from_translate(x, y);
    if let Some(vb) = parse_view_box(&element) {
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

    group.opacity = ctx.opacity(computed.as_deref());

    // A nested `svg` with explicit `width`/`height` is clipped to its viewport
    // rectangle unless `overflow` is `visible` (or `auto`).
    let overflow_visible = matches!(
        element.attribute_as_str(&ns!(), &LocalName::from("overflow")),
        Some("visible") | Some("auto")
    );
    if !overflow_visible {
        if let (Some(w), Some(h)) = (
            length_attr_opt(&element, "width"),
            length_attr_opt(&element, "height"),
        ) {
            if let Some(clip_rect) = usvg::NonZeroRect::from_xywh(x, y, w, h) {
                group.clip_path = rect_clip_path(clip_rect);
            }
        }
    }

    // Wrap the children in an inner group carrying the viewport transform, so it
    // applies as a proper transform in bounding-box and clip calculations.
    let mut inner = usvg::Group::empty();
    inner.transform = viewport_ts;
    inner.abs_transform = abs_transform.pre_concat(viewport_ts);

    for child in node.dom_children() {
        for child_node in convert_node(child, ctx, inner.abs_transform, host) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}
