/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node construction — converts Servo style values and SVG markup
//! into the SVG engine's native types.
//!
//! The central abstraction is the [`Build`] trait — a Factory Method that
//! constructs SVG domain objects from a common input bundle. The caller
//! passes a single [`SvgBuildInput`] and receives a fully-constructed value.
//!
//! # Architecture
//!
//! ```text
//! SvgRenderNode::build(input)
//!   ├── SvgTag::build(input)         → Container or Shape
//!   │     └── Shape::build(input)    → dispatches by element_name
//!   │           └── Rectangle::build, Circle::build, …
//!   └── NodeStyle::build(input)      → fill, stroke, transforms
//!         ├── FillParams::from_computed_values (internal helper)
//!         ├── StrokeParams::from_computed_values (internal helper)
//!         └── Vec<TransformOp>::build          (internal)
//! ```

use style::properties::ComputedValues;
use style::values::computed::svg::{SVGPaint, SVGPaintKind};
use style::color::ColorSpace;
use webrender_api::ColorF;

use crate::error::SvgResult;
use crate::render_tree::{Container, SvgRenderNode, SvgTag};
use crate::shapes::Shape;
use crate::style::*;
use crate::style::transform::TransformOp;
use crate::style::fill::FillParams;
use crate::style::stroke::StrokeParams;

// ======================= Build Trait =======================

/// Factory Method trait — every SVG type that can be constructed from DOM
/// attributes and/or computed style implements this.
///
/// Returns [`SvgResult`] so that construction failures carry a reason
/// (missing attribute, parse error, unimplemented feature).
pub trait Build: Sized {
    fn build(input: &SvgBuildInput) -> SvgResult<Self>;
}

// ======================= Build Input =======================

/// Bundle of all data sources needed to construct an SVG node.
///
/// The caller (typically [`replaced.rs`](crate::traversal)) constructs one
/// from the current DOM element and passes it by reference — each
/// [`Build`] impl reads only the fields it needs.
pub struct SvgBuildInput<'a> {
    /// Element tag name, e.g. `"rect"`, `"path"`, `"g"`.
    pub element_name: &'a str,
    /// Attribute accessor — given an attribute name, returns its string value.
    /// This is the *only* bridge between the SVG engine and the DOM.
    pub get_attr: &'a dyn Fn(&str) -> Option<String>,
    /// Servo computed style, if available. When `Some`, the engine uses
    /// the fully-resolved style cascade. When `None`, it falls back to
    /// parsing the inline `style` attribute via `get_attr("style")`.
    pub computed_values: Option<&'a ComputedValues>,
}

// ======================= SvgTag Construction =======================

impl Build for SvgTag {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        match input.element_name {
            "svg" => Ok(SvgTag::Container(Container::Svg)),
            "g" => Ok(SvgTag::Container(Container::Group)),
            _ => Shape::build(input).map(SvgTag::Shape),
        }
    }
}

// ======================= SvgRenderNode Construction =======================

impl Build for SvgRenderNode {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let tag = SvgTag::build(input)?;
        let style = NodeStyle::build(input)?;
        Ok(SvgRenderNode {
            id: None,          // caller sets this
            tag,
            style,
            children: vec![],  // caller populates via recursive walk
        })
    }
}

// ======================= NodeStyle Construction =======================

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
        // a separate `extract_transforms()` call.
        style.transform = <Vec<TransformOp> as Build>::build(input).unwrap_or_default();

        Ok(style)
    }
}

// ======================= FromComputedValues for NodeStyle (internal helper) =======================

impl FromComputedValues for NodeStyle {
    type Input = ComputedValues;

    fn from_computed_values(values: &ComputedValues) -> Option<Self> {
        Some(NodeStyle {
            opacity: 1.0,
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

// ======================= FromCssAttrs for NodeStyle (internal helper) =======================

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
            opacity: 1.0,
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

// ======================= Legacy Convenience Wrappers =======================

/// Parse an SVG element name and attribute accessor into a [`SvgTag`].
///
/// Container elements return [`SvgTag::Container`] directly.
/// Shape elements delegate to [`Shape::build`].
///
/// Prefer [`SvgTag::build`](Build::build) directly for new code.
pub fn extract_tag(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<SvgTag> {
    let input = SvgBuildInput {
        element_name: name,
        get_attr,
        computed_values: None,
    };
    SvgTag::build(&input).ok()
}

/// Convenience wrapper for external callers.
///
/// Prefer [`NodeStyle::build`](Build::build) directly for new code.
pub fn extract_node_style(computed_values: &ComputedValues) -> NodeStyle {
    NodeStyle::build(&SvgBuildInput {
        element_name: "",
        get_attr: &|_| None,
        computed_values: Some(computed_values),
    })
    .unwrap_or_default()
}

/// Parse a CSS `style` attribute string into a [`NodeStyle`].
///
/// Prefer [`NodeStyle::build`](Build::build) directly for new code.
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
        // FIXME: handle gradient/pattern paint servers
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
