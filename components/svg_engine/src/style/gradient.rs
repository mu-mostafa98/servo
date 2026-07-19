/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG gradient data types and parsing.
//!
//! These types store parsed gradient definitions collected from `<defs>`.
//! The actual rendering converts gradients into multiple `push_rect` calls
//! with interpolated colors (software gradient rendering).
//!
//! **No WebRender dependency** — pure SVG data types via `svgtypes::Color`.

use svgtypes::{Color as SvgColor, Length as SvgLength};

use crate::error::{SvgEngineError, SvgResult};
use crate::style::transform_ops::{TransformOp, parse_transform_str};

/// A paint server reference — either a solid color, gradient ID, or pattern ID.
#[derive(Debug, Clone)]
pub enum PaintServer {
    /// Solid color fill/stroke.
    Solid(SvgColor),
    /// Reference to a gradient defined elsewhere (e.g. `url(#myGrad)`).
    Gradient(String),
    /// Reference to a pattern defined elsewhere (e.g. `url(#myPattern)`).
    Pattern(String),
}

/// Definitions collected from `<defs>` during render tree construction.
#[derive(Debug, Clone)]
pub enum GradientDef {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

/// How gradient coordinates are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// How the gradient is extended beyond the 0..1 stop range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMethod {
    /// Clamp to the nearest stop color (default).
    Pad,
    /// Mirror the gradient (1→0→1...).
    Reflect,
    /// Repeat the gradient (0→1, 0→1...).
    Repeat,
}

#[derive(Debug, Clone, Copy)]
pub enum GradientLength {
    Number(f32),
    Percentage(f32),
}

impl GradientLength {
    pub fn to_object_bbox(self) -> f32 {
        match self {
            GradientLength::Number(v) => v,
            GradientLength::Percentage(p) => p / 100.0,
        }
    }

    pub fn to_user_space(self, axis_len: f32) -> f32 {
        match self {
            GradientLength::Number(v) => v,
            GradientLength::Percentage(p) => p / 100.0 * axis_len,
        }
    }
}

/// SVG `<linearGradient>` element data.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub id: String,
    pub x1: GradientLength,
    pub y1: GradientLength,
    pub x2: GradientLength,
    pub y2: GradientLength,
    pub units: GradientUnits,
    pub stops: Vec<GradientStop>,
    /// Transform applied to gradient coordinates (gradientTransform attribute).
    pub transform: Vec<TransformOp>,
    /// How the gradient extends beyond its stop range.
    pub spread_method: SpreadMethod,
}

/// SVG `<radialGradient>` element data.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub id: String,
    pub cx: GradientLength,
    pub cy: GradientLength,
    pub r: GradientLength,
    pub fx: GradientLength,
    pub fy: GradientLength,
    pub units: GradientUnits,
    pub stops: Vec<GradientStop>,
    /// Transform applied to gradient coordinates (gradientTransform attribute).
    pub transform: Vec<TransformOp>,
    /// How the gradient extends beyond its stop range.
    pub spread_method: SpreadMethod,
}

/// A single `<stop>` element in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    /// Offset in the range 0.0 – 1.0.
    pub offset: f32,
    /// Color at this offset.
    pub color: SvgColor,
}

impl PaintServer {
    /// Try to parse a paint server value from an attribute string.
    /// Supports: `"red"`, `"#ff0000"`, `"url(#myGrad)"`.
    pub fn from_attr(val: &str) -> Option<Self> {
        let val = val.trim();
        if val.starts_with("url(#") && val.ends_with(')') {
            let id = &val[5..val.len() - 1];
            if !id.is_empty() {
                return Some(PaintServer::Gradient(id.to_owned()));
            }
        }
        crate::style::color::parse_css_color(val).map(PaintServer::Solid)
    }
}

