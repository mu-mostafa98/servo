/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG Geometric Shapes Reference: https://www.w3.org/TR/SVG2/shapes.html
//!
//! This module defines SVG geometric shape structs based on the SVG 2 specification.
//! Each shape has its own file with its struct definition.
//! Shape construction is handled by the integration layer in `components/layout/svg_builder.rs`.

use kurbo::ParamCurve;
pub(crate) mod rectangle;
pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod polyline;
pub(crate) mod polygon;
pub(crate) mod path;
pub mod attr_parsers;

pub use self::rectangle::Rectangle;
pub use self::circle::Circle;
pub use self::ellipse::Ellipse;
pub use self::line::Line;
pub use self::polyline::Polyline;
pub use self::polygon::Polygon;
pub use self::path::Path;

use webrender_api::{
    BorderRadius,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::render_tree::ClipPathUnits;

/// Scale factor for objectBoundingBox clip-path coordinates (0..1 → 0..100).
const OBJECT_BBOX_REF_SIZE: f32 = 100.0;

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
    /// Return clip geometry for this shape, if supported.
    ///
    /// Returns `None` for shapes that cannot participate in clip paths
    /// (line only — has no area).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<(LayoutRect, Option<BorderRadius>)> {
        match self {
            Shape::Rect(r) => clip_info_rect(r, svg_origin, units),
            Shape::Circle(c) => clip_info_circle(c, svg_origin, units),
            Shape::Ellipse(e) => clip_info_ellipse(e, svg_origin, units),
            Shape::Polygon(p) => clip_info_points(&p.points, svg_origin, units),
            Shape::Polyline(p) => clip_info_points(&p.points, svg_origin, units),
            Shape::Path(p) => clip_info_path(&p.path, svg_origin, units),
            Shape::Line(_) => None,
        }
    }
}

// ======================= Clip geometry helpers =======================

fn clip_info_rect(
    r: &Rectangle, svg_origin: &LayoutPoint, units: ClipPathUnits,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    let (x, y, w, h) = if units == ClipPathUnits::ObjectBoundingBox {
        (r.x * OBJECT_BBOX_REF_SIZE, r.y * OBJECT_BBOX_REF_SIZE,
         r.width * OBJECT_BBOX_REF_SIZE, r.height * OBJECT_BBOX_REF_SIZE)
    } else {
        (r.x, r.y, r.width, r.height)
    };
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + x, svg_origin.y + y),
        LayoutSize::new(w, h),
    );
    let radii = match (r.rx, r.ry) {
        (Some(rx), _) if rx > 0.0 => {
            let ry = r.ry.unwrap_or(rx);
            Some(all_equal_radius(rx.clamp(0.0, w / 2.0), ry.clamp(0.0, h / 2.0)))
        },
        (_, Some(ry)) if ry > 0.0 => {
            Some(all_equal_radius(ry.clamp(0.0, h / 2.0), ry.clamp(0.0, h / 2.0)))
        },
        _ => None,
    };
    Some((bounds, radii))
}

fn clip_info_circle(
    c: &Circle, svg_origin: &LayoutPoint, units: ClipPathUnits,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    let (cx, cy, r) = if units == ClipPathUnits::ObjectBoundingBox {
        (c.cx * OBJECT_BBOX_REF_SIZE, c.cy * OBJECT_BBOX_REF_SIZE, c.r * OBJECT_BBOX_REF_SIZE)
    } else {
        (c.cx, c.cy, c.r)
    };
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + cx - r, svg_origin.y + cy - r),
        LayoutSize::new(r * 2.0, r * 2.0),
    );
    let radii = all_equal_radius(r, r);
    Some((bounds, Some(radii)))
}

fn clip_info_ellipse(
    e: &Ellipse, svg_origin: &LayoutPoint, units: ClipPathUnits,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    let (cx, cy, rx, ry) = if units == ClipPathUnits::ObjectBoundingBox {
        (e.cx * OBJECT_BBOX_REF_SIZE, e.cy * OBJECT_BBOX_REF_SIZE,
         e.rx * OBJECT_BBOX_REF_SIZE, e.ry * OBJECT_BBOX_REF_SIZE)
    } else {
        (e.cx, e.cy, e.rx, e.ry)
    };
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + cx - rx, svg_origin.y + cy - ry),
        LayoutSize::new(rx * 2.0, ry * 2.0),
    );
    let radii = all_equal_radius(rx, ry);
    Some((bounds, Some(radii)))
}

/// Build a BorderRadius with the same (rx, ry) on all four corners.
fn all_equal_radius(rx: f32, ry: f32) -> BorderRadius {
    BorderRadius {
        top_left: LayoutSize::new(rx, ry), top_right: LayoutSize::new(rx, ry),
        bottom_left: LayoutSize::new(rx, ry), bottom_right: LayoutSize::new(rx, ry),
    }
}

/// Compute clip geometry from a list of kurbo points (polygon/polyline).
fn clip_info_points(
    pts: &[kurbo::Point],
    svg_origin: &LayoutPoint,
    units: ClipPathUnits,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    if pts.len() < 2 {
        return None;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for p in pts {
        let (x, y) = if units == ClipPathUnits::ObjectBoundingBox {
            (p.x as f32 * OBJECT_BBOX_REF_SIZE, p.y as f32 * OBJECT_BBOX_REF_SIZE)
        } else {
            (p.x as f32, p.y as f32)
        };
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    }

    let w = (max_x - min_x).max(1.0);
    let h = (max_y - min_y).max(1.0);
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + min_x, svg_origin.y + min_y),
        LayoutSize::new(w, h),
    );
    Some((bounds, None))
}

/// Compute clip geometry from a BezPath.
fn clip_info_path(
    path: &kurbo::BezPath,
    svg_origin: &LayoutPoint,
    units: ClipPathUnits,
) -> Option<(LayoutRect, Option<BorderRadius>)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut has_points = false;

    for seg in path.segments() {
        let p = seg.end();
        let (x, y) = if units == ClipPathUnits::ObjectBoundingBox {
            (p.x as f32 * OBJECT_BBOX_REF_SIZE, p.y as f32 * OBJECT_BBOX_REF_SIZE)
        } else {
            (p.x as f32, p.y as f32)
        };
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
        has_points = true;
    }

    if !has_points {
        return None;
    }

    let w = (max_x - min_x).max(1.0);
    let h = (max_y - min_y).max(1.0);
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + min_x, svg_origin.y + min_y),
        LayoutSize::new(w, h),
    );
    Some((bounds, None))
}
