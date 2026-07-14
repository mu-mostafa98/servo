/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Phase 1: only [`Rectangle`]. More shapes added in later phases.

pub mod attr_parsers;
pub(crate) mod rectangle;

pub use self::rectangle::Rectangle;

/// Phase 1: only [`Rect`].
#[derive(Debug, Clone)]
pub enum Shape {
    Rect(Rectangle),
}
