/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module defines style-related enums and structs based on the SVG 2 specification.
//! Each style category has its own file — [`fill`] for fill properties, [`stroke`] for
//! stroke properties, [`hints`] for rendering hints, [`node_effects`] for node effects,
//! [`visibility`] for SVG visibility/display, [`transform_ops`] for SVG transform
//! operations, and [`color`] for color parsing.
//!
//! Style construction (FromComputedValues, FromCssAttrs) lives in
//! [`crate::layout::svg_builder`].

pub mod color;
pub(crate) mod fill;
pub mod gradient;
pub(crate) mod hints;
pub(crate) mod node_effects;
pub(crate) mod stroke;
pub mod transform_ops;
pub(crate) mod visibility;

pub use self::fill::{FillParams, FillRule};
pub use self::hints::{
    ColorInterpolation, ColorRendering, PaintOrder, RenderHints, ShapeRendering, VectorEffect,
};
pub use self::node_effects::NodeEffects;
pub use self::stroke::{LineCap, LineJoin, StrokeParams};
pub use self::visibility::{Display, Visibility};

/// Marker references attached to a shape (`marker-start`, `marker-mid`,
/// `marker-end`), each holding the referenced `id` (without the `#` prefix).
#[derive(Debug, Clone, Default)]
pub struct MarkerRefs {
    pub start: Option<String>,
    pub mid: Option<String>,
    pub end: Option<String>,
}

/// Combined fill + stroke styling for an SVG render node.
///
/// Layout-affecting properties (transforms) live on [`SvgRenderNode`],
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
    pub opacity: f32,
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
            opacity: 1.0,
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
