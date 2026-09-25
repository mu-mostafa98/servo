/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pure SVG data model for Servo.
//!
//! This crate describes *what* an SVG document is — elements, style properties,
//! the document tree, and definition types — with **no** dependency on WebRender
//! or any rendering backend. It is the shared vocabulary consumed by the
//! rendering half (added in a later change).
//!
//! | Module | Role |
//! |--------|------|
//! | [`model::element`] | SVG element types (shapes, image, text, nodes) |
//! | [`model::style`] | SVG property data types (fill, stroke, gradient, transform, …) |
//! | [`model::document`] | `SvgTree` document, viewport, and definition types |
//! | [`model::error`] | Error types for SVG parsing failures |
//! | [`model::geometry`] | Geometry value types (`Point`, `PathData`) |
//! | [`model::resource`] | Opaque resource keys (round-trip to WebRender keys) |
//! | [`model::units`] | Unit newtypes (`Length`, `Opacity`, `Id`) |

pub mod model;

pub use model::document;
pub use model::element;
pub use model::error;
pub use model::geometry;
pub use model::resource;
pub use model::style;
pub use model::units;

pub use model::element::SvgTag;
pub use model::element::image::SvgImage;
pub use model::element::text::{DominantBaseline, ShapedGlyph, TextAnchor, TextSpan};