/// Parse a `<linearGradient>` or `<radialGradient>` element from its attributes.
/// `element_name` is `"linearGradient"` or `"radialGradient"`.
/// Returns the GradientDef keyed by id.
pub fn parse_gradient_element(
    element_name: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
    stop_attrs: &[Vec<(String, String)>], // list of stop attributes: [(("offset","0"),("stop-color","red")), ...]
) -> SvgResult<GradientDef> {
    let id = get_attr("id").unwrap_or_default();
    if id.is_empty() {
        return Err(SvgEngineError::MissingAttribute(
            "id on gradient".to_owned(),
        ));
    }

    let mut stops: Vec<GradientStop> = Vec::new();
    for attrs in stop_attrs {
        let offset = attrs
            .iter()
            .find(|(k, _)| k == "offset")
            .and_then(|(_, v)| parse_offset(v));
        let mut color = attrs
            .iter()
            .find(|(k, _)| k == "stop-color")
            .and_then(|(_, v)| crate::style::color::parse_css_color(v))
            .unwrap_or(SvgColor::new_rgb(0, 0, 0));
        if let Some(stop_opacity) = attrs
            .iter()
            .find(|(k, _)| k == "stop-opacity")
            .and_then(|(_, v)| parse_offset(v))
        {
            color.alpha = (color.alpha as f32 * stop_opacity).round() as u8;
        }
        if let Some(offset) = offset {
            stops.push(GradientStop { offset, color });
        }
    }

    if stops.is_empty() {
        stops.push(GradientStop {
            offset: 0.0,
            color: SvgColor::new_rgb(0, 0, 0),
        });
        stops.push(GradientStop {
            offset: 1.0,
            color: SvgColor::new_rgb(0, 0, 0),
        });
    }

    stops.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let gradient_units = get_attr("gradientUnits")
        .and_then(|val| parse_gradient_units(&val))
        .unwrap_or(GradientUnits::ObjectBoundingBox);

    let spread_method = get_attr("spreadMethod")
        .as_deref()
        .and_then(parse_spread_method)
        .unwrap_or(SpreadMethod::Pad);

    match element_name {
        "linearGradient" => {
            let x1 = parse_length_attr("x1", get_attr).unwrap_or(GradientLength::Number(0.0));
            let y1 = parse_length_attr("y1", get_attr).unwrap_or(GradientLength::Number(0.0));
            let x2 = parse_length_attr("x2", get_attr).unwrap_or(match gradient_units {
                GradientUnits::ObjectBoundingBox => GradientLength::Percentage(100.0),
                GradientUnits::UserSpaceOnUse => GradientLength::Number(100.0),
            });
            let y2 = parse_length_attr("y2", get_attr).unwrap_or(match gradient_units {
                GradientUnits::ObjectBoundingBox => GradientLength::Number(0.0),
                GradientUnits::UserSpaceOnUse => GradientLength::Number(0.0),
            });
            Ok(GradientDef::Linear(LinearGradient {
                id,
                x1,
                y1,
                x2,
                y2,
                units: gradient_units,
                stops,
                transform: parse_transform_str(&get_attr("gradientTransform").unwrap_or_default()),
                spread_method,
            }))
        },
        "radialGradient" => {
            let cx = parse_length_attr("cx", get_attr).unwrap_or(GradientLength::Percentage(50.0));
            let cy = parse_length_attr("cy", get_attr).unwrap_or(GradientLength::Percentage(50.0));
            let r = parse_length_attr("r", get_attr).unwrap_or(GradientLength::Percentage(50.0));
            let fx = parse_length_attr("fx", get_attr).unwrap_or(cx);
            let fy = parse_length_attr("fy", get_attr).unwrap_or(cy);
            Ok(GradientDef::Radial(RadialGradient {
                id,
                cx,
                cy,
                r,
                fx,
                fy,
                units: gradient_units,
                stops,
                transform: parse_transform_str(&get_attr("gradientTransform").unwrap_or_default()),
                spread_method,
            }))
        },
        _ => Err(SvgEngineError::UnsupportedFeature(format!(
            "unknown gradient: {element_name}"
        ))),
    }
}

/// Parse a length attribute from a gradient coordinate.
///
/// Delegates to [`svgtypes::Length`] for spec-compliant SVG length parsing
/// that handles all units (`px`, `em`, `ex`, `cm`, `mm`, `in`, `pt`, `pc`, `%`).
/// Percent values are kept explicit so they can be interpreted correctly
/// in objectBoundingBox coordinates.
fn parse_length_attr(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> Option<GradientLength> {
    let v = get_attr(attr)?;
    let len: SvgLength = v.parse().ok()?;
    if len.unit == svgtypes::LengthUnit::Percent {
        Some(GradientLength::Percentage(len.number as f32))
    } else {
        Some(GradientLength::Number(len.number as f32))
    }
}

fn parse_gradient_units(val: &str) -> Option<GradientUnits> {
    match val.trim() {
        "objectBoundingBox" => Some(GradientUnits::ObjectBoundingBox),
        "userSpaceOnUse" => Some(GradientUnits::UserSpaceOnUse),
        _ => None,
    }
}

fn parse_spread_method(val: &str) -> Option<SpreadMethod> {
    match val.trim() {
        "pad" => Some(SpreadMethod::Pad),
        "reflect" => Some(SpreadMethod::Reflect),
        "repeat" => Some(SpreadMethod::Repeat),
        _ => None,
    }
}

/// Parse a stop offset value (e.g. "0", "0.5", "50%", "100%").
fn parse_offset(val: &str) -> Option<f32> {
    let val = val.trim();
    if let Some(pct) = val.strip_suffix('%') {
        pct.trim().parse::<f32>().ok().map(|v| v / 100.0)
    } else {
        val.parse::<f32>().ok()
    }
}
