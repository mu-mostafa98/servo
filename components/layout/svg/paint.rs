/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint server and color parsing from SVG attribute strings.
//!
//! These are build-time converters from DOM attribute text into the engine's
//! [`svg_engine::style::gradient::PaintServer`] type, so they live in layout
//! (not the engine's `model`), which never parses attribute strings itself.

use svg_engine::style::gradient::PaintServer;
use svg_engine::units::Id;
use svgtypes::Color as SvgColor;

/// Parse a CSS/SVG color value into an [`svgtypes::Color`].
///
/// Returns `None` for `none` (no paint), `transparent`, and unparseable values.
pub(crate) fn parse_css_color(val: &str) -> Option<SvgColor> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("none") || val.eq_ignore_ascii_case("transparent") {
        return None;
    }
    val.parse().ok()
}

/// Try to parse a paint server value from an attribute string.
/// Supports: `"red"`, `"#ff0000"`, `"url(#myGrad)"`.
///
/// URL references yield a transient [`PaintServer::Ref`], which is resolved
/// to a typed handle later by the build layer's reference-resolution pass.
pub(crate) fn parse_paint_server(val: &str) -> Option<PaintServer> {
    let val = val.trim();
    if val.starts_with("url(#") && val.ends_with(')') {
        let id = &val[5..val.len() - 1];
        if !id.is_empty() {
            return Some(PaintServer::Ref(Id::new(id)));
        }
    }
    parse_css_color(val).map(PaintServer::Solid)
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use svgtypes::Color as SvgColor;

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
        assert_eq!(c.red, 255);
    }
    #[test]
    fn parse_color_hex_3() {
        let c = parse_css_color("#0f0").unwrap();
        assert_eq!(c.red, 0);
    }
    #[test]
    fn parse_color_named_red() {
        let c = parse_css_color("red").unwrap();
        assert_eq!(c, SvgColor::red());
    }
    #[test]
    fn parse_color_gray_grey_equivalent() {
        assert_eq!(
            parse_css_color("gray").unwrap(),
            parse_css_color("grey").unwrap()
        );
    }
    #[test]
    fn parse_color_rgb_function() {
        let c = parse_css_color("rgb(255, 0, 0)").unwrap();
        assert_eq!(c.red, 255);
    }
    #[test]
    fn parse_color_invalid() {
        assert!(parse_css_color("notacolor").is_none());
    }
    #[test]
    fn parse_color_uppercase_named() {
        assert!(parse_css_color("RED").is_some());
        assert!(parse_css_color("CornflowerBlue").is_some());
    }

    #[test]
    fn paint_server_solid_color() {
        match parse_paint_server("red").unwrap() {
            PaintServer::Solid(_) => {},
            _ => panic!("expected Solid"),
        }
    }

    #[test]
    fn paint_server_url_ref() {
        match parse_paint_server("url(#myGrad)").unwrap() {
            PaintServer::Ref(id) => assert_eq!(id.as_str(), "myGrad"),
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn paint_server_none() {
        assert!(parse_paint_server("none").is_none());
    }
}
