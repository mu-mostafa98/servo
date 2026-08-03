/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software SVG render engine for Servo.
//!
//! Converts a [`usvg::Tree`] (built from DOM in `layout::svg_builder`) into
//! WebRender display list commands via [`render_svg_tree`].
//!
//! # Architecture
//!
//! | Module | Role |
//! |--------|------|
//! | [`emitter`] | Shape emitters — convert usvg types into backend-agnostic [`PaintCommand`]s |
//! | [`renderer`] | Renderer + Backend trait — dispatch commands to WebRender / Krilla / etc. |
//! | [`traversal`] | Recursive tree walk — visits usvg nodes, calls emitters, feeds renderer |

pub(crate) mod emitter;
pub mod renderer;
mod traversal;

/// Rasterized image produced by the path emitter (Vello CPU output).
/// Carries raw RGBA pixel data for upload to the compositor.
pub struct RasterizedImage {
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub use traversal::{render_svg_tree, render_svg_tree_to};
