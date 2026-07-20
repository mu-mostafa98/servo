/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render hints — quality-over-speed tradeoffs for rendering.

/// Rendering hints for SVG elements.
#[derive(Debug, Clone)]
pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
    pub paint_order: Option<PaintOrder>,
    // --- Spec stubs (blocked on new features) ---
    #[allow(dead_code)]
    pub text_rendering: Option<TextRendering>,
    #[allow(dead_code)]
    pub image_rendering: Option<ImageRendering>,
}

/// Controls how strokes scale under transforms.
#[derive(Debug, Clone, Copy)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}

/// Color rendering quality hint.
#[derive(Debug, Clone, Copy)]
pub enum ColorRendering {
    Auto,
    OptimizeSpeed,
    OptimizeQuality,
}

/// Color interpolation method.
#[derive(Debug, Clone, Copy)]
pub enum ColorInterpolation {
    Auto,
    Srgb,
    LinearRGB,
}

/// Shape rendering quality hint.
#[derive(Debug, Clone, Copy)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

/// Fill/stroke/marker rendering order.
///
/// Per SVG 2 §5.10, `paint-order` controls the stacking order of fill, stroke,
/// and markers.  The default (Normal) draws fill → stroke → markers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintOrder {
    /// Default: fill first, then stroke.
    Normal,
    /// Stroke then fill.
    StrokeFill,
    /// Fill then stroke (same as Normal, but explicit).
    FillStroke,
}

impl PaintOrder {
    /// Whether stroke should be drawn before fill.
    pub fn stroke_before_fill(&self) -> bool {
        matches!(self, PaintOrder::StrokeFill)
    }
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
