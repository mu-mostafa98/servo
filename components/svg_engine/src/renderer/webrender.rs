/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! WebRender backend — translates paint commands into WebRender display list calls.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    BorderRadius, ClipChainId, ClipMode, CommonItemProperties, ComplexClipRegion,
    DisplayListBuilder, PrimitiveFlags, PropertyBinding, ReferenceFrameKind,
    SpatialId, TransformStyle,
};

use super::{Backend, ClipDesc, FillRectDesc, PaintColorDesc, RadiiDesc};

pub(crate) struct WebRenderBackend<'a> {
    pub wr: &'a mut DisplayListBuilder,
}

impl Backend for WebRenderBackend<'_> {
    fn fill_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, clip: Option<ClipDesc>,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    ) {
        let rect = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bounds.x, bounds.y),
            LayoutSize::new(bounds.w, bounds.h),
        );

        let eff_clip = match clip {
            Some(c) => {
                let clip_id = self.wr.define_clip_rounded_rect(
                    spatial_id,
                    ComplexClipRegion {
                        rect,
                        radii: BorderRadius {
                            top_left: LayoutSize::new(c.rx, c.ry),
                            top_right: LayoutSize::new(c.rx, c.ry),
                            bottom_left: LayoutSize::new(c.rx, c.ry),
                            bottom_right: LayoutSize::new(c.rx, c.ry),
                        },
                        mode: ClipMode::Clip,
                    },
                );
                let parent = (clip_chain_id != ClipChainId::INVALID).then_some(clip_chain_id);
                self.wr.define_clip_chain(parent, [clip_id])
            }
            None => clip_chain_id,
        };

        let info = CommonItemProperties {
            clip_rect: rect,
            clip_chain_id: eff_clip,
            spatial_id,
            flags: PrimitiveFlags::default(),
        };
        self.wr.push_rect(&info, rect, webrender_api::ColorF::new(color.r, color.g, color.b, color.a));
    }

    fn stroke_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, width: f32, radii: Option<RadiiDesc>,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    ) {
        let rect = LayoutRect::from_origin_and_size(
            LayoutPoint::new(bounds.x, bounds.y),
            LayoutSize::new(bounds.w, bounds.h),
        );

        let r = radii.unwrap_or(RadiiDesc { rx: 0.0, ry: 0.0 });

        let info = CommonItemProperties {
            clip_rect: rect,
            clip_chain_id,
            spatial_id,
            flags: PrimitiveFlags::default(),
        };

        let border = webrender_api::BorderSide {
            color: webrender_api::ColorF::new(color.r, color.g, color.b, color.a),
            style: webrender_api::BorderStyle::Solid,
        };
        let widths = webrender_api::units::LayoutSideOffsets::new_all_same(width);
        let details = webrender_api::BorderDetails::Normal(webrender_api::NormalBorder {
            top: border, right: border, bottom: border, left: border,
            radius: BorderRadius {
                top_left: LayoutSize::new(r.rx, r.ry),
                top_right: LayoutSize::new(r.rx, r.ry),
                bottom_left: LayoutSize::new(r.rx, r.ry),
                bottom_right: LayoutSize::new(r.rx, r.ry),
            },
            do_aa: true,
        });
        self.wr.push_border(&info, rect, widths, details);
    }

    fn draw_image(
        &mut self, x: f32, y: f32, w: u32, h: u32, _data: &[u8],
        _fallback: PaintColorDesc, _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        // The actual vello_cpu image is pushed by the display list builder
        // via push_image(key). This fallback path is only used when the image
        // cache upload fails (no key available).
        let _ = (x, y, w, h);
    }

    fn stroke_line(
        &mut self, x1: f32, y1: f32, x2: f32, y2: f32,
        color: PaintColorDesc, width: f32,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    ) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.01 {
            return;
        }

        let angle = dy.atan2(dx);
        let cos = angle.cos();
        let sin = angle.sin();
        let lt = webrender_api::units::LayoutTransform::new(
            cos,  sin, 0.0, 0.0,
           -sin,  cos, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            x1, y1, 0.0, 1.0,
        );

        let child_spatial = self.wr.push_reference_frame(
            LayoutPoint::zero(),
            spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(lt),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );

        let rect = LayoutRect::from_origin_and_size(
            LayoutPoint::new(0.0, -width / 2.0),
            LayoutSize::new(length, width),
        );

        let info = CommonItemProperties {
            clip_rect: rect,
            clip_chain_id,
            spatial_id: child_spatial,
            flags: PrimitiveFlags::default(),
        };
        self.wr.push_rect(&info, rect, webrender_api::ColorF::new(color.r, color.g, color.b, color.a));

        self.wr.pop_reference_frame();
    }
}
