/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::render_tree::ClipPathUnits;
use crate::shapes::{all_equal_radius, ClipGeometry, OBJECT_BBOX_REF_SIZE};

/// SVG `<rect>` element.
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rx: Option<f32>,
    pub ry: Option<f32>,
}

impl Rectangle {
    /// Clip geometry for this rectangle.
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (x, y, w, h) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.x * OBJECT_BBOX_REF_SIZE,
                self.y * OBJECT_BBOX_REF_SIZE,
                self.width * OBJECT_BBOX_REF_SIZE,
                self.height * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (self.x, self.y, self.width, self.height)
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + x, svg_origin.y + y),
            LayoutSize::new(w, h),
        );
        let radii = match (self.rx, self.ry) {
            (Some(rx), _) if rx > 0.0 => {
                let ry = self.ry.unwrap_or(rx);
                Some(all_equal_radius(
                    rx.clamp(0.0, w / 2.0),
                    ry.clamp(0.0, h / 2.0),
                ))
            },
            (_, Some(ry)) if ry > 0.0 => Some(all_equal_radius(
                ry.clamp(0.0, h / 2.0),
                ry.clamp(0.0, h / 2.0),
            )),
            _ => None,
        };
        Some(match radii {
            Some(r) => ClipGeometry::RoundedRect { bounds, radii: r },
            None => ClipGeometry::Polygon { bounds },
        })
    }
}
