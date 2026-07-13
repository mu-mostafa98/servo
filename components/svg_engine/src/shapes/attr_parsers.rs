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
