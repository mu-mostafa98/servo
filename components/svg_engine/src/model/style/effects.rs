/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node effects — clip-path, mask, and filter.

use crate::model::document::{ClipPathDef, DefRef, FilterDef, MaskDef};

/// SVG node effects — clip-path, mask, filter.
///
/// Each reference is a [`DefRef`]: a raw `#id` string during tree building,
/// rewritten to a typed `Arc` handle during the post-build resolve pass.
#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub clip_path: Option<DefRef<ClipPathDef>>,
    pub mask: Option<DefRef<MaskDef>>,
    /// Reference to a `<filter>` element (e.g., `url(#myBlur)`).
    pub filter: Option<DefRef<FilterDef>>,
}
