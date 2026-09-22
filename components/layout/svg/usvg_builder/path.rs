/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use layout_api::LayoutElementType;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use super::SvgContext;
use crate::svg::primitives::attrs::element_id;
use crate::svg::primitives::paint::{build_fill, build_stroke};
use crate::svg::primitives::shape::resolve_shape_path;

pub(super) fn build_shape<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a>,
) -> Option<usvg::Node> {
    let Some(data) = resolve_shape_path(element, ty) else {
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

    // TODO(dom-to-usvg): apply the element's `opacity` by wrapping the path in a
    // group carrying the opacity, instead of returning the path directly.
    Some(path_node)
}
