/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::render_tree::ClipPathUnits;
use crate::shapes::{all_equal_radius, ClipGeometry, OBJECT_BBOX_REF_SIZE};

/// SVG `<ellipse>` element.
#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

impl Ellipse {
    /// Clip geometry for this ellipse.
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (cx, cy, rx, ry) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.cx * OBJECT_BBOX_REF_SIZE,
                self.cy * OBJECT_BBOX_REF_SIZE,
                self.rx * OBJECT_BBOX_REF_SIZE,
                self.ry * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (self.cx, self.cy, self.rx, self.ry)
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + cx - rx, svg_origin.y + cy - ry),
            LayoutSize::new(rx * 2.0, ry * 2.0),
        );
        Some(ClipGeometry::RoundedRect {
            bounds,
            radii: all_equal_radius(rx, ry),
        })
    }
}
