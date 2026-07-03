/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG color parsing.
//!
//! Parses CSS color strings into [`webrender_api::ColorF`].

use webrender_api::ColorF;

/// All 148 CSS named colors with their (r, g, b) values.
const NAMED_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("aliceblue", (240, 248, 255)),
    ("antiquewhite", (250, 235, 215)),
    ("aqua", (0, 255, 255)),
    ("aquamarine", (127, 255, 212)),
    ("azure", (240, 255, 255)),
    ("beige", (245, 245, 220)),
    ("bisque", (255, 228, 196)),
    ("black", (0, 0, 0)),
    ("blanchedalmond", (255, 235, 205)),
    ("blue", (0, 0, 255)),
    ("blueviolet", (138, 43, 226)),
    ("brown", (165, 42, 42)),
    ("burlywood", (222, 184, 135)),
    ("cadetblue", (95, 158, 160)),
    ("chartreuse", (127, 255, 0)),
    ("chocolate", (210, 105, 30)),
    ("coral", (255, 127, 80)),
    ("cornflowerblue", (100, 149, 237)),
    ("cornsilk", (255, 248, 220)),
    ("crimson", (220, 20, 60)),
    ("cyan", (0, 255, 255)),
    ("darkblue", (0, 0, 139)),
    ("darkcyan", (0, 139, 139)),
    ("darkgoldenrod", (184, 134, 11)),
    ("darkgray", (169, 169, 169)),
    ("darkgreen", (0, 100, 0)),
    ("darkgrey", (169, 169, 169)),
    ("darkkhaki", (189, 183, 107)),
    ("darkmagenta", (139, 0, 139)),
    ("darkolivegreen", (85, 107, 47)),
    ("darkorange", (255, 140, 0)),
    ("darkorchid", (153, 50, 204)),
    ("darkred", (139, 0, 0)),
    ("darksalmon", (233, 150, 122)),
    ("darkseagreen", (143, 188, 143)),
    ("darkslateblue", (72, 61, 139)),
    ("darkslategray", (47, 79, 79)),
    ("darkslategrey", (47, 79, 79)),
    ("darkturquoise", (0, 206, 209)),
    ("darkviolet", (148, 0, 211)),
    ("deeppink", (255, 20, 147)),
    ("deepskyblue", (0, 191, 255)),
    ("dimgray", (105, 105, 105)),
    ("dimgrey", (105, 105, 105)),
    ("dodgerblue", (30, 144, 255)),
    ("firebrick", (178, 34, 34)),
    ("floralwhite", (255, 250, 240)),
    ("forestgreen", (34, 139, 34)),
    ("fuchsia", (255, 0, 255)),
    ("gainsboro", (220, 220, 220)),
    ("ghostwhite", (248, 248, 255)),
    ("gold", (255, 215, 0)),
    ("goldenrod", (218, 165, 32)),
    ("gray", (128, 128, 128)),
    ("green", (0, 128, 0)),
    ("greenyellow", (173, 255, 47)),
    ("grey", (128, 128, 128)),
    ("honeydew", (240, 255, 240)),
    ("hotpink", (255, 105, 180)),
    ("indianred", (205, 92, 92)),
    ("indigo", (75, 0, 130)),
    ("ivory", (255, 255, 240)),
    ("khaki", (240, 230, 140)),
    ("lavender", (230, 230, 250)),
    ("lavenderblush", (255, 240, 245)),
    ("lawngreen", (124, 252, 0)),
    ("lemonchiffon", (255, 250, 205)),
    ("lightblue", (173, 216, 230)),
    ("lightcoral", (240, 128, 128)),
    ("lightcyan", (224, 255, 255)),
    ("lightgoldenrodyellow", (250, 250, 210)),
    ("lightgray", (211, 211, 211)),
    ("lightgreen", (144, 238, 144)),
    ("lightgrey", (211, 211, 211)),
    ("lightpink", (255, 182, 193)),
    ("lightsalmon", (255, 160, 122)),
    ("lightseagreen", (32, 178, 170)),
    ("lightskyblue", (135, 206, 250)),
    ("lightslategray", (119, 136, 153)),
    ("lightslategrey", (119, 136, 153)),
    ("lightsteelblue", (176, 196, 222)),
    ("lightyellow", (255, 255, 224)),
    ("lime", (0, 255, 0)),
    ("limegreen", (50, 205, 50)),
    ("linen", (250, 240, 230)),
    ("magenta", (255, 0, 255)),
    ("maroon", (128, 0, 0)),
    ("mediumaquamarine", (102, 205, 170)),
    ("mediumblue", (0, 0, 205)),
    ("mediumorchid", (186, 85, 211)),
    ("mediumpurple", (147, 112, 219)),
    ("mediumseagreen", (60, 179, 113)),
    ("mediumslateblue", (123, 104, 238)),
    ("mediumspringgreen", (0, 250, 154)),
    ("mediumturquoise", (72, 209, 204)),
    ("mediumvioletred", (199, 21, 133)),
    ("midnightblue", (25, 25, 112)),
    ("mintcream", (245, 255, 250)),
    ("mistyrose", (255, 228, 225)),
    ("moccasin", (255, 228, 181)),
    ("navajowhite", (255, 222, 173)),
    ("navy", (0, 0, 128)),
    ("oldlace", (253, 245, 230)),
    ("olive", (128, 128, 0)),
    ("olivedrab", (107, 142, 35)),
    ("orange", (255, 165, 0)),
    ("orangered", (255, 69, 0)),
    ("orchid", (218, 112, 214)),
    ("palegoldenrod", (238, 232, 170)),
    ("palegreen", (152, 251, 152)),
    ("paleturquoise", (175, 238, 238)),
    ("palevioletred", (219, 112, 147)),
    ("papayawhip", (255, 239, 213)),
    ("peachpuff", (255, 218, 185)),
    ("peru", (205, 133, 63)),
    ("pink", (255, 192, 203)),
    ("plum", (221, 160, 221)),
    ("powderblue", (176, 224, 230)),
    ("purple", (128, 0, 128)),
    ("red", (255, 0, 0)),
    ("rosybrown", (188, 143, 143)),
    ("royalblue", (65, 105, 225)),
    ("saddlebrown", (139, 69, 19)),
    ("salmon", (250, 128, 114)),
    ("sandybrown", (244, 164, 96)),
    ("seagreen", (46, 139, 87)),
    ("seashell", (255, 245, 238)),
    ("sienna", (160, 82, 45)),
    ("silver", (192, 192, 192)),
    ("skyblue", (135, 206, 235)),
    ("slateblue", (106, 90, 205)),
    ("slategray", (112, 128, 144)),
    ("slategrey", (112, 128, 144)),
    ("snow", (255, 250, 250)),
    ("springgreen", (0, 255, 127)),
    ("steelblue", (70, 130, 180)),
    ("tan", (210, 180, 140)),
    ("teal", (0, 128, 128)),
    ("thistle", (216, 191, 216)),
    ("tomato", (255, 99, 71)),
    ("turquoise", (64, 224, 208)),
    ("violet", (238, 130, 238)),
    ("wheat", (245, 222, 179)),
    ("white", (255, 255, 255)),
    ("whitesmoke", (245, 245, 245)),
    ("yellow", (255, 255, 0)),
    ("yellowgreen", (154, 205, 50)),
];

