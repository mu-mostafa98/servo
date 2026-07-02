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
use super::gradient::PaintServer;
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
        let mut fill_paint_server: Option<PaintServer> = None;
        let mut fill_opacity: f32 = 1.0;
        let mut fill_rule = FillRule::NonZero;
        let mut stroke_color: Option<ColorF> = None;
        let mut stroke_paint_server: Option<PaintServer> = None;
        let mut stroke_opacity: f32 = 1.0;
        let mut stroke_width: f32 = 1.0;
        let mut has_stroke_width = false;
        let mut line_cap = LineCap::Butt;
        let mut line_join = LineJoin::Miter;
        let mut miter_limit: f32 = 4.0;
        let mut dash_array: Option<Vec<f32>> = None;
        let mut dash_offset: f32 = 0.0;

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
                    match PaintServer::from_attr(val) {
                        Some(PaintServer::Solid(c)) => fill_color = Some(c),
                        other @ Some(PaintServer::Gradient(_)) => fill_paint_server = other,
                        None => {},
                    }
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
                    match PaintServer::from_attr(val) {
                        Some(PaintServer::Solid(c)) => stroke_color = Some(c),
                        other @ Some(PaintServer::Gradient(_)) => stroke_paint_server = other,
                        None => {},
                    }
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
                "stroke-linecap" => {
                    line_cap = match val {
                        "round" => LineCap::Round,
                        "square" => LineCap::Square,
                        _ => LineCap::Butt,
                    };
                },
                "stroke-linejoin" => {
                    line_join = match val {
                        "round" => LineJoin::Round,
                        "bevel" => LineJoin::Bevel,
                        _ => LineJoin::Miter,
                    };
                },
                "stroke-miterlimit" => {
                    if let Ok(v) = val.parse::<f32>() {
                        miter_limit = v.max(1.0);
                    }
                },
                "stroke-dasharray" => {
                    if val == "none" {
                        dash_array = None;
                    } else {
                        let dashes: Vec<f32> = val
                            .split(|c: char| c == ',' || c.is_whitespace())
                            .filter(|s| !s.is_empty())
                            .filter_map(|s| s.trim().parse::<f32>().ok())
                            .collect();
                        if !dashes.is_empty() {
                            dash_array = Some(dashes);
                        }
                    }
                },
                "stroke-dashoffset" => {
                    let v = val.trim_end_matches("px").trim();
                    if let Ok(offset) = v.parse::<f32>() {
                        dash_offset = offset;
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

        let fill = match (fill_color, fill_paint_server) {
            (Some(c), _) => Some(FillParams {
                color: Some(c),
                paint_server: None,
                opacity: fill_opacity,
                fill_rule,
            }),
            (None, Some(ps)) => Some(FillParams {
                color: None,
                paint_server: Some(ps),
                opacity: fill_opacity,
                fill_rule,
            }),
            (None, None) => None,
        };
        let stroke = match (stroke_color, stroke_paint_server) {
            (Some(c), _) => Some(StrokeParams {
                color: Some(c),
                paint_server: None,
                opacity: stroke_opacity,
                width: if has_stroke_width { stroke_width } else { 1.0 },
                line_cap,
                line_join,
                miter_limit,
                dash_array,
                dash_offset,
            }),
            (None, Some(ps)) => Some(StrokeParams {
                color: None,
                paint_server: Some(ps),
                opacity: stroke_opacity,
                width: if has_stroke_width { stroke_width } else { 1.0 },
                line_cap,
                line_join,
                miter_limit,
                dash_array,
                dash_offset,
            }),
            (None, None) => None,
        };

        Some(NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill,
            stroke,
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

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_css_basic_fill_stroke() {
        let ns = NodeStyle::from_css_attrs("fill:red;stroke:blue;stroke-width:2").unwrap();
        assert!(ns.fill.is_some());
        assert!(ns.stroke.is_some());
        let fill = ns.fill.unwrap();
        assert!((fill.color.unwrap().r - 1.0).abs() < 0.01);
        let stroke = ns.stroke.unwrap();
        assert!((stroke.width - 2.0).abs() < 0.001);
    }

    #[test]
    fn from_css_stroke_linecap() {
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linecap:round").unwrap();
        assert_eq!(ns.stroke.unwrap().line_cap, LineCap::Round);
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linecap:square").unwrap();
        assert_eq!(ns.stroke.unwrap().line_cap, LineCap::Square);
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linecap:butt").unwrap();
        assert_eq!(ns.stroke.unwrap().line_cap, LineCap::Butt);
    }

    #[test]
    fn from_css_stroke_linejoin() {
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linejoin:round").unwrap();
        assert_eq!(ns.stroke.unwrap().line_join, LineJoin::Round);
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linejoin:bevel").unwrap();
        assert_eq!(ns.stroke.unwrap().line_join, LineJoin::Bevel);
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-linejoin:miter").unwrap();
        assert_eq!(ns.stroke.unwrap().line_join, LineJoin::Miter);
    }

    #[test]
    fn from_css_stroke_miterlimit() {
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-miterlimit:8").unwrap();
        let delta = (ns.stroke.unwrap().miter_limit - 8.0).abs();
        assert!(delta < 0.001);
    }

    #[test]
    fn from_css_stroke_dasharray() {
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-dasharray:5,10,15").unwrap();
        let dashes = ns.stroke.unwrap().dash_array.unwrap();
        assert_eq!(dashes, vec![5.0, 10.0, 15.0]);
    }

    #[test]
    fn from_css_stroke_dashoffset() {
        let ns = NodeStyle::from_css_attrs("stroke:black;stroke-dashoffset:3").unwrap();
        let delta = (ns.stroke.unwrap().dash_offset - 3.0).abs();
        assert!(delta < 0.001);
    }

    #[test]
    fn from_css_fill_rule() {
        let ns = NodeStyle::from_css_attrs("fill:red;fill-rule:evenodd").unwrap();
        assert_eq!(ns.fill.unwrap().fill_rule, FillRule::EvenOdd);
        let ns = NodeStyle::from_css_attrs("fill:red;fill-rule:nonzero").unwrap();
        assert_eq!(ns.fill.unwrap().fill_rule, FillRule::NonZero);
    }

    #[test]
    fn from_css_empty() {
        let ns = NodeStyle::from_css_attrs("").unwrap();
        assert!(ns.fill.is_none());
        assert!(ns.stroke.is_none());
    }

    #[test]
    fn from_css_opacity_affects_both() {
        let ns = NodeStyle::from_css_attrs("fill:red;stroke:blue;opacity:0.5").unwrap();
        let delta_fill = (ns.fill.unwrap().opacity - 0.5).abs();
        let delta_stroke = (ns.stroke.unwrap().opacity - 0.5).abs();
        assert!(delta_fill < 0.001);
        assert!(delta_stroke < 0.001);
    }
}
