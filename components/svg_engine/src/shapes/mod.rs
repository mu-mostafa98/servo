/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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

pub(crate) const OBJECT_BBOX_REF_SIZE: f32 = 100.0;

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

pub(crate) fn all_equal_radius(rx: f32, ry: f32) -> BorderRadius {
    BorderRadius {
        top_left: LayoutSize::new(rx, ry),
        top_right: LayoutSize::new(rx, ry),
        bottom_left: LayoutSize::new(rx, ry),
        bottom_right: LayoutSize::new(rx, ry),
    }
}
