/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module defines style-related enums and structs based on the SVG 2 specification.

use webrender_api::ColorF;

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

// ----------------- Fill ------------------

/// SVG fill properties.
#[derive(Debug, Clone, Copy)]
pub struct FillParams {
    pub color: Option<ColorF>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

/// SVG fill rule: determines how overlapping regions are filled.
#[derive(Debug, Clone, Copy)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

// ----------------- Stroke ------------------

/// SVG stroke properties.
#[derive(Debug, Clone)]
pub struct StrokeParams {
    pub color: Option<ColorF>,
    pub opacity: f32,
    pub width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Option<Vec<f32>>,
    pub dash_offset: f32,
}

/// SVG line cap style — how the ends of open paths are rendered.
#[derive(Debug, Clone, Copy)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style — how corners are rendered in a polyline/polygon.
#[derive(Debug, Clone, Copy)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

// ----------------- Render Hints ------------------

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

/// Rendering order for fill, stroke, and markers.
#[derive(Debug, Clone, Copy)]
pub enum PaintOrder {
    Normal,
}

// ----------------- Node Effects ------------------

#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub transform: Option<euclid::Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}
