/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Leaf SVG parsing primitives.
//!
//! Each module here parses one category of input — attributes, geometry, text,
//! paint, inline CSS, viewport, transforms — with no dependency on the higher
//! [`super::builder`] / [`super::defines`] layers. This is the "primitives"
//! tier of the layering, mirroring the `usvg` integration's `primitives/`.

pub(crate) mod attrs;
pub(crate) mod css;
pub(crate) mod geometry;
pub(crate) mod paint;
pub(crate) mod text;
pub(crate) mod transforms;
pub(crate) mod viewport;
