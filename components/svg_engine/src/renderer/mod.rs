/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape renderers — each shape logs its geometry via `eprintln!`.
//!
//! In future PRs, each shape will emit WebRender display list commands
//! and fill/stroke/gradient/pattern/transform modules will be restored.

pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod image;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod rect;
pub(crate) mod render_trait;
pub(crate) mod text;

// TODO: fill, gradient, helpers, pattern, providers, stroke, transform

pub(crate) use render_trait::Render;
