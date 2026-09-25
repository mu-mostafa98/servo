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

use crate::svg::primitives::attrs::{parse_inline_style_prop, parse_length_value};

/// Default SVG font size used to resolve font-relative lengths (`em`/`ex`) on
/// viewport-establishing elements (matches the geometry layer's convention).
const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

/// Extract viewport info from the root `<svg>` element.
///
/// `width`/`height` are the *resolved* viewport dimensions (in user units),
/// computed by CSS layout from the `width`/`height` presentation attributes —
/// not re-parsed here. This keeps percentage/unit resolution for the root's
/// viewport size in the one place that knows the containing-block size.
pub(crate) fn extract_viewport_info<'dom>(
    node: ServoLayoutNode<'dom>,
    width: f32,
    height: f32,
) -> ViewportInfo {
    let element = node.as_element().unwrap();
    let get = |attr: &str| {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .map(|s| s.to_string())
    };
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
        width: Length::new(width),
        height: Length::new(height),
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
///
/// `parent_vw`/`parent_vh` are the current viewport's percentage-resolution
/// reference dimensions: the nested element's `x`/`y`/`width`/`height`
/// percentages resolve against them (§8.8).
pub(crate) fn extract_nested_viewport<'dom>(
    node: ServoLayoutNode<'dom>,
    parent_vw: f32,
    parent_vh: f32,
) -> Option<SvgViewport> {
    let element = node.as_element()?;
    let get = |attr: &str| {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .map(|s| s.to_string())
    };
    // `x`/`width` percentages resolve against the parent viewport width;
    // `y`/`height` percentages against the parent viewport height.
    let parse_len = |attr: &str, default: f32, reference: f32| -> f32 {
        get(attr)
            .and_then(|v| parse_length_value(&v, SVG_DEFAULT_FONT_SIZE, reference))
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
        x: Length::new(parse_len("x", 0.0, parent_vw)),
        y: Length::new(parse_len("y", 0.0, parent_vh)),
        // A nested `<svg>` defaults to 100% of the parent viewport (§8.8), so the
        // fallback is the parent reference dimension itself.
        width: Length::new(parse_len("width", parent_vw, parent_vw)),
        height: Length::new(parse_len("height", parent_vh, parent_vh)),
        view_box: get("viewBox").as_deref().and_then(extract_viewbox),
        aspect_ratio: get("preserveAspectRatio").as_deref().map(parse_aspect_ratio),
        overflow_visible,
    })
}

/// Compute the percentage-resolution reference dimensions for the root viewport.
///
/// SVG percentages resolve against the `viewBox` extent when one is present,
/// otherwise against the viewport `width`/`height` attributes.
pub(crate) fn viewport_reference(vp: &ViewportInfo) -> (f32, f32) {
    match vp.view_box.as_ref() {
        Some(vb) => (vb.width.get(), vb.height.get()),
        None => (vp.width.get(), vp.height.get()),
    }
}

/// Compute the percentage-resolution reference dimensions for a nested `<svg>`
/// viewport.
pub(crate) fn svg_viewport_reference(vp: &SvgViewport) -> (f32, f32) {
    match vp.view_box.as_ref() {
        Some(vb) => (vb.width.get(), vb.height.get()),
        None => (vp.width.get(), vp.height.get()),
    }
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
