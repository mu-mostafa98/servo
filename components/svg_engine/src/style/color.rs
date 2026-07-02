/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG color resolution and CSS color parsing.
//!
//! These helpers convert from Servo's computed color types and from
//! raw CSS colour strings into [`webrender_api::ColorF`].

use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::svg::{SVGPaint, SVGPaintKind};
use webrender_api::ColorF;

/// Result of resolving an SVG paint value — either a solid color
/// or a paint server reference (gradient/pattern URL).
pub(crate) enum ResolvedPaint {
    Color(ColorF),
    PaintServer(String),
    None,
}

/// Resolve an [`SVGPaint`] to either a [`ColorF`] or a paint server URL.
pub(crate) fn resolve_svg_paint(
    svg_paint: &SVGPaint,
    computed_values: &ComputedValues,
) -> ResolvedPaint {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            ResolvedPaint::Color(ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
            ))
        },
        SVGPaintKind::None => ResolvedPaint::None,
        SVGPaintKind::PaintServer(url) => {
            // Extract gradient/pattern URL fragment: url(#myGrad) → "myGrad"
            match url {
                style::url::ComputedUrl::Valid(u) => {
                    if let Some(fragment) = u.fragment() {
                        return ResolvedPaint::PaintServer(fragment.to_owned());
                    }
                },
                style::url::ComputedUrl::Invalid(s) => {
                    let trimmed = s.trim_start_matches('#');
                    if !trimmed.is_empty() {
                        return ResolvedPaint::PaintServer(trimmed.to_owned());
                    }
                },
            }
            ResolvedPaint::None
        },
        _ => ResolvedPaint::None,
    }
}

/// Parse a CSS color value (hex `#rrggbb` or named color).
pub(crate) fn parse_css_color(val: &str) -> Option<ColorF> {
    let val = val.trim();
    if val == "none" || val == "transparent" {
        return None;
    }
    if val.starts_with('#') {
        let hex = &val[1..];
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(ColorF::new(
                r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0,
            ));
        }
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                return Some(ColorF::new(
                    ((rgb >> 16) & 0xFF) as f32 / 255.0,
                    ((rgb >> 8) & 0xFF) as f32 / 255.0,
                    (rgb & 0xFF) as f32 / 255.0,
                    1.0,
                ));
            }
        }
    }
    match val {
        "red" => Some(ColorF::new(1.0, 0.0, 0.0, 1.0)),
        "green" => Some(ColorF::new(0.0, 0.502, 0.0, 1.0)),
        "blue" => Some(ColorF::new(0.0, 0.0, 1.0, 1.0)),
        "white" => Some(ColorF::new(1.0, 1.0, 1.0, 1.0)),
        "black" => Some(ColorF::new(0.0, 0.0, 0.0, 1.0)),
        "yellow" => Some(ColorF::new(1.0, 1.0, 0.0, 1.0)),
        "orange" => Some(ColorF::new(1.0, 0.647, 0.0, 1.0)),
        "purple" => Some(ColorF::new(0.502, 0.0, 0.502, 1.0)),
        "gray" | "grey" => Some(ColorF::new(0.5, 0.5, 0.5, 1.0)),
        _ => None,
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_color_none() {
        assert!(parse_css_color("none").is_none());
    }

    #[test]
    fn parse_color_transparent() {
        assert!(parse_css_color("transparent").is_none());
    }

    #[test]
    fn parse_color_hex_6() {
        let c = parse_css_color("#ff0000").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_hex_3() {
        let c = parse_css_color("#0f0").unwrap();
        assert!((c.r - 0.0).abs() < 0.01);
        assert!((c.g - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_hex_blue() {
        let c = parse_css_color("#0000ff").unwrap();
        assert!((c.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_named_red() {
        let c = parse_css_color("red").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_named_black() {
        let c = parse_css_color("black").unwrap();
        assert!((c.r - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_gray_grey_equivalent() {
        let gray = parse_css_color("gray").unwrap();
        let grey = parse_css_color("grey").unwrap();
        assert!((gray.r - grey.r).abs() < 0.001);
    }
}
