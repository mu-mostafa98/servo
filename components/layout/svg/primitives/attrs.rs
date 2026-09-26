/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG attribute parsing utilities.
//!
//! These functions parse raw SVG attribute strings into typed values. They
//! live in layout (not the engine's `model`) because they are build-time
//! converters: the DOM → [`svg_engine::model`] boundary. The engine never
//! parses attribute strings itself.
//!
//! Length parsing is backed by [`svgtypes::Length`] for spec‑compliant handling
//! of all SVG length units (`px`, `em`, `ex`, `in`, `cm`, `mm`, `pt`, `pc`, `%`).

use html5ever::LocalName;
use layout_api::LayoutElement;
use script::layout_dom::ServoLayoutElement;
use svg_engine::geometry::Point;
use svgtypes::{Length as SvgLength, PointsParser};
use web_atoms::ns;

// ======================= Attribute access =======================

/// Read an attribute from an SVG DOM element as an owned string.
pub(crate) fn get_attr(element: &ServoLayoutElement, attr: &str) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .map(|s| s.to_string())
}

/// Evaluate SVG 2 conditional-processing attributes (§5.7).
///
/// `requiredExtensions`, `systemLanguage`, and `requiredFeatures` each act as
/// a render test:
/// - **absent** → the test passes;
/// - **present, even empty** → the test fails (an empty value explicitly
///   evaluates to false).
///
/// For `requiredExtensions`/`systemLanguage` a *non-empty* value also fails,
/// because this engine supports no extension URLs and has no user-language
/// preference to match. For the deprecated `requiredFeatures` (§5.7.5) a
/// non-empty value passes only when every listed feature string is one the
/// engine supports (the base static SVG feature set).
///
/// The element renders only when *all three* tests pass.
pub(crate) fn conditional_processing_passes(element: &ServoLayoutElement) -> bool {
    required_extensions_pass(element)
        && system_language_pass(element)
        && required_features_pass(element)
}

/// `requiredExtensions` test (§5.7.3): absent passes; present (even empty)
/// fails — this engine supports no extension URLs.
fn required_extensions_pass(element: &ServoLayoutElement) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("requiredExtensions"))
        .is_none()
}

/// `systemLanguage` test (§5.7.4): absent passes; present (even empty) fails —
/// this engine has no user-language preference to match.
fn system_language_pass(element: &ServoLayoutElement) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("systemLanguage"))
        .is_none()
}

/// `requiredFeatures` test (§5.7.5): absent passes; empty fails; non-empty
/// passes only if every listed feature string is supported.
fn required_features_pass(element: &ServoLayoutElement) -> bool {
    let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from("requiredFeatures"))
    else {
        return true;
    };
    let tokens: Vec<&str> = value.split_whitespace().collect();
    !tokens.is_empty() && tokens.iter().all(|f| SUPPORTED_FEATURES.contains(f))
}

/// Feature strings this engine supports, for the deprecated `requiredFeatures`
/// test (§5.7.5). The engine implements the base static SVG feature set, so it
/// matches the corresponding feature strings of both SVG 2 and SVG 1.1 (the
/// still-widely-deployed form).
const SUPPORTED_FEATURES: &[&str] = &[
    "http://www.w3.org/TR/SVG2/feature#SVG",
    "http://www.w3.org/TR/SVG2/feature#CoreAttribute",
    "http://www.w3.org/TR/SVG2/feature#BasicStructure",
    "http://www.w3.org/TR/SVG2/feature#Shape",
    "http://www.w3.org/TR/SVG2/feature#Text",
    "http://www.w3.org/TR/SVG2/feature#Image",
    "http://www.w3.org/TR/SVG2/feature#Gradient",
    "http://www.w3.org/TR/SVG2/feature#Pattern",
    "http://www.w3.org/TR/SVG2/feature#Marker",
    "http://www.w3.org/TR/SVG2/feature#Clip",
    "http://www.w3.org/TR/SVG2/feature#Mask",
    "http://www.w3.org/TR/SVG2/feature#Filter",
    "http://www.w3.org/TR/SVG11/feature#SVG",
    "http://www.w3.org/TR/SVG11/feature#SVG-static",
    "http://www.w3.org/TR/SVG11/feature#CoreAttribute",
    "http://www.w3.org/TR/SVG11/feature#BasicStructure",
    "http://www.w3.org/TR/SVG11/feature#Shape",
    "http://www.w3.org/TR/SVG11/feature#Text",
    "http://www.w3.org/TR/SVG11/feature#Image",
    "http://www.w3.org/TR/SVG11/feature#Gradient",
    "http://www.w3.org/TR/SVG11/feature#Pattern",
    "http://www.w3.org/TR/SVG11/feature#Marker",
    "http://www.w3.org/TR/SVG11/feature#Clip",
    "http://www.w3.org/TR/SVG11/feature#Mask",
    "http://www.w3.org/TR/SVG11/feature#Filter",
];

