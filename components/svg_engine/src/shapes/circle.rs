/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::render_tree::ClipPathUnits;
use crate::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE, all_equal_radius};

/// SVG `<circle>` element.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl Circle {
    /// Clip geometry for this circle.
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (cx, cy, r) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.cx * OBJECT_BBOX_REF_SIZE,
                self.cy * OBJECT_BBOX_REF_SIZE,
                self.r * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (self.cx, self.cy, self.r)
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + cx - r, svg_origin.y + cy - r),
            LayoutSize::new(r * 2.0, r * 2.0),
        );
        Some(ClipGeometry::RoundedRect {
            bounds,
            radii: all_equal_radius(r, r),
        })
    }
}
