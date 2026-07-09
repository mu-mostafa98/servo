/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG attribute parsing utilities.
//!
//! These functions parse raw SVG attribute strings into typed values.
//! They are shared by multiple shape or element types and live in their
//! own file to keep each shape file focused on its own `Build` impl.
//!
//! Length parsing is backed by [`svgtypes::Length`] for spec‑compliant handling
//! of all SVG length units (`px`, `em`, `ex`, `in`, `cm`, `mm`, `pt`, `pc`, `%`).

use kurbo::Point;
use svgtypes::{Length as SvgLength, PointsParser};

use crate::error::{SvgEngineError, SvgResult};

/// Parse a named SVG length attribute (e.g. `x="10"`, `width="50%"`).
///
/// Handles all CSS/SVG length units via [`svgtypes::Length`].
/// Converts `em`/`ex`/`in`/`cm`/`mm`/`pt`/`pc` to pixels using the
/// provided `font_size` (default 16px for SVG).
///
/// Percent values are returned as-is (caller must resolve against
/// the appropriate reference dimension).
pub fn parse_length(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
    font_size: f32,
) -> SvgResult<f32> {
    let value = get_attr(attr).ok_or_else(|| SvgEngineError::MissingAttribute(attr.to_owned()))?;
    let len: SvgLength = value
        .parse()
        .map_err(|e| SvgEngineError::ParseError(format!("{attr}: {e}")))?;
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

/// Parse an SVG `points` attribute value into a list of coordinate pairs.
///
/// Used by both `<polyline>` and `<polygon>`.  Delegates to
/// [`svgtypes::PointsParser`] for SVG-spec-compliant parsing.
pub fn parse_points(get_attr: &dyn Fn(&str) -> Option<String>) -> SvgResult<Vec<Point>> {
    let value =
        get_attr("points").ok_or_else(|| SvgEngineError::MissingAttribute("points".to_owned()))?;
    let points: Vec<Point> = PointsParser::from(value.as_str())
        .map(|(x, y)| Point::new(x, y))
        .collect();

    if points.len() < 2 {
        return Err(SvgEngineError::ParseError(
            "points attribute requires at least 2 coordinate pairs".to_owned(),
        ));
    }
    Ok(points)
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing SVG attribute: width")
        );
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
        let result = parse_points(&|_| Some("10,20 30,40".to_owned()));
        let pts = result.unwrap();
        assert_eq!(pts.len(), 2);
        assert!((pts[0].x - 10.0).abs() < 0.001);
        assert!((pts[0].y - 20.0).abs() < 0.001);
        assert!((pts[1].x - 30.0).abs() < 0.001);
        assert!((pts[1].y - 40.0).abs() < 0.001);
    }

    #[test]
    fn parse_points_three_pairs() {
        let result = parse_points(&|_| Some("0,0 50,100 100,0".to_owned()));
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn parse_points_missing() {
        let result = parse_points(&|_| None);
        assert!(result.is_err());
    }

    #[test]
    fn parse_points_too_few() {
        let result = parse_points(&|_| Some("10,20".to_owned()));
        assert!(result.is_err());
    }

    #[test]
    fn parse_points_comma_variants() {
        let result = parse_points(&|_| Some("10,20 30,40  50,60".to_owned()));
        assert_eq!(result.unwrap().len(), 3);
    }
}
