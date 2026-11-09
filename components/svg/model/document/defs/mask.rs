/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<mask>` definitions, their `mask-type`, and their unit systems.

use crate::model::element::SvgNode;
use crate::model::document::RegionValue;
use malloc_size_of_derive::MallocSizeOf;

/// The `mask-type` of a `<mask>`: whether the mask value is derived from the
/// mask content's luminance or its alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, MallocSizeOf)]
pub enum MaskType {
    /// `mask-type="luminance"` (the SVG default): mask value = luminance of the
    /// mask content's composited color.
    Luminance,
    /// `mask-type="alpha"`: mask value = alpha channel of the mask content.
    Alpha,
}

/// Coordinate system for `<mask>` content (`maskContentUnits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, MallocSizeOf)]
pub enum MaskContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// Coordinate system for the `<mask>` element's own region attributes
/// (`x`/`y`/`width`/`height`), from `maskUnits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, MallocSizeOf)]
pub enum MaskUnits {
    /// Region values are in the current user coordinate system.
    UserSpaceOnUse,
    /// Region values are fractions/percentages of the referencing element's
    /// object bounding box (the default).
    ObjectBoundingBox,
}

/// A mask definition collected from `<mask>`.
#[derive(Debug, MallocSizeOf)]
pub struct MaskDef {
    /// The mask content, stored as a nested node subtree (see
    /// [`crate::model::document::ClipPathDef::root`]).
    pub root: SvgNode,
    /// Whether the mask value is luminance- or alpha-derived (`mask-type`).
    pub mask_type: MaskType,
    /// Coordinate system for the mask content (`maskContentUnits`).
    pub content_units: MaskContentUnits,
    /// Mask region left edge (`x`) — `<number-or-percentage>`, resolved against
    /// the referencing element's bounding box at render time. Default `-10%`.
    pub x: RegionValue,
    /// Mask region top edge (`y`). Default `-10%`.
    pub y: RegionValue,
    /// Mask region width. Default `120%`.
    pub width: RegionValue,
    /// Mask region height. Default `120%`.
    pub height: RegionValue,
    /// Coordinate system for the mask region (`maskUnits`).
    pub mask_units: MaskUnits,
}
