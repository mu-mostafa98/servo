/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<mask>` definitions, their `mask-type`, and `maskContentUnits`.

use crate::model::element::SvgNode;

/// The `mask-type` of a `<mask>`: whether the mask value is derived from the
/// mask content's luminance or its alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    /// `mask-type="luminance"` (the SVG default): mask value = luminance of the
    /// mask content's composited color.
    Luminance,
    /// `mask-type="alpha"`: mask value = alpha channel of the mask content.
    Alpha,
}

/// Coordinate system for `<mask>` content (`maskContentUnits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A mask definition collected from `<mask>`.
#[derive(Debug)]
pub struct MaskDef {
    /// The mask content, stored as a nested node subtree (see
    /// [`crate::model::document::ClipPathDef::root`]).
    pub root: SvgNode,
    /// Whether the mask value is luminance- or alpha-derived (`mask-type`).
    pub mask_type: MaskType,
    /// Coordinate system for the mask content (`maskContentUnits`).
    pub content_units: MaskContentUnits,
}
