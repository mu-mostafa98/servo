/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software SVG render engine for Servo.
//!
//! Converts an [`SvgRenderTree`] (built from DOM in `layout::svg_builder`) into
//! rendered output via [`render_svg_tree`].
//!
//! # Architecture (PR #1)
//!
//! | Module | Role |
//! |--------|------|
//! | [`render_tree`] | [`SvgRenderTree`] node tree and structural types |
//! | [`error`] | Error types for SVG parsing failures |
//! | [`traversal`] | Recursive tree walk with enter/exit logging |
//!
//! TODO: implement shapes, text, image, attr_parsers, style, renderer,
//! effects, tessellator, visitor modules

pub mod attr_parsers;
pub mod error;
pub mod image;
pub mod render_tree;
pub mod shapes;
pub mod text;

mod renderer;
mod traversal;

pub use render_tree::SvgTag;
pub use traversal::render_svg_tree;

pub use self::image::SvgImage;
pub use self::text::{TextAnchor, TextSpan};
