/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render hints — quality-over-speed tradeoffs for rendering.
//!
//! These types are future SVG spec stubs and are not yet wired into
//! the rendering pipeline.

/// Rendering hints for SVG elements.
#[allow(dead_code)]
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

/// Controls how strokes scale under transforms.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}

/// Color rendering quality hint.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ColorRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

/// Color interpolation method.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ColorInterpolation {
    Auto,
    Srgb,
    LinearRGB,
}

/// Shape rendering quality hint.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

/// Text rendering quality hint.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

/// Image rendering quality hint.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ImageRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

/// Fill/stroke/marker rendering order.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum PaintOrder {
    Normal,
}
