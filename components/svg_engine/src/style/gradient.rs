/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG gradient data types and parsing.
//!
//! These types store parsed gradient definitions collected from `<defs>`.
//! The actual rendering converts gradients into multiple `push_rect` calls
//! with interpolated colors (software gradient rendering).

use webrender_api::ColorF;

use crate::error::{SvgEngineError, SvgResult};

/// A paint server reference — either a solid color or a gradient ID.
#[derive(Debug, Clone)]
pub enum PaintServer {
    /// Solid color fill/stroke.
    Solid(ColorF),
    /// Reference to a gradient defined elsewhere (e.g. `url(#myGrad)`).
    Gradient(String),
}

/// Definitions collected from `<defs>` during render tree construction.
#[derive(Debug, Clone)]
pub enum GradientDef {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

/// SVG `<linearGradient>` element data.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stops: Vec<GradientStop>,
}

/// SVG `<radialGradient>` element data.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub id: String,
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
    pub fx: f32,
    pub fy: f32,
    pub stops: Vec<GradientStop>,
}

/// A single `<stop>` element in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    /// Offset in the range 0.0 – 1.0.
    pub offset: f32,
    /// Color at this offset.
    pub color: ColorF,
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
        return Err(SvgEngineError::MissingAttribute("id on gradient".to_owned()));
    }

    let mut stops: Vec<GradientStop> = Vec::new();
    for attrs in stop_attrs {
        let offset = attrs.iter()
            .find(|(k, _)| k == "offset")
            .and_then(|(_, v)| parse_offset(v));
        let color = attrs.iter()
            .find(|(k, _)| k == "stop-color")
            .and_then(|(_, v)| crate::style::color::parse_css_color(v))
            .unwrap_or(ColorF::new(0.0, 0.0, 0.0, 1.0));
        if let Some(offset) = offset {
            stops.push(GradientStop { offset, color });
        }
    }

    if stops.is_empty() {
        stops.push(GradientStop { offset: 0.0, color: ColorF::new(0.0, 0.0, 0.0, 1.0) });
        stops.push(GradientStop { offset: 1.0, color: ColorF::new(0.0, 0.0, 0.0, 1.0) });
    }

    match element_name {
        "linearGradient" => {
            let x1 = parse_length_attr("x1", get_attr).unwrap_or(0.0);
            let y1 = parse_length_attr("y1", get_attr).unwrap_or(0.0);
            let x2 = parse_length_attr("x2", get_attr).unwrap_or(100.0);
            let y2 = parse_length_attr("y2", get_attr).unwrap_or(100.0);
            Ok(GradientDef::Linear(LinearGradient { id, x1, y1, x2, y2, stops }))
        },
        "radialGradient" => {
            let cx = parse_length_attr("cx", get_attr).unwrap_or(50.0);
            let cy = parse_length_attr("cy", get_attr).unwrap_or(50.0);
            let r = parse_length_attr("r", get_attr).unwrap_or(50.0);
            let fx = parse_length_attr("fx", get_attr).unwrap_or(cx);
            let fy = parse_length_attr("fy", get_attr).unwrap_or(cy);
            Ok(GradientDef::Radial(RadialGradient { id, cx, cy, r, fx, fy, stops }))
        },
        _ => Err(SvgEngineError::UnsupportedFeature(format!("unknown gradient: {element_name}"))),
    }
}

/// Parse a length attribute (strips px, falls back to f32).
fn parse_length_attr(attr: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<f32> {
    let v = get_attr(attr)?;
    let trimmed = v.trim_end_matches("px").trim().trim_end_matches('%').trim();
    trimmed.parse::<f32>().ok()
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
