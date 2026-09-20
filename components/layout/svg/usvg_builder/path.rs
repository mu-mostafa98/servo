/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Builds a basic shape element into an [`usvg::Path`].

use std::sync::Arc;

use layout_api::LayoutElementType;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use super::SvgContext;
use crate::svg::primitives::attrs::element_id;
use crate::svg::primitives::paint::{build_fill, build_stroke};
use crate::svg::primitives::shape::resolve_shape_path;

/// Builds the path geometry + paint for a basic shape element.
///
/// Returns a single node, wrapped in a carrying group when the shape has an
/// opacity below 1.0.
pub(super) fn build_shape<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a>,
) -> Option<usvg::Node> {
    let Some(data) = resolve_shape_path(element, ty, computed) else {
        return None;
    };

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

    let fill = computed.and_then(build_fill);
    let stroke = computed.and_then(|c| build_stroke(c, ctx.diagonal));

    let Some(path_node) = usvg::Path::new(
        id,
        visible,
        fill,
        stroke,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::default(),
        Arc::new(data),
        usvg::Transform::identity(),
    )
    .map(|p| usvg::Node::Path(Box::new(p)))
    else {
        return None;
    };

    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);

    if element_opacity < 1.0 {
        let mut group = usvg::Group::empty();
        group.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        group.push_child(path_node);
        Some(usvg::Node::Group(Box::new(group)))
    } else {
        Some(path_node)
    }
}
