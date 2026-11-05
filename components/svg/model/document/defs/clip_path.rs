/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<clipPath>` definitions and their coordinate-system units.

use crate::model::element::SvgNode;
use crate::model::style::FillRule;
use malloc_size_of_derive::MallocSizeOf;

use super::DefRef;

/// A clip path definition collected from `<clipPath>`.
#[derive(Debug, MallocSizeOf)]
pub struct ClipPathDef {
    /// The clipping region, stored as a nested node subtree so that `<g>`,
    /// `<use>`, `<text>` and nested-`<defs>` children are not silently dropped.
    pub root: SvgNode,
    /// Coordinate system for the clip path.
    pub clip_path_units: ClipPathUnits,
    /// A `clip-path` on the `<clipPath>` element itself: the referenced clip is
    /// applied to this definition's content (nested clipping), so the effective
    /// clip is the intersection of the two.
    pub clip_path: Option<DefRef<ClipPathDef>>,
    /// `clip-rule` on the `<clipPath>` element. `clip-rule` is an inherited
    /// property, so this is the default rule for shapes that do not set their
    /// own `clip-rule` (`nonzero` unless the element overrides it).
    pub clip_rule: FillRule,
}

/// Coordinate system for clip path contents.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum ClipPathUnits {
    /// Coordinates are relative to the object's bounding box (0..1 range).
    ObjectBoundingBox,
    /// Coordinates are in the current user coordinate system.
    UserSpaceOnUse,
}
