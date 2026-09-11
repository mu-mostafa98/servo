/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `mask` resolution: turns a `<mask>` element into a [`usvg::Mask`].

use std::sync::Arc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::attrs::{element_id, element_layout_type, length_or_percentage_attr};
use crate::svg::usvg_builder::{SvgContext, build_usvg_node};

/// Extracts the referenced id from a `mask` attribute, if it is a local
/// `url(#id)` reference (not `none`).
fn mask_reference(element: &ServoLayoutElement<'_>) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from("mask"))?
        .trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = value.strip_prefix("url(")?.strip_suffix(')')?;
    let inner = inner.trim().trim_matches('"').trim_matches('\'');
    let id = inner.strip_prefix('#')?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Builds a [`usvg::Mask`] from a `<mask>` element, mirroring usvg's
/// `parser::mask::convert`.
///
/// `object_bbox` is the bounding box of the element *being masked* (in its local
/// coordinate system); `depth` guards against cyclic `mask` references.
fn build_mask<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
    depth: usize,
) -> Option<Arc<usvg::Mask>> {
    if depth > 8 || element_layout_type(element) != LayoutElementType::SVGMaskElement {
        return None;
    }
    let id_str = element_id(element)?;

    // `maskUnits` (the mask *region*) defaults to `objectBoundingBox`; the mask
    // *content* (`maskContentUnits`) defaults to `userSpaceOnUse`.
    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("maskUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let content_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("maskContentUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    // Region x/y/width/height default to `-10% -10% 120% 120%`. Percentages are
    // resolved as a fraction of the object bbox (`length_or_percentage_attr` maps
    // `%` → `value/100`), which is the correct interpretation for `objectBoundingBox`.
    let x = length_or_percentage_attr(element, "x", -0.1);
    let y = length_or_percentage_attr(element, "y", -0.1);
    let width = length_or_percentage_attr(element, "width", 1.2);
    let height = length_or_percentage_attr(element, "height", 1.2);
    let rect = usvg::NonZeroRect::from_xywh(x, y, width, height)?;

    let rect = match units {
        usvg::Units::ObjectBoundingBox => {
            // `objectBoundingBox` maps the `(0,0)-(1,1)` square onto the object's
            // bbox. When there is no bbox (zero-sized/empty object), the whole
            // element is masked, mirroring usvg's `mask_all` branch.
            match object_bbox {
                Some(bbox) => rect.bbox_transform(bbox),
                None => {
                    let id = usvg::NonEmptyString::new(id_str)?;
                    return Some(Arc::new(usvg::Mask::new(
                        id,
                        rect,
                        usvg::MaskType::Luminance,
                        None,
                        usvg::Group::empty(),
                    )));
                },
            }
        },
        usvg::Units::UserSpaceOnUse => rect,
    };

    // A `<mask>` may itself reference another `<mask>` via its own `mask` attribute,
    // nesting the two.
    let mask = match mask_reference(element) {
        Some(ref_id) => {
            let linked = ctx.defs.get(&ref_id)?;
            Some(build_mask(linked, ctx, object_bbox, depth + 1)?)
        },
        None => None,
    };

    let kind = if element.attribute_as_str(&ns!(), &LocalName::from("mask-type")) == Some("alpha") {
        usvg::MaskType::Alpha
    } else {
        usvg::MaskType::Luminance
    };

    let mut root = usvg::Group::empty();

    // Mask content is authored in the referencing element's user space
    // (`maskContentUnits="userSpaceOnUse"`, the default) or in `[0,1]` bbox units
    // (`objectBoundingBox`), in which case it is wrapped in a `from_bbox` group.
    if content_units == usvg::Units::ObjectBoundingBox {
        let object_bbox = object_bbox?;
        let mut subroot = usvg::Group::empty();
        subroot.transform = usvg::Transform::from_bbox(object_bbox);
        subroot.abs_transform = subroot.transform;

        for child in element.as_node().dom_children() {
            for node in build_usvg_node(child, ctx, subroot.transform, None) {
                subroot.push_child(node);
            }
        }

        if !subroot.has_children() {
            return None;
        }

        root.push_child(usvg::Node::Group(Box::new(subroot)));
    } else {
        for child in element.as_node().dom_children() {
            for node in build_usvg_node(child, ctx, usvg::Transform::identity(), None) {
                root.push_child(node);
            }
        }

        // A mask without children is invalid (the referencing element is dropped),
        // except in the zero-bbox case handled above.
        if !root.has_children() {
            return None;
        }
    }

    Some(Arc::new(usvg::Mask::new(
        usvg::NonEmptyString::new(id_str)?,
        rect,
        kind,
        mask,
        root,
    )))
}

/// The result of resolving an element's `mask` attribute.
pub(crate) enum MaskOutcome {
    /// No mask: the attribute is absent, `none`, malformed, or a dangling
    /// reference — render the element normally.
    None,
    /// A valid mask to apply.
    Mask(Arc<usvg::Mask>),
    /// The `mask` references a `mask` element that is invalid (empty, or not a
    /// `<mask>`) — the element must be dropped entirely.
    Invalid,
}

/// Resolves an element's `mask` reference into a [`usvg::Mask`], mirroring usvg's
/// `convert_group`/`mask::convert` handshake: a dangling reference is ignored, while
/// a present-but-invalid mask drops the element.
pub(crate) fn resolve_mask<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> MaskOutcome {
    let Some(ref_id) = mask_reference(element) else {
        return MaskOutcome::None;
    };
    let Some(linked) = ctx.defs.get(&ref_id) else {
        // Dangling id: not in the document, so treat as "no mask" like usvg.
        return MaskOutcome::None;
    };
    match build_mask(linked, ctx, object_bbox, 0) {
        Some(mask) => MaskOutcome::Mask(mask),
        None => MaskOutcome::Invalid,
    }
}
