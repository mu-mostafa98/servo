/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering half of the engine.
//!
//! Consumes the [`crate::model`] tree and emits WebRender display-list
//! commands. Kept private — the public entry point is [`crate::render_svg_tree`].

pub(crate) mod effects;
pub(crate) mod renderer;
pub(crate) mod tessellator;
pub(crate) mod traversal;
