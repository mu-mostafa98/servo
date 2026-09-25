/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG properties — style and presentation attributes.
//!
//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module holds everything that can appear as a style or presentation
//! attribute: fill/stroke ([`paint`]), gradients ([`gradient`]), transforms
//! ([`transform`]), node effects ([`effects`]), plus rendering hints and the
//! combined [`NodeStyle`]. Style construction (FromComputedValues,
//! FromCssAttrs) lives in `components/layout/svg`.

pub mod effects;
pub mod gradient;
pub mod paint;
pub mod transform;

pub use self::effects::NodeEffects;
pub use self::paint::{FillParams, FillRule, LineCap, LineJoin, StrokeParams};

use crate::model::document::{DefRef, MarkerDef};
use crate::model::units::Opacity;

// ======================= Visibility & Display =======================

/// Element visibility.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Visible,
    Hidden,
}

/// Element display type.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Display {
    Inline,
    Block,
    None,
}

// ======================= Rendering Hints =======================

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

// ======================= Node Style =======================

/// Marker references attached to a shape (`marker-start`, `marker-mid`,
/// `marker-end`), each holding a [`DefRef`] to a [`MarkerDef`] — a raw `#id`
/// during tree building, a typed `Arc` handle after the resolve pass.
#[derive(Debug, Clone, Default)]
pub struct MarkerRefs {
    pub start: Option<DefRef<MarkerDef>>,
    pub mid: Option<DefRef<MarkerDef>>,
    pub end: Option<DefRef<MarkerDef>>,
}

/// Combined fill + stroke styling for an SVG render node.
///
/// Layout-affecting properties (transforms) live on [`SvgNode`],
/// not here — this struct only holds paint-level styling.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub visibility: Visibility,
    pub display: Display,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
    pub render_hints: Option<RenderHints>,
    pub effects: Option<NodeEffects>,
    /// Element-level opacity (the CSS `opacity` property).
    /// Applied as a multiplier on top of fill-/stroke-opacity.
    pub opacity: Opacity,
    /// Marker references (start/mid/end).
    pub markers: Option<MarkerRefs>,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            fill: None,
            stroke: None,
            render_hints: None,
            effects: None,
            opacity: Opacity::ONE,
            markers: None,
        }
    }
}

// ======================= Convenience Methods =======================

impl NodeStyle {
    /// Whether the element is visible (per the SVG `visibility` property).
    pub fn is_visible(&self) -> bool {
        matches!(self.visibility, Visibility::Visible)
    }

    /// Whether the element is displayed (per the SVG `display` property).
    /// Returns `false` for `display: none`.
    pub fn is_displayed(&self) -> bool {
        !matches!(self.display, Display::None)
    }
}
