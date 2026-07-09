/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG fill properties — pure data types, no WebRender dependency.

use svgtypes::Color as SvgColor;
use super::gradient::PaintServer;

/// SVG fill properties.
#[derive(Debug, Clone)]
pub struct FillParams {
    pub color: Option<SvgColor>,
    /// Paint server reference (gradient url). When set, takes priority over `color`.
    pub paint_server: Option<PaintServer>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

/// SVG fill rule: determines how overlapping regions are filled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
