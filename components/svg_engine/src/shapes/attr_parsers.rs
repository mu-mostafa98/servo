/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG attribute parsing utilities.
//!
//! These functions parse raw SVG attribute strings into typed values.
//! They are shared by multiple shape or element types and live in their
//! own file to keep each shape file focused on its own `Build` impl.

use kurbo::Point;

use crate::error::{SvgEngineError, SvgResult};

/// Parse a named SVG length attribute (e.g. `x="10"`, `width="50"`).
/// Strips trailing `px` suffix and returns the raw float value.
pub(crate) fn parse_length(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> SvgResult<f32> {
    let value =
        get_attr(attr).ok_or_else(|| SvgEngineError::MissingAttribute(attr.to_owned()))?;
    value
        .trim_end_matches("px")
        .trim()
        .parse::<f32>()
        .map_err(|e| SvgEngineError::ParseError(format!("{attr}: {e}")))
}

/// Parse an SVG `points` attribute value into a list of coordinate pairs.
///
/// Used by both `<polyline>` and `<polygon>`.
pub(crate) fn parse_points(
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> SvgResult<Vec<Point>> {
    let value =
        get_attr("points").ok_or_else(|| SvgEngineError::MissingAttribute("points".to_owned()))?;
    let coords: Vec<f64> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    let points: Vec<Point> = coords
        .chunks(2)
        .filter_map(|chunk| {
            let x = *chunk.first()?;
            let y = *chunk.get(1)?;
            Some(Point::new(x, y))
        })
        .collect();

    if points.len() < 2 {
        return Err(SvgEngineError::ParseError(
            "points attribute requires at least 2 coordinate pairs".to_owned(),
        ));
    }
    Ok(points)
}
