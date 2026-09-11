/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint servers, clip paths, masks and filters.
//!
//! These resolvers turn Servo SVG DOM nodes into usvg's effect nodes. They sit
//! between the leaf [`crate::svg::primitives`] (parsing/geometry/text) and the
//! [`crate::svg::builder`] facades, and may call back into the builder for
//! recursive content (`<pattern>`/`<clipPath>`/`<mask>` children) — the cycle is
//! fine in Rust.

pub(crate) mod clip;
pub(crate) mod filter;
pub(crate) mod mask;
pub(crate) mod paint;
