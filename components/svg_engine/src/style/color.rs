/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG color parsing powered by [`svgtypes`](https://docs.rs/svgtypes).
//!
//! Supports all CSS3 color formats: named colors, `#rgb` / `#rrggbb`, `#rgba` / `#rrggbbaa`,
//! `rgb()`, `rgba()`, `hsl()`, `hsla()`, and `transparent`.
//!
//! **No WebRender dependency** — returns [`svgtypes::Color`], a pure-data SVG color type.

use svgtypes::Color as SvgColor;

/// Parse a CSS/SVG color value into an [`svgtypes::Color`].
///
/// Returns `None` for `none` (no paint), `transparent`, and unparseable values.
pub fn parse_css_color(val: &str) -> Option<SvgColor> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("none") || val.eq_ignore_ascii_case("transparent") {
        return None;
    }
    val.parse().ok()
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;
    use svgtypes::Color as SvgColor;

    #[test]
    fn parse_color_none() { assert!(parse_css_color("none").is_none()); }
    #[test]
    fn parse_color_transparent() { assert!(parse_css_color("transparent").is_none()); }
    #[test]
    fn parse_color_hex_6() { let c = parse_css_color("#ff0000").unwrap(); assert_eq!(c.red, 255); }
    #[test]
    fn parse_color_hex_3() { let c = parse_css_color("#0f0").unwrap(); assert_eq!(c.red, 0); }
    #[test]
    fn parse_color_named_red() { let c = parse_css_color("red").unwrap(); assert_eq!(c, SvgColor::red()); }
    #[test]
    fn parse_color_gray_grey_equivalent() {
        assert_eq!(parse_css_color("gray").unwrap(), parse_css_color("grey").unwrap());
    }
    #[test]
    fn parse_color_navy() { assert!(parse_css_color("navy").is_some()); }
    #[test]
    fn parse_color_rgb_function() {
        let c = parse_css_color("rgb(255, 0, 0)").unwrap();
        assert_eq!(c.red, 255);
    }
    #[test]
    fn parse_color_rgba_function() {
        let c = parse_css_color("rgba(255, 0, 0, 0.5)").unwrap();
        assert_eq!(c.red, 255);
        assert_eq!(c.alpha, 128);
    }
    #[test]
    fn parse_color_hsl() {
        let c = parse_css_color("hsl(120, 100%, 50%)").unwrap();
        assert_eq!(c.green, 255);
    }
    #[test]
    fn parse_color_hex_8() {
        let c = parse_css_color("#ff000080").unwrap();
        assert_eq!(c.red, 255);
        assert_eq!(c.alpha, 128);
    }
    #[test]
    fn parse_color_invalid() { assert!(parse_css_color("notacolor").is_none()); }
    #[test]
    fn parse_color_number_prefix() { assert!(parse_css_color("123").is_none()); }
    #[test]
    fn parse_color_uppercase_named() {
        assert!(parse_css_color("RED").is_some());
        assert!(parse_css_color("CornflowerBlue").is_some());
    }
}
