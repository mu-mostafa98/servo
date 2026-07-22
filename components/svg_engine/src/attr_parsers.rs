/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;
use svgtypes::{Length as SvgLength, PointsParser};

use crate::error::{SvgEngineError, SvgResult};

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

fn to_px(len: SvgLength, font_size: f32) -> f32 {
    let n = len.number as f32;
    match len.unit {
        svgtypes::LengthUnit::None | svgtypes::LengthUnit::Px => n,
        svgtypes::LengthUnit::In => n * 96.0,
        svgtypes::LengthUnit::Cm => n * 96.0 / 2.54,
        svgtypes::LengthUnit::Mm => n * 96.0 / 25.4,
        svgtypes::LengthUnit::Pt => n * 96.0 / 72.0,
        svgtypes::LengthUnit::Pc => n * 96.0 / 6.0,
        svgtypes::LengthUnit::Em => n * font_size,
        svgtypes::LengthUnit::Ex => n * font_size * 0.5,
        svgtypes::LengthUnit::Percent => n,
    }
}

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
