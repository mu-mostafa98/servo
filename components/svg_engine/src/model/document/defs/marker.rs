/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<marker>` definitions — sizing units, orientation, and geometry.

use crate::model::document::viewport::ViewBox;
use crate::model::element::SvgNode;

/// Coordinate system for marker sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerUnits {
    /// Marker size scales with the referencing shape's stroke width (default).
    StrokeWidth,
    /// Marker size is fixed in the current user coordinate system.
    UserSpaceOnUse,
}

/// Orientation of a marker relative to the path.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerOrient {
    /// Rotate to align with the path tangent.
    Auto,
    /// Like `Auto`, but the marker at the path *start* is flipped 180°.
    AutoStartReverse,
    /// Fixed rotation angle in degrees.
    Angle(f32),
}

impl Default for MarkerOrient {
    fn default() -> Self {
        // SVG 2: the initial value of `orient` is `0` (a fixed angle), not `auto`.
        MarkerOrient::Angle(0.0)
    }
}

/// A marker definition collected from `<marker>`.
#[derive(Debug)]
pub struct MarkerDef {
    /// Marker content, stored as a nested node subtree (see
    /// [`crate::model::document::ClipPathDef::root`]).
    pub root: SvgNode,
    /// Optional `viewBox` establishing the marker's coordinate system.
    pub view_box: Option<ViewBox>,
    /// Reference point (in viewBox coords) aligned with the path vertex.
    pub ref_x: f32,
    pub ref_y: f32,
    /// Rendered marker viewport width (scaled per `marker_units`).
    pub marker_width: f32,
    pub marker_height: f32,
    pub marker_units: MarkerUnits,
    pub orient: MarkerOrient,
}
