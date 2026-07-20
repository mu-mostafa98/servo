/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Viewport and viewBox extraction from the root `<svg>` DOM element.

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::render_tree::{ViewportInfo, extract_viewbox, parse_aspect_ratio};
use web_atoms::ns;

use super::style::parse_inline_style_prop;

/// Extract viewport info from the root `<svg>` element.
pub(crate) fn extract_viewport_info<'dom>(node: ServoLayoutNode<'dom>) -> ViewportInfo {
    let element = node.as_element().unwrap();
    let get = |attr: &str| {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .map(|s| s.to_string())
    };
    let svg_width = get("width")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(300.0);
    let svg_height = get("height")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(150.0);
    let view_box = get("viewBox").as_deref().and_then(extract_viewbox);

    let overflow_visible = get("overflow")
        .or_else(|| {
            get("style")
                .as_deref()
                .and_then(|s| parse_inline_style_prop(s, "overflow"))
        })
        .map_or(false, |v| v.trim().eq_ignore_ascii_case("visible"));

    let aspect_ratio = get("preserveAspectRatio")
        .as_deref()
        .map(parse_aspect_ratio);

    ViewportInfo {
        width: svg_width,
        height: svg_height,
        view_box,
        overflow_visible,
        aspect_ratio,
    }
}
