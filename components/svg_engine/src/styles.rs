/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
// This module defines style-related enums and structs based on the SVG 2 specification.

use webrender_api::ColorF;
use euclid::Transform2D;

// ----------------- Node Style ------------------

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

#[derive(Debug)]
pub enum Display {
    Inline,
    Block,
    None,
}

// ----------------- Fill ------------------
#[derive(Debug)]
pub struct  FillParams{
    pub color: Option<ColorF>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

#[derive(Debug, Clone, Copy)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

// ----------------- Stroke ------------------

#[derive(Debug)]
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

#[derive(Debug, Clone, Copy)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

// ----------------- Render Hints ------------------


#[derive(Debug)]
pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
    pub text_rendering: Option<TextRendering>,
    pub image_rendering: Option<ImageRendering>,
    pub paint_order: Option<PaintOrder>,
}

#[derive(Debug)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}


#[derive(Debug)]
pub enum ColorRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

#[derive(Debug)]
pub enum ColorInterpolation {
    Auto,
    SRGB,
    LinearRGB,
}

#[derive(Debug)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

#[derive(Debug)]
pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

#[derive(Debug)]
pub enum ImageRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

#[derive(Debug)]
pub enum PaintOrder {
    Normal, // Default: fill, stroke, markers
}

// ----------------- Node Effects ------------------
#[derive(Debug)]
pub struct NodeEffects {
    pub transform: Option<Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}