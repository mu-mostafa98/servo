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

pub mod attr_parsers;
pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod rectangle;

use webrender_api::BorderRadius;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

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


