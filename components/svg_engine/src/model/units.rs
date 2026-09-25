/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Strict scalar newtypes that turn unit and id-vs-url mix-ups into
//! compile-time errors instead of silent runtime bugs.
//!
//! * [`Opacity`] is a unitless `[0, 1]` value, distinct from a length — so an
//!   opacity can never be passed where a length is expected, and vice versa.
//! * [`Id`] is an element id (a fragment identifier without the leading `#`),
//!   distinct from an arbitrary string or URL.
//! * [`Length`] is a user-space length scalar, distinct from an opacity, ratio,
//!   angle, or id — so a length can't be passed where one of those is expected.

use std::fmt;

/// An opacity value, clamped to the valid `[0, 1]` range on construction.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Opacity(f32);

impl Opacity {
    /// Fully opaque.
    pub const ONE: Opacity = Opacity(1.0);

    /// Construct an opacity, clamping out-of-range values to `[0, 1]`.
    pub fn new(value: f32) -> Opacity {
        Opacity(value.clamp(0.0, 1.0))
    }

    /// The raw `f32` value in `[0, 1]`.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for Opacity {
    fn default() -> Self {
        Opacity::ONE
    }
}

/// An element id (a fragment identifier without the leading `#`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(String);

impl Id {
    /// Construct an id from any string-like value.
    pub fn new(id: impl Into<String>) -> Id {
        Id(id.into())
    }

    /// The raw id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A length in the current user coordinate system (a `f32` scalar, but typed so
/// it can't be confused with an opacity, ratio, angle, or id).
///
/// Units are **erased** at this boundary: by the time a [`Length`] is built,
/// its raw value is already resolved to user space — `px`, `em`, `ex`, `%`,
/// etc. have been resolved against the appropriate viewport/bbox by the layout
/// layer. Consequently, relative lengths that depend on a containing context
/// (percentages, font-relative units) must be resolved *before* reaching this
/// type; the render engine cannot recover the original unit.
///
/// Note: SVG lengths may be negative (e.g. `x`/`y` coordinates), so unlike
/// [`Opacity`] this newtype does not clamp.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Length(f32);

impl Length {
    /// Zero length.
    pub const ZERO: Length = Length(0.0);

    /// Construct a length from a raw value.
    pub fn new(value: f32) -> Length {
        Length(value)
    }

    /// The raw `f32` value.
    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for Length {
    fn default() -> Self {
        Length::ZERO
    }
}
