/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG gradient data types.
//!
//! These types store parsed gradient definitions collected from `<defs>`.
//! The actual rendering converts gradients into multiple `push_rect` calls
//! with interpolated colors (software gradient rendering).

use webrender_api::ColorF;

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
        // Not a url() — try to parse as a color.
        crate::style::color::parse_css_color(val).map(PaintServer::Solid)
    }
}
