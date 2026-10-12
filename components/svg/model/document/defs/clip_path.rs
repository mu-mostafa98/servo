/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<clipPath>` definitions and their coordinate-system units.

use crate::model::element::SvgNode;

/// A clip path definition collected from `<clipPath>`.
#[derive(Debug)]
pub struct ClipPathDef {
    /// The clipping region, stored as a nested node subtree so that `<g>`,
    /// `<use>`, `<text>` and nested-`<defs>` children are not silently dropped.
    pub root: SvgNode,
    /// Coordinate system for the clip path.
    pub clip_path_units: ClipPathUnits,
}

/// Coordinate system for clip path contents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipPathUnits {
    /// Coordinates are relative to the object's bounding box (0..1 range).
    ObjectBoundingBox,
    /// Coordinates are in the current user coordinate system.
    UserSpaceOnUse,
}
