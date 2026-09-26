/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint server and color parsing from SVG attribute strings.
//!
//! These are build-time converters from DOM attribute text into the engine's
//! [`svg_engine::style::paint_servers::PaintServer`] type, so they live in layout
//! (not the engine's `model`), which never parses attribute strings itself.

use svg_engine::style::paint_servers::PaintServer;
use svg_engine::style::{PaintOperation, PaintOrder};
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
///
/// Supports the SVG 2 `<paint>` grammar:
/// `none | <color> | <url> [none | <color>]? | context-fill | context-stroke`.
///
/// - `"red"`, `"#ff0000"` → [`PaintServer::Solid`].
/// - `"url(#myGrad)"` → [`PaintServer::Ref`] with no fallback.
/// - `"url(#myGrad) red"` → [`PaintServer::Ref`] with a red fallback.
/// - `"context-fill"` / `"context-stroke"` → the corresponding keyword.
///
/// URL references yield a transient [`PaintServer::Ref`], which is resolved
/// to a typed handle (or its fallback color) later by the build layer's
/// reference-resolution pass.
pub(crate) fn parse_paint_server(val: &str) -> Option<PaintServer> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("context-fill") {
        return Some(PaintServer::ContextFill);
    }
    if val.eq_ignore_ascii_case("context-stroke") {
        return Some(PaintServer::ContextStroke);
    }
    if val.starts_with("url(") {
        if let Some(close) = val.find(')') {
            let url_part = &val[..close + 1];
            let rest = val[close + 1..].trim();
            // Strip `url(` / `)`, any quotes, and an optional leading `#`.
            let inner = url_part[4..url_part.len() - 1]
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .trim_start_matches('#')
                .trim();
            if inner.is_empty() {
                return parse_css_color(val).map(PaintServer::Solid);
            }
            let fallback = if rest.is_empty() || rest.eq_ignore_ascii_case("none") {
                None
            } else {
                parse_css_color(rest)
            };
            return Some(PaintServer::Ref {
                id: Id::new(inner),
                fallback,
            });
        }
    }
    parse_css_color(val).map(PaintServer::Solid)
}

/// Parse a `paint-order` value into an ordered triple of painting operations.
///
/// SVG 2 grammar: `normal | [fill || stroke || markers]`.  Omitted operations
/// are appended in the default order (fill, stroke, markers).
pub(crate) fn parse_paint_order(val: &str) -> Option<PaintOrder> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("normal") {
        return Some(PaintOrder::default());
    }
    let mut listed: Vec<PaintOperation> = Vec::new();
    for token in val.split_whitespace() {
        let op = match token {
            "fill" => PaintOperation::Fill,
            "stroke" => PaintOperation::Stroke,
            "markers" => PaintOperation::Markers,
            _ => return None,
        };
        if !listed.contains(&op) {
            listed.push(op);
        }
    }
    if listed.is_empty() {
        return None;
    }
    for op in [
        PaintOperation::Fill,
        PaintOperation::Stroke,
        PaintOperation::Markers,
    ] {
        if !listed.contains(&op) {
            listed.push(op);
        }
    }
    Some(PaintOrder {
        order: [listed[0], listed[1], listed[2]],
    })
}

/// Parse a CSS color string (named, hex, `rgb()`/`rgba()`) into `(r, g, b, a)`
/// float components in `[0, 1]`.
///
/// Used by filter primitives (`flood-color`), which store colors as raw floats.
/// Falls back to opaque black when the value is absent, `none`, or unparseable.
pub(crate) fn parse_color_rgba(input: &str) -> (f32, f32, f32, f32) {
    match parse_css_color(input) {
        Some(c) => (
            c.red as f32 / 255.0,
            c.green as f32 / 255.0,
            c.blue as f32 / 255.0,
            c.alpha as f32 / 255.0,
        ),
        None => (0.0, 0.0, 0.0, 1.0),
    }
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
    fn parse_color_rgba_named() {
        assert_eq!(parse_color_rgba("red"), (1.0, 0.0, 0.0, 1.0));
    }
    #[test]
    fn parse_color_rgba_default_black() {
        assert_eq!(parse_color_rgba("none"), (0.0, 0.0, 0.0, 1.0));
        assert_eq!(parse_color_rgba("transparent"), (0.0, 0.0, 0.0, 1.0));
        assert_eq!(parse_color_rgba("notacolor"), (0.0, 0.0, 0.0, 1.0));
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
            PaintServer::Ref { id, fallback } => {
                assert_eq!(id.as_str(), "myGrad");
                assert!(fallback.is_none());
            },
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn paint_server_url_ref_with_color_fallback() {
        match parse_paint_server("url(#myGrad) red").unwrap() {
            PaintServer::Ref { id, fallback } => {
                assert_eq!(id.as_str(), "myGrad");
                assert_eq!(fallback, Some(SvgColor::red()));
            },
            _ => panic!("expected Ref with fallback"),
        }
    }

    #[test]
    fn paint_server_url_ref_with_none_fallback() {
        match parse_paint_server("url(#myGrad) none").unwrap() {
            PaintServer::Ref { id, fallback } => {
                assert_eq!(id.as_str(), "myGrad");
                assert!(fallback.is_none());
            },
            _ => panic!("expected Ref with none fallback"),
        }
    }

    #[test]
    fn paint_server_context_keywords() {
        assert!(matches!(
            parse_paint_server("context-fill"),
            Some(PaintServer::ContextFill)
        ));
        assert!(matches!(
            parse_paint_server("context-stroke"),
            Some(PaintServer::ContextStroke)
        ));
    }

    #[test]
    fn paint_order_parsing() {
        let po = parse_paint_order("normal").unwrap();
        assert_eq!(po.order[0], PaintOperation::Fill);
        assert_eq!(po.order[1], PaintOperation::Stroke);
        assert_eq!(po.order[2], PaintOperation::Markers);

        let po = parse_paint_order("stroke").unwrap();
        assert!(po.stroke_before_fill());
        assert_eq!(po.order[0], PaintOperation::Stroke);

        let po = parse_paint_order("stroke markers").unwrap();
        assert_eq!(po.order, [
            PaintOperation::Stroke,
            PaintOperation::Markers,
            PaintOperation::Fill,
        ]);

        let po = parse_paint_order("markers stroke fill").unwrap();
        assert_eq!(po.order, [
            PaintOperation::Markers,
            PaintOperation::Stroke,
            PaintOperation::Fill,
        ]);

        assert!(parse_paint_order("bogus").is_none());
    }

    #[test]
    fn paint_server_none() {
        assert!(parse_paint_server("none").is_none());
    }
}
