/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod shapes;
pub mod style;
pub mod render_tree;
pub mod error;

pub mod builder;
mod traversal;
mod renderer;
mod tessellator;

pub use builder::{Build, SvgBuildInput};
pub use traversal::render_svg_tree;
