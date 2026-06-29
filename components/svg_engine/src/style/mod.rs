/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module defines style-related enums and structs based on the SVG 2 specification.
//! Each style category has its own file — [`fill`] for fill properties, [`stroke`] for
//! stroke properties, [`hints`] for rendering hints, [`effects`] for node effects,
//! and [`transform`] for SVG transform operations.

pub(crate) mod fill;
pub(crate) mod stroke;
pub(crate) mod hints;
pub(crate) mod effects;
pub mod transform;

pub use self::fill::{FillParams, FillRule};
pub use self::stroke::{StrokeParams, LineCap, LineJoin};
#[allow(unused_imports)]
pub use self::hints::{
    RenderHints, VectorEffect, ColorRendering, ColorInterpolation,
    ShapeRendering, TextRendering, ImageRendering, PaintOrder,
};
#[allow(unused_imports)]
pub use self::effects::{Visibility, Display, NodeEffects};

use self::transform::TransformOp;

// ----------------- Node Style ------------------

/// Combined fill + stroke styling for an SVG render node.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub opacity: f32,
    pub visibility: Visibility,
    pub display: Display,
    pub transform: Vec<TransformOp>,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
    pub render_hints: Option<RenderHints>,
    pub effects: Option<NodeEffects>,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            opacity: 1.0,
            visibility: Visibility::Visible,
            display: Display::Inline,
            transform: Vec::new(),
            fill: None,
            stroke: None,
            render_hints: None,
            effects: None,
        }
    }
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
