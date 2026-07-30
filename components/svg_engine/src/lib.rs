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
//! | [`renderer`] | Per-shape rendering via a `Render` trait + fill/stroke pipelines |
//! | [`traversal`] | Recursive tree walk that produces the display list |
//!
//! The entry point is [`render_svg_tree`], called from
//! `layout::display_list::mod.rs`.

mod renderer;
mod traversal;

pub use traversal::render_svg_tree;
