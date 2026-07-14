/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Phase 1: only fill styling. Stroke, gradient, transform, etc. added later.

pub mod fill;
pub(crate) mod visibility;

pub use self::fill::{FillParams, FillRule};
pub use self::visibility::{Display, Visibility};

/// Minimal node style for Phase 1 — only fill + opacity + basic visibility.
#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub visibility: Visibility,
    pub display: Display,
    pub fill: Option<FillParams>,
    /// Element-level opacity (CSS `opacity` property).
    pub opacity: f32,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            fill: None,
            opacity: 1.0,
        }
    }
}

impl NodeStyle {
    /// Whether the element is visible (per the SVG `visibility` property).
    pub fn is_visible(&self) -> bool {
        matches!(self.visibility, Visibility::Visible)
    }

    /// Whether the element is displayed (per the SVG `display` property).
    /// Returns `false` for `display: none`.
    pub fn is_displayed(&self) -> bool {
        !matches!(self.display, Display::None)
    }
}
