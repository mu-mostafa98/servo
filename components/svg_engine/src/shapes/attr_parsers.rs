/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::Length as SvgLength;

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
