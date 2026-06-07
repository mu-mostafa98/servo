/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
// This module defines SVG geometric shape structs based on the SVG 2 specification.

use kurbo::{BezPath, Point};

// ------------------- Geometry ------------------

#[derive(Debug)]
pub enum Shape {
    Rect(Rectangle),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polyline(Polyline),
    Polygon(Polygon),
    Path(Path),
}

#[derive(Debug)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rx: Option<f32>,
    pub ry: Option<f32>,
}

#[derive(Debug)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

#[derive(Debug)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

#[derive(Debug)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Debug)]
pub struct Polyline {
    pub points: Vec<Point>,
}

#[derive(Debug)]
pub struct Polygon {
    pub points: Vec<Point>,
}

#[derive(Debug)]
pub struct Path {
    pub path: BezPath, // The 'd' attribute content
}
