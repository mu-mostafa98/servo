/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<pattern>` definitions — tile geometry, units, and transform.

use crate::model::document::viewport::{AspectRatio, ViewBox};
use crate::model::element::SvgNode;
use crate::model::transform::TransformOp;

/// Coordinate system for pattern tile sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// Coordinate system for pattern content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A length or percentage value for pattern geometry (`x`/`y`/`width`/`height`).
///
/// The interpretation depends on `patternUnits` (§13.3.1):
/// - `objectBoundingBox` (the default): both forms are a *fraction* of the host
///   shape's bounding box, so `Number(0.5)` and `Percentage(50.0)` are equivalent.
/// - `userSpaceOnUse`: a `Number` is an absolute user-space length and a
///   `Percentage` is `p/100` of the reference dimension (viewport width/height).
///
/// The percentage/unit is preserved here (rather than baked to `f32` at parse
/// time) because the reference box is only known at render time.
#[derive(Debug, Clone, Copy)]
pub enum PatternLength {
    Number(f32),
    Percentage(f32),
}

impl PatternLength {
    /// Resolve in `objectBoundingBox` space: both `Number` and `Percentage`
    /// become a fraction of the bounding box (0..1).
    pub fn to_object_bbox(self) -> f32 {
        match self {
            PatternLength::Number(v) => v,
            PatternLength::Percentage(p) => p / 100.0,
        }
    }

    /// Resolve in `userSpaceOnUse` space: a `Number` is an absolute length, a
    /// `Percentage` is `p/100` of `axis_len`.
    pub fn to_user_space(self, axis_len: f32) -> f32 {
        match self {
            PatternLength::Number(v) => v,
            PatternLength::Percentage(p) => p / 100.0 * axis_len,
        }
    }

    /// Whether this length resolves to a non-positive value in either space
    /// (a zero or negative tile size means "no pattern", §13.3.1).
    pub fn is_non_positive(self) -> bool {
        match self {
            PatternLength::Number(v) => v <= 0.0,
            PatternLength::Percentage(p) => p <= 0.0,
        }
    }
}

/// A pattern definition collected from `<pattern>`.
#[derive(Debug)]
pub struct PatternDef {
    pub width: PatternLength,
    pub height: PatternLength,
    pub x: PatternLength,
    pub y: PatternLength,
    pub pattern_units: PatternUnits,
    pub pattern_content_units: PatternContentUnits,
    /// The `patternTransform` attribute, applied to the tile coordinate system.
    pub transform: Vec<TransformOp>,
    /// Optional `viewBox` on the pattern, mapped into the tile via
    /// `preserveAspectRatio`.
    pub view_box: Option<ViewBox>,
    pub aspect_ratio: Option<AspectRatio>,
    /// The pattern tile content, stored as a nested node subtree (see
    /// [`crate::model::document::ClipPathDef::root`]).
    pub root: SvgNode,
}