/// Parse a CSS color value (hex `#rrggbb`, `#rgb`, or named color).
pub fn parse_css_color(val: &str) -> Option<ColorF> {
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
            return Some(ColorF::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0));
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
        return None;
    }
    // Linear scan of named colors (small array, fine for this use case).
    let (r, g, b) = NAMED_COLORS.iter().find(|(name, _)| *name == val)?.1;
    Some(ColorF::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0))
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_color_none() { assert!(parse_css_color("none").is_none()); }
    #[test]
    fn parse_color_transparent() { assert!(parse_css_color("transparent").is_none()); }
    #[test]
    fn parse_color_hex_6() { let c = parse_css_color("#ff0000").unwrap(); assert!((c.r - 1.0).abs() < 0.01); }
    #[test]
    fn parse_color_hex_3() { let c = parse_css_color("#0f0").unwrap(); assert!((c.r - 0.0).abs() < 0.01); }
    #[test]
    fn parse_color_named_red() { let c = parse_css_color("red").unwrap(); assert!((c.r - 1.0).abs() < 0.01); }
    #[test]
    fn parse_color_gray_grey_equivalent() {
        let gray = parse_css_color("gray").unwrap();
        let grey = parse_css_color("grey").unwrap();
        assert!((gray.r - grey.r).abs() < 0.001);
    }
    #[test]
    fn parse_color_teal() {
        let c = parse_css_color("teal").unwrap();
        assert!((c.r - 0.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
        assert!((c.b - 0.502).abs() < 0.01);
    }
    #[test]
    fn parse_color_coral() {
        let c = parse_css_color("coral").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.498).abs() < 0.01);
        assert!((c.b - 0.314).abs() < 0.01);
    }
    #[test]
    fn parse_color_aqua_cyan_equivalent() {
        let a = parse_css_color("aqua").unwrap();
        let c = parse_css_color("cyan").unwrap();
        assert!((a.r - c.r).abs() < 0.001);
        assert!((a.g - c.g).abs() < 0.001);
        assert!((a.b - c.b).abs() < 0.001);
    }
    #[test]
    fn parse_color_navy() { assert!(parse_css_color("navy").is_some()); }
    #[test]
    fn parse_color_invalid() { assert!(parse_css_color("notacolor").is_none()); }
    #[test]
    fn parse_color_number_prefix() { assert!(parse_css_color("123").is_none()); }
}
