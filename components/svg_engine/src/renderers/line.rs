/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::atomic::{AtomicU64, Ordering};

use euclid::Angle;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, PropertyBinding, ReferenceFrameKind,
    SpatialTreeItemKey, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::shapes::Line;
use crate::styles::*;

static SVG_LINE_KEY: AtomicU64 = AtomicU64::new(1);

pub fn render_line(
    line: &Line,
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

    let x1 = svg_origin.x + line.x1;
    let y1 = svg_origin.y + line.y1;
    let x2 = svg_origin.x + line.x2;
    let y2 = svg_origin.y + line.y2;
    let half_w = (stroke.width / 2.0).max(0.5);

    if (x1 - x2).abs() < 0.001 && (y1 - y2).abs() < 0.001 {
        return;
    }

    // For axis-aligned lines, use push_rect directly
    if (x1 - x2).abs() < 0.001 {
        // Vertical line
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x1 - half_w, y1.min(y2)),
            LayoutSize::new(stroke.width, (y1 - y2).abs()),
        );
        let common = CommonItemProperties::new(
            bounds,
            webrender_api::SpaceAndClipInfo { spatial_id, clip_chain_id },
        );
        wr.push_rect(&common, bounds, color);
        return;
    }

    if (y1 - y2).abs() < 0.001 {
        // Horizontal line
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x1.min(x2), y1 - half_w),
            LayoutSize::new((x1 - x2).abs(), stroke.width),
        );
        let common = CommonItemProperties::new(
            bounds,
            webrender_api::SpaceAndClipInfo { spatial_id, clip_chain_id },
        );
        wr.push_rect(&common, bounds, color);
        return;
    }

    // Angled line — rotated rect via reference frame
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);
    let mx = (x1 + x2) / 2.0;
    let my = (y1 + y2) / 2.0;

    let key = SVG_LINE_KEY.fetch_add(1, Ordering::Relaxed);
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
        SpatialTreeItemKey::new(0, key),
    );

    let line_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(-len / 2.0, -half_w),
        LayoutSize::new(len, stroke.width),
    );
    let common = CommonItemProperties::new(
        line_bounds,
        webrender_api::SpaceAndClipInfo { spatial_id: line_spatial_id, clip_chain_id },
    );
    wr.push_rect(&common, line_bounds, color);
    wr.pop_reference_frame();
}
