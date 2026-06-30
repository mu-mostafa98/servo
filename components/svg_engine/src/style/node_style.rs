/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node style — the combined presentation attributes for a single
//! SVG render node.
//!
//! This module defines [`NodeStyle`], the central style container that
//! aggregates fill, stroke, transform, and future rendering hints into
//! a single struct consumed by the shape renderers.

use webrender_api::ColorF;
use style::properties::ComputedValues;

use crate::builder::{Build, SvgBuildInput};
use crate::error::SvgResult;
use super::transform_ops::TransformOp;
use super::fill::{FillParams, FillRule};
use super::stroke::{StrokeParams, LineCap, LineJoin};
use super::color;
use super::hints::RenderHints;
use super::visibility::{Visibility, Display};
use super::effects::NodeEffects;
use super::FromComputedValues;
use super::FromCssAttrs;

/// Combined fill + stroke styling for an SVG render node.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub visibility: Visibility,
    pub display: Display,
    pub transform: Vec<TransformOp>,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
    pub render_hints: Option<RenderHints>,
    pub effects: Option<NodeEffects>,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill: None,
            stroke: None,
            render_hints: None,
            effects: None,
        }
    }
}

// ======================= FromComputedValues for NodeStyle =======================

impl FromComputedValues for NodeStyle {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        Some(NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill: FillParams::from_computed_values(values),
            stroke: StrokeParams::from_computed_values(values),
            render_hints: None,
            effects: None,
        })
    }
}

// ======================= FromCssAttrs for NodeStyle =======================

impl FromCssAttrs for NodeStyle {
    fn from_css_attrs(style_str: &str) -> Option<Self> {
        let mut fill_color: Option<ColorF> = None;
        let mut fill_opacity: f32 = 1.0;
        let mut fill_rule = FillRule::NonZero;
        let mut stroke_color: Option<ColorF> = None;
        let mut stroke_opacity: f32 = 1.0;
        let mut stroke_width: f32 = 1.0;
        let mut has_stroke_width = false;

        for decl in style_str.split(';') {
            let decl = decl.trim();
            if decl.is_empty() {
                continue;
            }
            let parts: Vec<&str> = decl.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let prop = parts[0].trim();
            let val = parts[1].trim();

            match prop {
                "fill" => {
                    fill_color = color::parse_css_color(val);
                },
                "fill-opacity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        fill_opacity = v.clamp(0.0, 1.0);
                    }
                },
                "fill-rule" => {
                    fill_rule = if val == "evenodd" {
                        FillRule::EvenOdd
                    } else {
                        FillRule::NonZero
                    };
                },
                "stroke" => {
                    stroke_color = color::parse_css_color(val);
                },
                "stroke-width" => {
                    let v = val.trim_end_matches("px").trim();
                    if let Ok(w) = v.parse::<f32>() {
                        stroke_width = w.max(0.0);
                        has_stroke_width = true;
                    }
                },
                "stroke-opacity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        stroke_opacity = v.clamp(0.0, 1.0);
                    }
                },
                "opacity" => {
                    if let Ok(v) = val.parse::<f32>() {
                        fill_opacity *= v;
                        stroke_opacity *= v;
                    }
                },
                _ => {},
            }
        }

        Some(NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill: fill_color.map(|c| FillParams {
                color: Some(c),
                opacity: fill_opacity,
                fill_rule,
            }),
            stroke: stroke_color.map(|c| StrokeParams {
                color: Some(c),
                opacity: stroke_opacity,
                width: if has_stroke_width { stroke_width } else { 1.0 },
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                miter_limit: 4.0,
                dash_array: None,
                dash_offset: 0.0,
            }),
            render_hints: None,
            effects: None,
        })
    }
}

// ======================= Build for NodeStyle =======================

impl Build for NodeStyle {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        // Prefer Servo's computed style cascade; fall back to inline `style` attr.
        let mut style = match input.computed_values {
            Some(cv) => {
                Self::from_computed_values(cv).unwrap_or_default()
            },
            None => {
                match (input.get_attr)("style") {
                    Some(css) => Self::from_css_attrs(&css).unwrap_or_default(),
                    None => NodeStyle::default(),
                }
            },
        };

        // Transforms are constructed internally — the caller does not need
        // a separate transform-build call.
        style.transform = <Vec<TransformOp> as Build>::build(input).unwrap_or_default();

        Ok(style)
    }
}
