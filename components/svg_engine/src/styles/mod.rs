/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module defines style-related enums and structs based on the SVG 2 specification.
//! Each style category has its own file — [`fill`] for fill properties, [`stroke`] for
//! stroke properties, and this module for the top-level [`NodeStyle`] and shared traits.

pub(crate) mod fill;
pub(crate) mod stroke;

pub use self::fill::{FillParams, FillRule};
pub use self::stroke::{StrokeParams, LineCap, LineJoin};

// ----------------- Node Style ------------------

/// Combined fill + stroke styling for an SVG render node.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    // pub opacity: f32,
    // pub visibility: Visibility,
    // pub display: Display,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
    // pub render_hints: RenderHints,
    // pub effects: NodeEffects,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            fill: None,
            stroke: None,
        }
    }
}

// ----------------- Unused stub types (future SVG spec support) ------------------

#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

#[derive(Debug, Clone, Copy)]
pub enum Display {
    Inline,
    Block,
    None,
}

#[derive(Debug, Clone)]
pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
    pub text_rendering: Option<TextRendering>,
    pub image_rendering: Option<ImageRendering>,
    pub paint_order: Option<PaintOrder>,
}

#[derive(Debug, Clone, Copy)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}

#[derive(Debug, Clone, Copy)]
pub enum ColorRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

#[derive(Debug, Clone, Copy)]
pub enum ColorInterpolation {
    Auto,
    SRGB,
    LinearRGB,
}

#[derive(Debug, Clone, Copy)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

#[derive(Debug, Clone, Copy)]
pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

#[derive(Debug, Clone, Copy)]
pub enum ImageRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

#[derive(Debug, Clone, Copy)]
pub enum PaintOrder {
    Normal,
}

#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub transform: Option<euclid::Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}

// ----------------------- FromComputedValues Trait -----------------------

/// Construct a style value from Servo's [`ComputedValues`](style::properties::ComputedValues).
///
/// Every SVG style type implements this trait so that construction from
/// the style system is uniform and dispatchable.
pub trait FromComputedValues: Sized {
    type Input;
    fn from_computed_values(values: &Self::Input) -> Option<Self>;
}

// ----------------------- FromCssAttrs Trait -----------------------

/// Parse a style value from a CSS `style` attribute string
/// (e.g. `"fill:red;stroke:blue;stroke-width:2"`).
///
/// Used as a fallback when `ComputedValues` aren't available
/// (e.g. for SVG child elements inside `<g>`).
pub trait FromCssAttrs: Sized {
    fn from_css_attrs(style_str: &str) -> Option<Self>;
}
