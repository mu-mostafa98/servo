/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG node effects — visibility, display, transforms, clipping, and masking.
//!
//! These types are future SVG spec stubs and are not yet wired into
//! the rendering pipeline.

/// Element visibility.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

/// Element display type.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Display {
    Inline,
    Block,
    None,
}

/// SVG node effects — clip-path, mask.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}
