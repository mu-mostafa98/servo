/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `<text>` element (span/tspan/textPath content).

use html5ever::{LocalName, ns};
use layout_api::LayoutElement;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use crate::svg::builder::SvgContext;
use crate::svg::effects::clip::{ClipPathOutcome, resolve_clip_path};
use crate::svg::effects::mask::{MaskOutcome, resolve_mask};
use crate::svg::primitives::attrs::element_id;
use crate::svg::primitives::text::{
    collect_text_chunks, convert_direction, convert_writing_mode, resolve_positions_list,
    resolve_rotate_list, trim_text_tree,
};

/// Builds a [`usvg::Text`] node from a `<text>` element, laying it out into glyph
/// outlines via usvg's own text engine.
pub(crate) fn convert_text<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(computed) = computed else {
        return Vec::new();
    };

    let transform = ctx.transform_attr(element);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let node = element.as_node();
    let texts = trim_text_tree(node);
    let pos_list = resolve_positions_list(node, &texts);
    let rotate_list = resolve_rotate_list(node, &texts);
    let writing_mode = convert_writing_mode(element);
    let direction = convert_direction(computed);
    let chunks = collect_text_chunks(
        element,
        &pos_list,
        ctx.context,
        ctx.gradients,
        ctx.defs,
        ctx.diagonal,
        &texts,
    );
    if chunks.is_empty() {
        return Vec::new();
    }

    let rendering_mode = element
        .attribute_as_str(&ns!(), &LocalName::from("text-rendering"))
        .and_then(|s| s.parse::<usvg::TextRendering>().ok())
        .unwrap_or_default();

    let mut text = usvg::Text::new(element_id(element).unwrap_or_default(), abs_transform);
    text.rendering_mode = rendering_mode;
    text.dx = pos_list.iter().map(|p| p.dx.unwrap_or(0.0)).collect();
    text.dy = pos_list.iter().map(|p| p.dy.unwrap_or(0.0)).collect();
    text.rotate = rotate_list;
    text.writing_mode = writing_mode;
    text.direction = direction;
    text.chunks = chunks;

    if usvg::layout(
        &mut text,
        &ctx.fonts.resolver,
        &mut ctx.fonts.cache.borrow_mut(),
    )
    .is_none()
    {
        return Vec::new();
    }

    let object_bbox = text.bounding_box().to_non_zero_rect();

    let node = usvg::Node::Text(Box::new(text));

    // Like shapes, a `<text>` element's local `transform`/`opacity`/`clip-path`/
    // `mask` are carried by a wrapper group (usvg::Text has no such fields).
    let element_opacity = computed.get_effects().opacity;
    let clip_path = match resolve_clip_path(element, ctx, object_bbox) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return Vec::new(),
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, ctx, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    if !transform.is_identity() || element_opacity < 1.0 || clip_path.is_some() || mask.is_some() {
        let mut group = usvg::Group::empty();
        group.transform = transform;
        group.abs_transform = abs_transform;
        group.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        group.clip_path = clip_path;
        group.mask = mask;
        group.push_child(node);
        vec![usvg::Node::Group(Box::new(group))]
    } else {
        vec![node]
    }
}
