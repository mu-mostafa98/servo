use euclid::Angle;
use webrender_api::{
    CommonItemProperties, PropertyBinding, ReferenceFrameKind, SpaceAndClipInfo, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::shapes::Line;
use crate::renderer::{Render, RenderContext, to_colorf};

impl Render for Line {
    fn render(&self, ctx: &mut RenderContext) {
        let Some(stroke) = &ctx.style.stroke else { return };
        let Some(svg_color) = stroke.color else { return };
        if stroke.width <= 0.0 {
            return;
        }

        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;

        let x1 = ctx.svg_origin.x + self.x1;
        let y1 = ctx.svg_origin.y + self.y1;
        let x2 = ctx.svg_origin.x + self.x2;
        let y2 = ctx.svg_origin.y + self.y2;

        // Zero-length check
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

        // Create a rotated reference frame aligned with the line direction.
        // In this rotated space, the line is a horizontal rect centered at origin.
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
            LayoutSize::new(len, stroke.width),
        );
        let common = CommonItemProperties::new(
            line_bounds,
            SpaceAndClipInfo {
                spatial_id: line_spatial_id,
                clip_chain_id: ctx.clip_chain_id,
            },
        );
        ctx.wr.push_rect(&common, line_bounds, color);
        ctx.wr.pop_reference_frame();
    }
}
