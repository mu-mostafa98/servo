/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape renderers — convert usvg types into WebRender display list commands.
//!
//! Each simple shape implements the [`Render`] trait. The traversal module
//! dispatches to the appropriate renderer based on node type.

pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod rect;
mod render_trait;

pub(crate) use render_trait::{Render, RenderContext};
