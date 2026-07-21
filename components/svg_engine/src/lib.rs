/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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
pub use self::text::TextSpan;
