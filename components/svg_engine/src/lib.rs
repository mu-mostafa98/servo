/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software SVG render engine for Servo.
//!
//! Converts an [`SvgRenderTree`] (built from DOM in `layout::svg_builder`) into
//! WebRender display list commands via [`render_svg_tree`].
//!
//! # Architecture
//!
//! | Module | Role |
//! |--------|------|
//! | [`shapes`] | Pure data structs for SVG geometric shapes (rect, circle, etc.) |
//! | [`style`] | SVG property data types (fill, stroke, gradient, transform, …) |
//! | [`render_tree`] | [`SvgRenderTree`] node tree and definition types |
//! | [`error`] | Error types for SVG parsing failures |
//! | [`traversal`] | Recursive tree walk that produces the display list |
//! | [`renderer`] | Per-shape [`Render`] trait impls + fill/stroke/gradient pipelines |
//! | [`tessellator`] | Polygon triangulation + scanline rasterization |
//! | [`effects`] | Clip-path, mask, and filter resolution |
//!
//! The entry point is [`render_svg_tree`], called from
//! `layout::display_list::mod.rs`.  Shape construction happens in
//! `layout::svg_builder.rs`.

pub mod attr_parsers;
pub mod error;
pub mod image;
pub mod render_tree;
pub mod shapes;
pub mod style;
pub mod text;
pub mod visitor;

mod effects;
mod renderer;
mod tessellator;
mod traversal;

pub use render_tree::SvgTag;
pub use renderer::gradient::color_at_t_with_space;
pub use traversal::render_svg_tree;

pub use self::image::SvgImage;
pub use self::text::{ShapedGlyph, TextAnchor, TextSpan};
