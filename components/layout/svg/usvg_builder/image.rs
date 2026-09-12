/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Builds an `<image>` element into an [`usvg::Image`] node, wrapped in align and
//! effect groups.

use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use crate::svg::effects::resolve_effects;
use crate::svg::primitives::attrs::element_id;
use crate::svg::primitives::image::{decode_image, image_geometry};

use super::{SvgContext, carry_group, needs_carry};

/// Builds an `<image>` element into an `Image` node (always wrapped in an inner
/// align group, plus an outer effect group when the element carries
/// transform/opacity/clip/mask/filter).
pub(super) fn build_image<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let (kind, size) = decode_image(element)?;
    let (translate_scale, object_bbox) = image_geometry(element, size)?;

    let element_transform = ctx.transform_attr(element);
    let image_ts = element_transform.pre_concat(translate_scale);
    let abs_transform = parent_abs_transform.pre_concat(image_ts);

    let id = element_id(element).unwrap_or_default();
    let visible = computed
        .map(|c| {
            !matches!(
                c.get_inherited_box().visibility,
                style::computed_values::visibility::T::Hidden |
                    style::computed_values::visibility::T::Collapse
            )
        })
        .unwrap_or(true);

    let image = usvg::Image::new(
        id,
        visible,
        size,
        usvg::ImageRendering::default(),
        kind,
        abs_transform,
    )?;

    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let effects = resolve_effects(element, ctx, Some(object_bbox))?;

    // The image is always wrapped in an inner group carrying the align transform
    // (x/y/width/height/preserveAspectRatio position+scale). clip-path/mask/filter
    // must NOT live on this group: resvg isolates the group that carries them and
    // applies the clip/mask in that group's shifted *local* coordinate system, so a
    // `userSpaceOnUse` clip authored in the element's user space would be scaled by
    // the align transform and clip the image away. Instead those properties go on an
    // *outer* group whose transform is the element's own `transform` attribute.
    let mut inner = usvg::Group::empty();
    inner.transform = translate_scale;
    inner.abs_transform = abs_transform;
    inner.push_child(usvg::Node::Image(Box::new(image)));

    let inner_node = usvg::Node::Group(Box::new(inner));

    if needs_carry(&element_transform, element_opacity, &effects) {
        let mut outer = usvg::Group::empty();
        carry_group(
            &mut outer,
            element_transform,
            parent_abs_transform.pre_concat(element_transform),
            element_opacity,
            effects,
        );
        outer.push_child(inner_node);
        Some(usvg::Node::Group(Box::new(outer)))
    } else {
        Some(inner_node)
    }
}
