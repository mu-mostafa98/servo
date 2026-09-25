/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Viewport and viewBox extraction from the root `<svg>` DOM element.

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{AspectAlign, AspectRatio, MeetOrSlice, SvgViewport, ViewBox, ViewportInfo};
use svg_engine::units::Length;
use svgtypes::ViewBox as SvgViewBox;
use web_atoms::ns;

use crate::svg::primitives::attrs::parse_inline_style_prop;

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
        width: Length::new(svg_width),
        height: Length::new(svg_height),
        view_box,
        overflow_visible,
        aspect_ratio,
    }
}

/// Extract viewport info from a **nested** `<svg>` element (not the root).
///
/// Unlike the root (whose size is imposed by layout), a nested `<svg>` carries
/// its own `x`/`y`/`width`/`height` attributes that position and size the
/// sub-viewport, plus an optional `viewBox` and `preserveAspectRatio`.
pub(crate) fn extract_nested_viewport<'dom>(node: ServoLayoutNode<'dom>) -> Option<SvgViewport> {
    let element = node.as_element()?;
    let get = |attr: &str| {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .map(|s| s.to_string())
    };
    let parse_len = |attr: &str, default: f32| -> f32 {
        get(attr)
            .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(default)
    };

    let overflow_visible = get("overflow")
        .or_else(|| {
            get("style")
                .as_deref()
                .and_then(|s| parse_inline_style_prop(s, "overflow"))
        })
        .map_or(false, |v| v.trim().eq_ignore_ascii_case("visible"));

    Some(SvgViewport {
        x: Length::new(parse_len("x", 0.0)),
        y: Length::new(parse_len("y", 0.0)),
        width: Length::new(parse_len("width", 300.0)),
        height: Length::new(parse_len("height", 150.0)),
        view_box: get("viewBox").as_deref().and_then(extract_viewbox),
        aspect_ratio: get("preserveAspectRatio").as_deref().map(parse_aspect_ratio),
        overflow_visible,
    })
}

// ======================= AspectRatio Parsing =======================

/// Parse a `preserveAspectRatio` attribute value.
///
/// SVG spec: `<align> <meetOrSlice>?`
/// Defaults to `xMidYMid meet`.
pub(crate) fn parse_aspect_ratio(value: &str) -> AspectRatio {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return AspectRatio {
            align: AspectAlign::None,
            meet_or_slice: MeetOrSlice::Meet,
        };
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    let align = match parts.first().copied().unwrap_or("xMidYMid") {
        "none" => AspectAlign::None,
        "xMinYMin" => AspectAlign::XMinYMin,
        "xMidYMin" => AspectAlign::XMidYMin,
        "xMaxYMin" => AspectAlign::XMaxYMin,
        "xMinYMid" => AspectAlign::XMinYMid,
        "xMidYMid" => AspectAlign::XMidYMid,
        "xMaxYMid" => AspectAlign::XMaxYMid,
        "xMinYMax" => AspectAlign::XMinYMax,
        "xMidYMax" => AspectAlign::XMidYMax,
        "xMaxYMax" => AspectAlign::XMaxYMax,
        _ => AspectAlign::XMidYMid,
    };
    let meet_or_slice = parts
        .get(1)
        .copied()
        .and_then(|s| {
            if s.eq_ignore_ascii_case("slice") {
                Some(MeetOrSlice::Slice)
            } else {
                None
            }
        })
        .unwrap_or(MeetOrSlice::Meet);

    AspectRatio {
        align,
        meet_or_slice,
    }
}

// ======================= ViewBox Parsing =======================

/// Parse the `viewBox` attribute value into a [`ViewBox`].
///
/// Delegates to [`svgtypes::ViewBox`] for spec-compliant parsing.
/// Handles formats: `"0 0 200 200"`, `"0,0 200,200"`, etc.
pub(crate) fn extract_viewbox(value: &str) -> Option<ViewBox> {
    value.parse::<SvgViewBox>().ok().map(|vb| ViewBox {
        min_x: Length::new(vb.x as f32),
        min_y: Length::new(vb.y as f32),
        width: Length::new(vb.w as f32),
        height: Length::new(vb.h as f32),
    })
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_valid() {
        let vb = extract_viewbox("0 0 200 200").unwrap();
        assert_eq!(vb.min_x.get(), 0.0);
        assert_eq!(vb.min_y.get(), 0.0);
        assert_eq!(vb.width.get(), 200.0);
        assert_eq!(vb.height.get(), 200.0);
    }

    #[test]
    fn viewbox_with_commas() {
        let vb = extract_viewbox("10,20 300,400").unwrap();
        assert_eq!(vb.min_x.get(), 10.0);
        assert_eq!(vb.min_y.get(), 20.0);
        assert_eq!(vb.width.get(), 300.0);
        assert_eq!(vb.height.get(), 400.0);
    }

    #[test]
    fn viewbox_invalid_too_few() {
        assert!(extract_viewbox("0 0 200").is_none());
    }

    #[test]
    fn viewbox_invalid_too_many() {
        // svgtypes::ViewBox tolerates trailing data by spec (it stops at the 4th number).
        // If there are at least 4 valid numbers, it parses OK.
        assert!(extract_viewbox("0 0 200 200 100").is_some());
    }

    #[test]
    fn viewbox_zero_width() {
        assert!(extract_viewbox("0 0 0 200").is_none());
    }

    #[test]
    fn viewbox_negative_width() {
        assert!(extract_viewbox("0 0 -100 200").is_none());
    }

    #[test]
    fn viewbox_negative_coords() {
        let vb = extract_viewbox("-100 -100 200 200").unwrap();
        assert_eq!(vb.min_x.get(), -100.0);
        assert_eq!(vb.min_y.get(), -100.0);
    }

    #[test]
    fn viewbox_empty() {
        assert!(extract_viewbox("").is_none());
    }

    #[test]
    fn viewbox_garbage() {
        assert!(extract_viewbox("abc def ghi jkl").is_none());
    }
}
