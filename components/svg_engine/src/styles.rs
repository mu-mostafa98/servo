/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
// This module defines style-related enums and structs based on the SVG 2 specification.

use webrender_api::ColorF;
use euclid::Transform2D;

// ----------------- Node Style ------------------

pub struct NodeStyle {
    // pub opacity: f32,
    // pub visibility: Visibility,
    // pub display: Display,
    pub fill: Option<FillParams>,
    // pub stroke: Option<StrokeParams>,
    // pub render_hints: RenderHints,
    // pub effects: NodeEffects,
}

pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

pub enum Display {
    Inline,
    Block,
    None,
}

// ----------------- Fill ------------------
pub struct  FillParams{
    pub color: Option<ColorF>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}


pub enum FillRule {
    NonZero,
    EvenOdd,
}

// ----------------- Stroke ------------------

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

pub enum LineCap {
    Butt,
    Round,
    Square,
}

pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

// ----------------- Render Hints ------------------


pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
    pub text_rendering: Option<TextRendering>,
    pub image_rendering: Option<ImageRendering>,
    pub paint_order: Option<PaintOrder>,
}

pub enum VectorEffect {
    None,
    NonScalingStroke,
}


pub enum ColorRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

pub enum ColorInterpolation {
    Auto,
    SRGB,
    LinearRGB,
}

pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

pub enum ImageRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

pub enum PaintOrder {
    Normal, // Default: fill, stroke, markers
}

// ----------------- Node Effects ------------------
pub struct NodeEffects {
    pub transform: Option<Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}