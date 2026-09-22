/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType};
use resvg::usvg::tiny_skia_path;
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::geometry::parse_path_d;

pub(crate) fn resolve_shape_path(
    element: &ServoLayoutElement<'_>,
    ty: LayoutElementType,
) -> Option<tiny_skia_path::Path> {
    match ty {
        LayoutElementType::SVGPathElement => element
        .attribute_as_str(&ns!(), &LocalName::from("d"))
        .and_then(parse_path_d),
        // TODO: Complete all the 7 type
        _ => None,
    }
}
