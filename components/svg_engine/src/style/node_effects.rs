/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node effects — clip-path and mask.
//!
//! These types are future SVG spec stubs and are not yet wired into
//! the rendering pipeline.

/// SVG node effects — clip-path, mask, filter.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub clip_path: Option<String>,
    pub mask: Option<String>,
    /// Reference to a `<filter>` element (e.g., `url(#myBlur)`).
    pub filter: Option<String>,
}
