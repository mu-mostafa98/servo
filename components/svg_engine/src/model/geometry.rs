/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pure geometry primitives for the SVG data model.
//!
//! Kept free of any external geometry library (`kurbo`, `webrender`, …): the
//! layout layer produces these from parsed attribute values, and the render
//! layer converts them into [`kurbo::BezPath`] for rasterization.

/// A 2D point in SVG user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Construct a point from raw `f32` coordinates.
    pub fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// A single SVG path command, with all coordinates already resolved to
/// absolute user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCommand {
    MoveTo(Point),
    LineTo(Point),
    QuadTo(Point, Point),
    CurveTo(Point, Point, Point),
    Close,
}

/// A parsed SVG path (`d` attribute), stored as a flat list of
/// [`PathCommand`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct PathData {
    pub commands: Vec<PathCommand>,
}
