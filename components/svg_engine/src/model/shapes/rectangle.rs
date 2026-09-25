/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::model::tree::ClipPathUnits;
use crate::model::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE, all_equal_radius};
use crate::model::units::Length;

/// SVG `<rect>` element.
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: Length,
    pub y: Length,
    pub width: Length,
    pub height: Length,
    pub rx: Option<Length>,
    pub ry: Option<Length>,
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
                self.x.get() * OBJECT_BBOX_REF_SIZE,
                self.y.get() * OBJECT_BBOX_REF_SIZE,
                self.width.get() * OBJECT_BBOX_REF_SIZE,
                self.height.get() * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (
                self.x.get(),
                self.y.get(),
                self.width.get(),
                self.height.get(),
            )
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + x, svg_origin.y + y),
            LayoutSize::new(w, h),
        );
        let radii = match (self.rx, self.ry) {
            (Some(rx), _) if rx.get() > 0.0 => {
                let ry = self.ry.unwrap_or(rx);
                Some(all_equal_radius(
                    rx.get().clamp(0.0, w / 2.0),
                    ry.get().clamp(0.0, h / 2.0),
                ))
            },
            (_, Some(ry)) if ry.get() > 0.0 => Some(all_equal_radius(
                ry.get().clamp(0.0, h / 2.0),
                ry.get().clamp(0.0, h / 2.0),
            )),
            _ => None,
        };
        Some(match radii {
            Some(r) => ClipGeometry::RoundedRect { bounds, radii: r },
            None => ClipGeometry::Rect { bounds },
        })
    }
}
