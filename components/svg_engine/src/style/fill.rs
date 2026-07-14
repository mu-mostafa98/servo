/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::Color as SvgColor;

#[derive(Debug, Clone)]
pub struct FillParams {
    pub color: Option<SvgColor>,
    // pub paint_server: Option<PaintServer>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