/// `true` when `tag` is an SVG-namespace element that is neither a known
/// renderable element nor a known non-rendering element — i.e. an *unknown*
/// element, which §5.3 says must be treated as a `<g>` (children render, styles
/// inherit). Elements in other namespaces are excluded (they must not render).
pub(crate) fn is_unknown_svg_element(element: &ServoLayoutElement, tag: &str) -> bool {
    element.is_svg_element() && !is_non_rendering_svg_element(tag)
}

/// Known SVG element local names that must never be rendered directly (§5.3).
///
/// These are definition/metadata/style/effect/animation/media/font elements —
/// content that only takes effect when *referenced* (`<defs>`, paint servers,
/// clip paths, markers, …) or that is structurally non-graphical. Anything in
/// the SVG namespace that is *not* listed here and *not* a known renderable
/// element is an unknown element and renders as a `<g>`.
fn is_non_rendering_svg_element(tag: &str) -> bool {
    matches!(
        tag,
        // Document metadata / scripting.
        "title" | "desc" | "metadata" | "style" | "script"
        // Paint servers.
        | "linearGradient" | "radialGradient" | "pattern" | "solidColor" | "hatch"
        | "meshgradient" | "meshrow" | "meshpatch" | "stop"
        // Effects / reusable definitions.
        | "clipPath" | "mask" | "filter" | "marker"
        // Animation.
        | "animate" | "animateColor" | "animateMotion" | "animateTransform" | "set"
        | "discard" | "mpath"
        // Embedded / foreign media.
        | "audio" | "video" | "foreignObject"
        // Font machinery and text-structural references.
        | "font" | "font-face" | "glyph" | "missing-glyph" | "hkern" | "vkern"
        | "font-face-src" | "font-face-uri" | "font-face-format" | "font-face-name"
        | "altGlyph" | "altGlyphDef" | "altGlyphItem" | "glyphRef"
        | "textPath" | "tref"
        // Misc.
        | "view" | "cursor" | "color-profile"
    )
}

/// Extract the fragment from a `url(#fragment)` CSS/SVG URL value.
pub(crate) fn extract_url_fragment(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed.strip_prefix("url(") {
        let inner = inner.trim_end_matches(')').trim();
        inner.strip_prefix('#').map(|s| s.to_owned())
    } else {
        trimmed.strip_prefix('#').map(|s| s.to_owned())
    }
}

/// Parse a single `prop: value` pair out of an inline `style` attribute value.
pub(crate) fn parse_inline_style_prop(style_value: &str, prop_name: &str) -> Option<String> {
    for part in style_value.split(';') {
        let mut parts = part.splitn(2, ':');
        let key = parts.next()?.trim();
        let val = parts.next()?.trim();
        if key.eq_ignore_ascii_case(prop_name) && !val.is_empty() {
            return Some(val.to_owned());
        }
    }
    None
}

// ======================= Length & points =======================

/// Parse a named SVG length attribute (e.g. `x="10"`, `width="50%"`).
///
/// Handles all CSS/SVG length units via [`svgtypes::Length`].
/// Converts `em`/`ex`/`in`/`cm`/`mm`/`pt`/`pc` to pixels using the
/// provided `font_size` (default 16px for SVG).
///
/// Percent values are returned as-is (caller must resolve against
/// the appropriate reference dimension).
pub(crate) fn parse_length(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
    font_size: f32,
) -> Result<f32, String> {
    let value = get_attr(attr).ok_or_else(|| format!("missing SVG attribute: {attr}"))?;
    let len: SvgLength = value.parse().map_err(|e| format!("{attr}: {e}"))?;
    Ok(to_px(len, font_size))
}

