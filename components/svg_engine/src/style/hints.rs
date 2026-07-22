/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone)]
pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
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
    Srgb,
    LinearRGB,
}

#[derive(Debug, Clone, Copy)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintOrder {
    Normal,
    StrokeFill,
    FillStroke,
}

impl PaintOrder {
    pub fn stroke_before_fill(&self) -> bool {
        matches!(self, PaintOrder::StrokeFill)
    }
}
