/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The group-like elements (`g`/`a`/`clipPath`/`mask`).

use layout_api::LayoutNode;
use resvg::usvg;
use script::layout_dom::ServoLayoutNode;
use style::properties::ComputedValues;

use crate::svg::builder::{SvgContext, convert_node};
use crate::svg::effects::clip::{ClipPathOutcome, resolve_clip_path};
use crate::svg::effects::filter::{FilterOutcome, resolve_filter};
use crate::svg::effects::mask::{MaskOutcome, resolve_mask};
use crate::svg::primitives::attrs::element_id;

/// Converts a group-like element into a [`usvg::Group`]. Unlike a basic shape, a
/// group's `transform`/`opacity` live *on* the group node itself (usvg groups carry
/// those fields), so no wrapper group is needed.
pub(crate) fn convert_group<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = ctx.computed_style(&element);

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let transform = ctx.transform_attr(&element);
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    group.opacity = ctx.opacity(computed.as_deref());

    for child in node.dom_children() {
        for child_node in convert_node(child, ctx, abs_transform, host) {
            group.push_child(child_node);
        }
    }

    // Compute the object bounding box before resolving `clip-path`/`mask`, since
    // either may use `objectBoundingBox` units that depend on it.
    let object_bbox = group.compute_object_bbox();

    match resolve_clip_path(&element, ctx, object_bbox) {
        ClipPathOutcome::Clip(clip) => group.clip_path = Some(clip),
        ClipPathOutcome::Invalid => return None,
        ClipPathOutcome::None => {},
    }

    match resolve_mask(&element, ctx, object_bbox) {
        MaskOutcome::Mask(mask) => group.mask = Some(mask),
        MaskOutcome::Invalid => return None,
        MaskOutcome::None => {},
    }

    match resolve_filter(&element, ctx, object_bbox) {
        FilterOutcome::Filter(filter) => group.filters.push(filter),
        FilterOutcome::Invalid => return None,
        FilterOutcome::None => {},
    }

    Some(usvg::Node::Group(Box::new(group)))
}
