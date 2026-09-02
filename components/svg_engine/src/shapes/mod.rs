/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! This module defines SVG geometric shape structs based on the SVG 2 specification.
//! Each shape has its own file with its struct definition.
//!
//! Shapes are pure data structs constructed directly from computed geometry
//! in the layout integration layer.

pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod rectangle;

use webrender_api::BorderRadius;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use kurbo::Shape as _;

pub use self::circle::Circle;
pub use self::ellipse::Ellipse;
pub use self::line::Line;
pub use self::path::Path;
pub use self::polygon::Polygon;
pub use self::polyline::Polyline;
pub use self::rectangle::Rectangle;
use crate::render_tree::ClipPathUnits;

/// Scale factor for objectBoundingBox clip-path coordinates (0..1 → 0..100).
pub(crate) const OBJECT_BBOX_REF_SIZE: f32 = 100.0;

/// Clip geometry result — either a rounded-rect clip (radii radius) or a
/// polygon clip (polygon points).  The polygon variant pre-computes the
/// bounding rect so callers can still size an image mask or bounding clip.
#[derive(Debug, Clone)]
pub(crate) enum ClipGeometry {
    RoundedRect {
        bounds: LayoutRect,
        radii: BorderRadius,
    },
    Polygon {
        bounds: LayoutRect,
    },
}

// ======================= An SVG geometric shape =======================
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
    /// Return clip geometry for this shape, if supported.
    ///
    /// Returns `None` for shapes that cannot participate in clip paths
    /// (line only — has no area).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        match self {
            Shape::Rect(r) => r.clip_info(svg_origin, units),
            Shape::Circle(c) => c.clip_info(svg_origin, units),
            Shape::Ellipse(e) => e.clip_info(svg_origin, units),
            Shape::Polygon(p) => p.clip_info(svg_origin, units),
            Shape::Polyline(p) => p.clip_info(svg_origin, units),
            Shape::Path(p) => p.clip_info(svg_origin, units),
            Shape::Line(_) => None,
        }
    }
}

impl Shape {
    /// Convert the shape to a [`kurbo::BezPath`] in its local coordinate
    /// space, used for vello_cpu rasterization (gradient fills/strokes).
    pub(crate) fn to_bez_path(&self) -> Option<kurbo::BezPath> {
        use kurbo::{BezPath, Circle, Ellipse, Rect, RoundedRect, RoundedRectRadii, Vec2};

        match self {
            Shape::Rect(r) => {
                let x0 = r.x as f64;
                let y0 = r.y as f64;
                let x1 = (r.x + r.width) as f64;
                let y1 = (r.y + r.height) as f64;
                let rx = r.rx.unwrap_or(0.0) as f64;
                let ry = r.ry.unwrap_or(rx as f32) as f64;
                if rx > 0.0 || ry > 0.0 {
                    let radius = (rx + ry) / 2.0;
                    Some(RoundedRect::new(x0, y0, x1, y1, RoundedRectRadii::from(radius)).to_path(0.1))
                } else {
                    Some(Rect::new(x0, y0, x1, y1).to_path(0.1))
                }
            },
            Shape::Circle(c) => {
                Some(Circle::new((c.cx as f64, c.cy as f64), c.r as f64).to_path(0.1))
            },
            Shape::Ellipse(e) => {
                Some(Ellipse::new(
                    (e.cx as f64, e.cy as f64),
                    Vec2::new(e.rx as f64, e.ry as f64),
                    0.0,
                ).to_path(0.1))
            },
            Shape::Line(l) => {
                let mut bez = BezPath::new();
                bez.move_to((l.x1 as f64, l.y1 as f64));
                bez.line_to((l.x2 as f64, l.y2 as f64));
                Some(bez)
            },
            Shape::Polyline(p) => Some(points_to_bez(&p.points, false)),
            Shape::Polygon(p) => Some(points_to_bez(&p.points, true)),
            Shape::Path(p) => Some(p.path.clone()),
        }
    }
}

/// Build an open or closed [`kurbo::BezPath`] from a list of points.
fn points_to_bez(points: &[kurbo::Point], close: bool) -> kurbo::BezPath {
    let mut bez = kurbo::BezPath::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            bez.move_to((p.x, p.y));
        } else {
            bez.line_to((p.x, p.y));
        }
    }
    if close {
        bez.close_path();
    }
    bez
}

// ======================= Clip geometry helpers =======================

/// Build a BorderRadius with the same (rx, ry) on all four corners.
pub(crate) fn all_equal_radius(rx: f32, ry: f32) -> BorderRadius {
    BorderRadius {
        top_left: LayoutSize::new(rx, ry),
        top_right: LayoutSize::new(rx, ry),
        bottom_left: LayoutSize::new(rx, ry),
        bottom_right: LayoutSize::new(rx, ry),
    }
}
