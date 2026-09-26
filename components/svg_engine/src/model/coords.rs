/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG coordinate systems, transformations, and units.
//!
//! Coordinate Systems spec: <https://www.w3.org/TR/SVG2/coords.html>
//!
//! Chapter-facing home for the coordinate-system types, re-exported from their
//! canonical locations:
//! - viewport / `viewBox` / `preserveAspectRatio` → [`crate::model::document::viewport`]
//! - transformations → [`crate::model::style::transform`]
//! - lengths → [`crate::model::units`]
//! - points / paths → [`crate::model::geometry`]

pub use crate::model::document::viewport::{
    AspectAlign, AspectRatio, MeetOrSlice, SvgViewport, ViewBox, ViewportInfo,
};
pub use crate::model::geometry::{PathCommand, PathData, Point};
pub use crate::model::style::transform::TransformOp;
pub use crate::model::units::Length;
