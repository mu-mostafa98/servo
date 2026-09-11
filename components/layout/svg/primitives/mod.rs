/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Leaf parsing/geometry/text primitives used to build a [`usvg::Tree`] from
//! Servo's SVG DOM. These functions depend only on Servo's style/layout types and
//! on usvg's public constructors — never on the higher `builder`/`effects` layers.

pub(crate) mod attrs;
pub(crate) mod geometry;
pub(crate) mod image;
pub(crate) mod marker;
pub(crate) mod paint;
pub(crate) mod shape;
pub(crate) mod text;
