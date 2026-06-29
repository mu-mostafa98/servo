/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG attribute extraction — converts Servo style values and SVG markup
//! into the SVG engine's native types.
//!
//! This module is the dispatch hub:
//! - Shape construction uses [`FromAttributes`] via [`extract_tag`]
//! - Style construction uses [`FromComputedValues`] / [`FromCssAttrs`]
//! - Transform/viewBox parsing delegates to their respective data modules.

use style::properties::ComputedValues;
use style::values::computed::svg::{SVGPaint, SVGPaintKind};
use style::color::ColorSpace;
use webrender_api::ColorF;

use crate::render_tree::{Container, SvgTag};
use crate::shapes::{FromAttributes, Shape};
use crate::styles::*;

// ======================= Tag Dispatch =======================

/// Parse an SVG element name and attribute accessor into a [`SvgTag`].
///
/// Container elements return [`SvgTag::Container`] directly.
/// Shape elements delegate to [`Shape::from_attributes`].
pub fn extract_tag(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<SvgTag> {
    match name {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        _ => Shape::from_attributes(name, get_attr).map(SvgTag::Shape),
    }
}

// ======================= FromComputedValues impls =======================

impl FromComputedValues for NodeStyle {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        Some(NodeStyle {
            fill: FillParams::from_computed_values(values),
            stroke: StrokeParams::from_computed_values(values),
        })
    }
}

/// Convenience wrapper for external callers.
pub fn extract_node_style(computed_values: &ComputedValues) -> NodeStyle {
    NodeStyle::from_computed_values(computed_values).unwrap_or_default()
}

// ======================= FromCssAttrs impls =======================

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
                    fill_color = parse_css_color(val);
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
                    stroke_color = parse_css_color(val);
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
            fill: fill_color.map(|c| FillParams {
                color: Some(c),
                opacity: fill_opacity,
                fill_rule,
            }),
            stroke: stroke_color.map(|c| StrokeParams {
                color: Some(c),
                opacity: stroke_opacity,
                width: if has_stroke_width {
                    stroke_width
                } else {
                    1.0
                },
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                miter_limit: 4.0,
                dash_array: None,
                dash_offset: 0.0,
            }),
        })
    }
}

/// Convenience wrapper for external callers.
pub fn extract_node_style_from_css(style_str: &str) -> NodeStyle {
    NodeStyle::from_css_attrs(style_str).unwrap_or_default()
}

// ======================= Internal Helpers =======================

/// Resolve an [`SVGPaint`] to a concrete [`ColorF`], handling `currentColor`
/// and color space conversion.
pub(crate) fn resolve_svg_paint(
    svg_paint: &SVGPaint,
    computed_values: &ComputedValues,
) -> Option<ColorF> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
            ))
        },
        SVGPaintKind::None => None,
        _ => None,
    }
}

/// Parse a CSS color value (hex `#rrggbb` or named color).
fn parse_css_color(val: &str) -> Option<ColorF> {
    let val = val.trim();
    if val == "none" || val == "transparent" {
        return None;
    }
    if val.starts_with('#') {
        let hex = &val[1..];
        // #rgb → expand each digit
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(ColorF::new(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                1.0,
            ));
        }
        // #rrggbb
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                return Some(ColorF::new(
                    ((rgb >> 16) & 0xFF) as f32 / 255.0,
                    ((rgb >> 8) & 0xFF) as f32 / 255.0,
                    (rgb & 0xFF) as f32 / 255.0,
                    1.0,
                ));
            }
        }
    }
    match val {
        "red" => Some(ColorF::new(1.0, 0.0, 0.0, 1.0)),
        "green" => Some(ColorF::new(0.0, 0.502, 0.0, 1.0)),
        "blue" => Some(ColorF::new(0.0, 0.0, 1.0, 1.0)),
        "white" => Some(ColorF::new(1.0, 1.0, 1.0, 1.0)),
        "black" => Some(ColorF::new(0.0, 0.0, 0.0, 1.0)),
        "yellow" => Some(ColorF::new(1.0, 1.0, 0.0, 1.0)),
        "orange" => Some(ColorF::new(1.0, 0.647, 0.0, 1.0)),
        "purple" => Some(ColorF::new(0.502, 0.0, 0.502, 1.0)),
        "gray" | "grey" => Some(ColorF::new(0.5, 0.5, 0.5, 1.0)),
        _ => None,
    }
}
