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

pub mod emitter;
pub mod renderer;
mod traversal;

pub use traversal::render_svg_tree;