/// Convert an [`svgtypes::Length`] to a pixel value using CSS/SVG unit conventions.
fn to_px(len: SvgLength, font_size: f32) -> f32 {
    let n = len.number as f32;
    match len.unit {
        // Absolute units
        svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px => n,
        svgtypes::LengthUnit::In => n * 96.0,
        svgtypes::LengthUnit::Cm => n * 96.0 / 2.54,
        svgtypes::LengthUnit::Mm => n * 96.0 / 25.4,
        svgtypes::LengthUnit::Pt => n * 96.0 / 72.0,
        svgtypes::LengthUnit::Pc => n * 96.0 / 6.0,
        // Font-relative units
        svgtypes::LengthUnit::Em => n * font_size,
        svgtypes::LengthUnit::Ex => n * font_size * 0.5,
        // Percent — returned as-is (caller resolves against reference).
        svgtypes::LengthUnit::Percent => n,
    }
}

/// Parse a named SVG length attribute, resolving `<percentage>` values against
/// the given `reference` dimension (e.g. the current viewport width/height).
///
/// Non-percentage units are converted exactly like [`parse_length`]; a
/// percentage is returned as `number * reference / 100`.
pub(crate) fn parse_length_resolved(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
    font_size: f32,
    reference: f32,
) -> Result<f32, String> {
    let value = get_attr(attr).ok_or_else(|| format!("missing SVG attribute: {attr}"))?;
    let len: SvgLength = value.parse().map_err(|e| format!("{attr}: {e}"))?;
    Ok(to_px_resolved(len, font_size, reference))
}

/// Convert an [`svgtypes::Length`] to pixels, resolving a `<percentage>` against
/// `reference`. Absolute and font-relative units delegate to [`to_px`].
fn to_px_resolved(len: SvgLength, font_size: f32, reference: f32) -> f32 {
    if len.unit == svgtypes::LengthUnit::Percent {
        (len.number as f32) * reference / 100.0
    } else {
        to_px(len, font_size)
    }
}

/// Parse a raw SVG length string with full unit support (`px`, `in`, `cm`, `mm`,
/// `pt`, `pc`, `em`, `ex`, `%`), resolving a `<percentage>` against
/// `percent_reference` and font-relative units against `font_size`.
///
/// Returns `None` when the string is not a valid length (e.g. `auto`, garbage).
/// Unlike [`parse_length`] (which leaves a percentage unresolved for the caller
/// to resolve against a known reference), this is the single-stop helper used by
/// the viewport-establishing elements — root/nested `<svg>`, `<use>`, `<symbol>`,
/// `<image>`, `<pattern>`, `<marker>` — whose `x`/`y`/`width`/`height` may carry
/// any unit or a percentage (§8.8/§8.9).
pub(crate) fn parse_length_value(
    value: &str,
    font_size: f32,
    percent_reference: f32,
) -> Option<f32> {
    let len: SvgLength = value.trim().parse().ok()?;
    Some(to_px_resolved(len, font_size, percent_reference))
}

