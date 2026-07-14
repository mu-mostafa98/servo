/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software SVG render engine for Servo.
//!
//! Converts an [`SvgRenderTree`] into WebRender display list commands.
//!
//! # Phase 1
//!
//! Only supports `<rect>` with fill. More shapes and stroke/gradient/etc.
//! are added in later phases.

pub mod error;
pub mod render_tree;
pub mod shapes;
pub mod style;

mod renderer;
mod traversal;

pub use traversal::render_svg_tree;
