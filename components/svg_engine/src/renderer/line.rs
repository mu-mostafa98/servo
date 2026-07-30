/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<line>` renderer — draws via WebRender rotated reference frame.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    CommonItemProperties, PrimitiveFlags, PropertyBinding, ReferenceFrameKind,
    TransformStyle,
};

use super::RenderContext;

/// Render an SVG `<line>` as a rotated rectangle.
pub(crate) fn render(
    shape: &usvg::SimpleShape,
    x1: f32, y1: f32, x2: f32, y2: f32,
    ctx: &mut RenderContext,
) {
    if !shape.is_visible() {
        return;
    }

    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.01 {
        return;
    }

    let stroke_width = shape
        .stroke()
        .map(|s| s.width().get())
        .unwrap_or(1.0);
    let angle = dy.atan2(dx);

    let cos = angle.cos();
    let sin = angle.sin();
    let lt = webrender_api::units::LayoutTransform::new(
        cos,  sin, 0.0, 0.0,
       -sin,  cos, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        ctx.svg_origin.x + x1, ctx.svg_origin.y + y1, 0.0, 1.0,
    );

    let spatial_id = ctx.wr.push_reference_frame(
        LayoutPoint::zero(),
        ctx.spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(lt),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    );

    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(0.0, -stroke_width / 2.0),
        LayoutSize::new(length, stroke_width),
    );

    if let Some(ref stroke) = shape.stroke() {
        let color = match stroke.paint() {
            usvg::Paint::Color(c) => c,
            _ => &usvg::Color::black(),
        };
        let c = webrender_api::ColorF::new(
            color.red as f32 / 255.0,
            color.green as f32 / 255.0,
            color.blue as f32 / 255.0,
            stroke.opacity().get(),
        );

        let info = CommonItemProperties {
            clip_rect: bounds,
            clip_chain_id: ctx.clip_chain_id,
            spatial_id,
            flags: PrimitiveFlags::default(),
        };
        ctx.wr.push_rect(&info, bounds, c);
    }

    ctx.wr.pop_reference_frame();
}
