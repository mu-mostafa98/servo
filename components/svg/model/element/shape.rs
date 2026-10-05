/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG geometric shapes — a single module for all `<shape>` elements.
//!
//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! Shapes are pure data structs constructed directly from computed geometry
//! in the layout integration layer.

use crate::model::geometry::{PathData, Point};
use crate::model::units::Length;

/// SVG `<rect>` element.
///
/// `rx`/`ry` are `Option<Length>`: `None` is SVG 2's `auto` value, resolved to
/// a used radius against the sibling (or to `0` when both are `auto`) at render
/// time.
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: Length,
    pub y: Length,
    pub width: Length,
    pub height: Length,
    pub rx: Option<Length>,
    pub ry: Option<Length>,
    /// The `pathLength` presentation attribute (a positive number), or `None`.
    pub path_length: Option<f32>,
}

/// SVG `<circle>` element.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: Length,
    pub cy: Length,
    pub r: Length,
    pub path_length: Option<f32>,
}

/// SVG `<ellipse>` element.
///
/// Like [`Rectangle`], `rx`/`ry` are `Option<Length>`: SVG 2 makes the radii
/// support `auto`, so a single `auto` radius derives its used value from the
/// other (producing a circle), while `auto`+`auto` disables rendering.
#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: Length,
    pub cy: Length,
    pub rx: Option<Length>,
    pub ry: Option<Length>,
    pub path_length: Option<f32>,
}

/// SVG `<line>` element.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: Length,
    pub y1: Length,
    pub x2: Length,
    pub y2: Length,
    pub path_length: Option<f32>,
}

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
    pub path_length: Option<f32>,
}

/// SVG `<polygon>` element — a closed shape formed by connected line segments.
#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point>,
    pub path_length: Option<f32>,
}

/// SVG `<path>` element with its `d` attribute parsed into a [`PathData`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: PathData,
    pub path_length: Option<f32>,
}

/// An SVG geometric shape.
#[derive(Debug, Clone)]
pub enum Shape {
    Rect(Rectangle),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polyline(Polyline),
    Polygon(Polygon),
    Path(Path),
}

impl Shape {
    /// The shape's `pathLength` presentation attribute, if specified.
    ///
    /// Every basic shape (and `<path>`) carries `pathLength`; it calibrates
    /// distance-along-a-path computations (notably stroke dashing) against the
    /// author-asserted total path length.
    pub fn path_length(&self) -> Option<f32> {
        match self {
            Shape::Rect(r) => r.path_length,
            Shape::Circle(c) => c.path_length,
            Shape::Ellipse(e) => e.path_length,
            Shape::Line(l) => l.path_length,
            Shape::Polyline(p) => p.path_length,
            Shape::Polygon(p) => p.path_length,
            Shape::Path(p) => p.path_length,
        }
    }
}

impl Rectangle {
    /// Resolve the `auto` corner radii to used values.
    ///
    /// SVG 2: both `rx`/`ry` `auto` resolve to `0`; a single `auto` radius
    /// derives its used value from the other.
    pub fn resolved_radii(&self) -> (Length, Length) {
        (
            self.rx.or(self.ry).unwrap_or(Length::new(0.0)),
            self.ry.or(self.rx).unwrap_or(Length::new(0.0)),
        )
    }
}

impl Ellipse {
    /// Resolve the `auto` radii to concrete used values.
    ///
    /// SVG 2: both `rx`/`ry` `auto` → `None` (no rendering); a single `auto`
    /// radius derives its used value from the other (yielding a circle).
    pub fn resolved_radii(&self) -> Option<(Length, Length)> {
        match (self.rx, self.ry) {
            (Some(rx), Some(ry)) => Some((rx, ry)),
            (Some(r), None) => Some((r, r)),
            (None, Some(r)) => Some((r, r)),
            (None, None) => None,
        }
    }
}
