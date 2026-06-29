/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Angle;
use webrender_api::{
    ClipChainId, DisplayListBuilder, PropertyBinding, ReferenceFrameKind, SpatialId,
    TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::shapes::Line;
use crate::styles::*;
use crate::renderer::{Render, make_common_props};

impl Render for Line {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        let Some(stroke) = &style.stroke else { return };
        let Some(mut color) = stroke.color else { return };
        if stroke.width <= 0.0 {
            return;
        }
        color.a *= stroke.opacity;

        let x1 = svg_origin.x + self.x1;
        let y1 = svg_origin.y + self.y1;
        let x2 = svg_origin.x + self.x2;
        let y2 = svg_origin.y + self.y2;

        // Zero-length line — nothing to render
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }

        // Compute midpoint and angle
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let angle = dy.atan2(dx);
        let half_w = stroke.width / 2.0;

        // Create a rotated reference frame centered at the midpoint,
        // aligned with the line direction.
        // In this rotated space, the line is a horizontal rect centered at the origin.
        let transform = LayoutTransform::rotation(0.0, 0.0, 1.0, Angle::radians(angle));
        let line_spatial_id = wr.push_reference_frame(
            LayoutPoint::new(mx, my),
            spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(transform),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );

        let line_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(-len / 2.0, -half_w),
            LayoutSize::new(len, stroke.width),
        );
        let common = make_common_props(line_bounds, line_spatial_id, clip_chain_id);
        wr.push_rect(&common, line_bounds, color);
        wr.pop_reference_frame();
    }
}
