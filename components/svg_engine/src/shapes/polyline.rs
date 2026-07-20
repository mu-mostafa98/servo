/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::render_tree::ClipPathUnits;
use crate::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE};

/// SVG `<polyline>` element — an open sequence of connected line segments.
#[derive(Debug, Clone)]
pub struct Polyline {
    pub points: Vec<Point>,
}

impl Polyline {
    /// Clip geometry for this polyline (bounding box around all points).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        clip_points(&self.points, svg_origin, units)
    }
}

/// Shared helper: compute clip geometry from a list of points.
/// Used by both [`Polyline`] and [`Polygon`].
pub(crate) fn clip_points(
    pts: &[Point],
    svg_origin: &LayoutPoint,
    units: ClipPathUnits,
) -> Option<ClipGeometry> {
    if pts.len() < 2 {
        return None;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for p in pts {
        let (x, y) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                p.x as f32 * OBJECT_BBOX_REF_SIZE,
                p.y as f32 * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (p.x as f32, p.y as f32)
        };
        if x < min_x {
            min_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if x > max_x {
            max_x = x;
        }
        if y > max_y {
            max_y = y;
        }
    }

    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + min_x, svg_origin.y + min_y),
        LayoutSize::new((max_x - min_x).max(1.0), (max_y - min_y).max(1.0)),
    );
    Some(ClipGeometry::Polygon { bounds })
}
