/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Angle;
use webrender_api::{
    PropertyBinding, ReferenceFrameKind, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::shapes::Line;
use crate::renderer::{Render, RenderContext, make_common_props, to_colorf, effective_stroke_width, ZERO_LENGTH_EPSILON};

/// Renders an SVG `<line>`.
///
/// LSP contract:
/// - SVG specification: `<line>` has no fill geometry.
/// - Only emits stroke commands (via [`push_rect`] in a rotated frame).
/// - Correctly ignores `ctx.style.fill` even when `Some`.
/// - This is a **spec-mandated behavioral difference**, not a violation
///   of LSP — the [`Render`] trait contract explicitly permits shapes
///   to omit fill when the SVG spec requires it.
impl Render for Line {
    fn render(&self, ctx: &mut RenderContext) {
        let Some(stroke) = &ctx.style.stroke else { return };
        let Some(svg_color) = stroke.color else { return };
        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;

        let stroke_width = effective_stroke_width(ctx, stroke.width);
        if stroke_width <= 0.0 {
            return;
        }

        let x1 = ctx.svg_origin.x + self.x1;
        let y1 = ctx.svg_origin.y + self.y1;
        let x2 = ctx.svg_origin.x + self.x2;
        let y2 = ctx.svg_origin.y + self.y2;

        // Zero-length line — nothing to render
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < ZERO_LENGTH_EPSILON {
            return;
        }

        // Compute midpoint and angle
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let angle = dy.atan2(dx);
        let half_w = stroke_width / 2.0;

        // Create a rotated reference frame centered at the midpoint,
        // aligned with the line direction.
        // In this rotated space, the line is a horizontal rect centered at the origin.
        let transform = LayoutTransform::rotation(0.0, 0.0, 1.0, Angle::radians(angle));
        let line_spatial_id = ctx.wr.push_reference_frame(
            LayoutPoint::new(mx, my),
            ctx.spatial_id,
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
            LayoutSize::new(len, stroke_width),
        );
        let common = make_common_props(line_bounds, line_spatial_id, ctx.clip_chain_id);
        ctx.wr.push_rect(&common, line_bounds, color);
        ctx.wr.pop_reference_frame();
    }
}
