/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Property Reference: https://www.w3.org/TR/SVG2/propidx.html
//!
//! This module defines style-related enums and structs based on the SVG 2 specification.
//! Each style category has its own file — [`fill`] for fill properties, [`stroke`] for
//! stroke properties, [`hints`] for rendering hints, [`effects`] for node effects,
//! [`visibility`] for SVG visibility/display, [`node_style`] for the combined node style,
//! [`transform_ops`] for SVG transform operations and parsing,
//! WebRender integration lives in [`crate::renderer::transform`],
//! and [`color`] for color parsing.

pub(crate) mod fill;
pub(crate) mod stroke;
pub mod gradient;
pub(crate) mod hints;
pub(crate) mod effects;
pub(crate) mod visibility;
pub(crate) mod node_style;
pub(crate) mod color;
pub(crate) mod transform_ops;

pub use self::fill::{FillParams, FillRule};
pub use self::stroke::{StrokeParams, LineCap, LineJoin};
pub use self::hints::{
    RenderHints, VectorEffect, ColorRendering, ColorInterpolation,
    ShapeRendering, TextRendering, ImageRendering, PaintOrder,
};
pub use self::visibility::{Visibility, Display};
pub use self::effects::NodeEffects;
pub use self::node_style::NodeStyle;

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
