/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod shapes;
mod styles;
pub mod transform;
pub mod render_tree;

pub mod extract;
pub mod render;
mod renderers;

pub use extract::extract_node_style;
pub use render::render_svg_tree; // render_svg_tree(tree, origin, size, spatial_id, clip_chain_id, wr)
