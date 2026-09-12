/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Builds a basic shape element into an [`usvg::Path`], plus any sibling marker
//! groups it carries.

use std::sync::Arc;

use layout_api::LayoutElementType;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;
use style::values::computed::Length;
use style::values::generics::svg::SVGLength;

use super::marker::build_markers;
use crate::svg::effects::{Effects, resolve_effects};
use crate::svg::primitives::attrs::{
    element_has_explicit_fill, element_has_explicit_stroke, element_id,
};
use crate::svg::primitives::paint::{build_fill, build_stroke};
use crate::svg::primitives::shape::resolve_shape_path;

use super::{SvgContext, carry_group, needs_carry};

/// Builds the path geometry + paint + markers for a basic shape element.
///
/// This is the one builder that returns a `Vec` rather than a single node: a shape
/// carrying markers emits the path plus one group per placed marker, and markers
/// are *siblings* of the path in usvg.
pub(super) fn build_shape<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    host: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(data) = resolve_shape_path(element, ty, computed) else {
        return Vec::new();
    };

    let transform = ctx.transform_attr(element);
    let abs_transform = parent_abs_transform.pre_concat(transform);

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

    // When this shape is reachable through a `<use>`, its `fill` and `stroke`
    // inherit from the `<use>` host independently: each is taken from the host
    // unless this element explicitly sets it (attribute or inline `style`). This
    // matters for `<use href="#path" stroke="…">` guides, where the referenced
    // path may set `fill="none"` but leave `stroke` to be inherited.
    let fill_computed = match host {
        Some(_) if !element_has_explicit_fill(element) => host,
        _ => computed,
    };
    let stroke_computed = match host {
        Some(_) if !element_has_explicit_stroke(element) => host,
        _ => computed,
    };
    let fill = fill_computed.and_then(|c| build_fill(c, ctx.gradients));
    let stroke = stroke_computed.and_then(|c| build_stroke(c, ctx.gradients, ctx.diagonal));

    // Marker scaling uses the stroke width in `markerUnits="strokeWidth"` mode
    // (the default). This is the raw stroke width, resolved independently of
    // whether a stroke is actually painted (`stroke="none"` still scales markers
    // at the default 1.0).
    let stroke_width = stroke_computed
        .map(|c| {
            let inherited = c.get_inherited_svg();
            match &inherited.stroke_width {
                SVGLength::LengthPercentage(nn_lp) => {
                    nn_lp.0.resolve(Length::new(ctx.diagonal)).px()
                },
                _ => 1.0,
            }
        })
        .unwrap_or(1.0);

    let Some(path_node) = usvg::Path::new(
        id,
        visible,
        fill,
        stroke,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::default(),
        Arc::new(data.clone()),
        abs_transform,
    )
    .map(|p| usvg::Node::Path(Box::new(p))) else {
        return Vec::new();
    };

    let mut nodes = vec![path_node];
    nodes.extend(build_markers(
        element,
        &data,
        stroke_width,
        ctx,
        abs_transform,
    ));

    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let object_bbox = data.bounds().to_non_zero_rect();

    let Some(effects) = resolve_effects(element, ctx, object_bbox) else {
        return Vec::new();
    };

    if needs_carry(&transform, element_opacity, &effects) {
        let mut group = usvg::Group::empty();
        carry_group(&mut group, transform, abs_transform, element_opacity, effects);
        for node in nodes {
            group.push_child(node);
        }
        vec![usvg::Node::Group(Box::new(group))]
    } else {
        nodes
    }
}
