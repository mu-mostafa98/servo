/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG stroke properties — pure data types, no WebRender dependency.

use svgtypes::Color as SvgColor;
use super::gradient::PaintServer;

/// SVG stroke properties.
#[derive(Debug, Clone)]
pub struct StrokeParams {
    pub color: Option<SvgColor>,
    /// Paint server reference (gradient url). When set, takes priority over `color`.
    pub paint_server: Option<PaintServer>,
    pub opacity: f32,
    pub width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Option<Vec<f32>>,
    pub dash_offset: f32,
}

/// SVG line cap style — how the ends of open paths are rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style — how corners are rendered in a polyline/polygon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}