/// Parse an SVG `points` attribute value into a list of coordinate pairs.
///
/// Used by both `<polyline>` and `<polygon>`.  Delegates to
/// [`svgtypes::PointsParser`] for SVG-spec-compliant parsing. A missing, empty,
/// or single-coordinate `points` value yields an empty/short list that is valid
/// but renders nothing (SVG 2: the attribute's initial value is `none`).
pub(crate) fn parse_points(get_attr: &dyn Fn(&str) -> Option<String>) -> Vec<Point> {
    let value = get_attr("points").unwrap_or_default();
    PointsParser::from(value.as_str())
        .map(|(x, y)| Point::new(x as f32, y as f32))
        .collect()
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f32 = 16.0;

    #[test]
    fn parse_length_missing_attr() {
        let result = parse_length("width", &|_| None, FS);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("missing SVG attribute: width"));
    }

    #[test]
    fn parse_length_simple() {
        let result = parse_length("x", &|_| Some("10".to_owned()), FS);
        assert_eq!(result.unwrap(), 10.0);
    }

    #[test]
    fn parse_length_with_px() {
        let result = parse_length("width", &|_| Some("50px".to_owned()), FS);
        assert_eq!(result.unwrap(), 50.0);
    }

    #[test]
    fn parse_length_with_percent() {
        let result = parse_length("width", &|_| Some("80%".to_owned()), FS);
        assert_eq!(result.unwrap(), 80.0);
    }

    #[test]
    fn parse_length_invalid() {
        let result = parse_length("r", &|_| Some("abc".to_owned()), FS);
        assert!(result.is_err());
    }

    #[test]
    fn parse_length_negative_allowed() {
        let result = parse_length("x", &|_| Some("-5".to_owned()), FS);
        assert_eq!(result.unwrap(), -5.0);
    }

    #[test]
    fn parse_length_with_em() {
        // 2em at default 16px = 32px
        let result = parse_length("x", &|_| Some("2em".to_owned()), 16.0);
        assert_eq!(result.unwrap(), 32.0);
    }

    #[test]
    fn parse_length_with_ex() {
        // 3ex at default 16px = 3 * 16 * 0.5 = 24px
        let result = parse_length("x", &|_| Some("3ex".to_owned()), 16.0);
        assert_eq!(result.unwrap(), 24.0);
    }

    #[test]
    fn parse_length_with_cm() {
        // 5cm = 5 * 96/2.54 px
        let result = parse_length("width", &|_| Some("5cm".to_owned()), FS);
        let expected = 5.0 * 96.0 / 2.54;
        assert!((result.unwrap() - expected).abs() < 0.01);
    }

    #[test]
    fn parse_length_with_font_size_override() {
        // 2em at 20px = 40px
        let result = parse_length("x", &|_| Some("2em".to_owned()), 20.0);
        assert_eq!(result.unwrap(), 40.0);
    }

    #[test]
    fn parse_length_with_in() {
        // 1in = 96px
        let result = parse_length("width", &|_| Some("1in".to_owned()), FS);
        assert_eq!(result.unwrap(), 96.0);
    }

    #[test]
    fn parse_length_with_pt() {
        // 12pt = 12 * 96/72 = 16px
        let result = parse_length("width", &|_| Some("12pt".to_owned()), FS);
        assert!((result.unwrap() - 16.0).abs() < 0.01);
    }

    #[test]
    fn parse_points_two_pairs() {
        let pts = parse_points(&|_| Some("10,20 30,40".to_owned()));
        assert_eq!(pts.len(), 2);
        assert!((pts[0].x - 10.0).abs() < 0.001);
        assert!((pts[0].y - 20.0).abs() < 0.001);
        assert!((pts[1].x - 30.0).abs() < 0.001);
        assert!((pts[1].y - 40.0).abs() < 0.001);
    }

    #[test]
    fn parse_points_three_pairs() {
        let pts = parse_points(&|_| Some("0,0 50,100 100,0".to_owned()));
        assert_eq!(pts.len(), 3);
    }

    #[test]
    fn parse_points_missing() {
        // A missing `points` attribute is the initial value `none`: valid but
        // empty (renders nothing).
        let pts = parse_points(&|_| None);
        assert!(pts.is_empty());
    }

    #[test]
    fn parse_points_single_pair() {
        // A single coordinate pair is valid but renders nothing (SVG 2).
        let pts = parse_points(&|_| Some("10,20".to_owned()));
        assert_eq!(pts.len(), 1);
    }

    #[test]
    fn parse_points_comma_variants() {
        let pts = parse_points(&|_| Some("10,20 30,40  50,60".to_owned()));
        assert_eq!(pts.len(), 3);
    }

    #[test]
    fn parse_length_value_px_and_bare() {
        assert_eq!(parse_length_value("40", FS, 100.0), Some(40.0));
        assert_eq!(parse_length_value("40px", FS, 100.0), Some(40.0));
    }

    #[test]
    fn parse_length_value_percent_resolves() {
        // 25% of a 200px reference = 50px.
        assert_eq!(parse_length_value("25%", FS, 200.0), Some(50.0));
    }

    #[test]
    fn parse_length_value_absolute_units() {
        // 1in = 96px.
        assert_eq!(parse_length_value("1in", FS, 0.0), Some(96.0));
    }

    #[test]
    fn parse_length_value_invalid_is_none() {
        // `auto` and garbage are not lengths.
        assert_eq!(parse_length_value("auto", FS, 100.0), None);
        assert_eq!(parse_length_value("nonsense", FS, 100.0), None);
    }
}
