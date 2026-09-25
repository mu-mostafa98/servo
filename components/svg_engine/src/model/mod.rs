/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pure SVG data model.
//!
//! This half of the engine has **no** dependency on WebRender, vello, or the
//! [`crate::render`] half — it only describes *what* an SVG is (shapes, style,
//! tree structure, units), not *how* it is drawn.

pub mod error;
pub mod geometry;
pub mod image;
pub mod resource;
pub mod shapes;
pub mod style;
pub mod text;
pub mod tree;
pub mod units;
