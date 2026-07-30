/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<rect>` renderer — draws via WebRender rounded-rect clips.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{BorderRadius, ClipMode, ComplexClipRegion, CommonItemProperties, PrimitiveFlags};

use super::RenderContext;

/// Render an SVG `<rect>` shape into WebRender display list commands.
pub(crate) fn render(
    shape: &usvg::SimpleShape,
    x: f32, y: f32, width: f32, height: f32,
    rx: Option<f32>, ry: Option<f32>,
    ctx: &mut RenderContext,
) {
    if !shape.is_visible() {
        return;
    }

    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(ctx.svg_origin.x + x, ctx.svg_origin.y + y),
        LayoutSize::new(width, height),
    );

    // Clamp corner radii
    let r = rx.or(ry).unwrap_or(0.0).clamp(0.0, width / 2.0);
    let ry_val = ry.or(rx).unwrap_or(0.0).clamp(0.0, height / 2.0);
    let has_radius = r > 0.0 || ry_val > 0.0;

    let clip_chain_id = if has_radius {
        let clip_id = ctx.wr.define_clip_rounded_rect(
            ctx.spatial_id,
            ComplexClipRegion {
                rect: bounds,
                radii: BorderRadius {
                    top_left: LayoutSize::new(r, ry_val),
                    top_right: LayoutSize::new(r, ry_val),
                    bottom_left: LayoutSize::new(r, ry_val),
                    bottom_right: LayoutSize::new(r, ry_val),
                },
                mode: ClipMode::Clip,
            },
        );
        let parent = (ctx.clip_chain_id != webrender_api::ClipChainId::INVALID)
            .then_some(ctx.clip_chain_id);
        ctx.wr.define_clip_chain(parent, [clip_id])
    } else {
        ctx.clip_chain_id
    };

    // Fill
    if let Some(ref fill) = shape.fill() {
        let color = match fill.paint() {
            usvg::Paint::Color(c) => c,
            _ => &usvg::Color::black(),
        };
        let opacity = fill.opacity().get();
        let c = webrender_api::ColorF::new(
            color.red as f32 / 255.0,
            color.green as f32 / 255.0,
            color.blue as f32 / 255.0,
            opacity,
        );

        let info = CommonItemProperties {
            clip_rect: bounds,
            clip_chain_id,
            spatial_id: ctx.spatial_id,
            flags: PrimitiveFlags::default(),
        };
        ctx.wr.push_rect(&info, bounds, c);
    }

    // Stroke
    if let Some(ref stroke) = shape.stroke() {
        let color = match stroke.paint() {
            usvg::Paint::Color(c) => c,
            _ => &usvg::Color::black(),
        };
        let opacity = stroke.opacity().get();
        let sw = stroke.width().get();

        let c = webrender_api::ColorF::new(
            color.red as f32 / 255.0,
            color.green as f32 / 255.0,
            color.blue as f32 / 255.0,
            opacity,
        );

        let info = CommonItemProperties {
            clip_rect: bounds,
            clip_chain_id: ctx.clip_chain_id,
            spatial_id: ctx.spatial_id,
            flags: PrimitiveFlags::default(),
        };

        let border = webrender_api::BorderSide {
            color: c,
            style: webrender_api::BorderStyle::Solid,
        };
        let widths = webrender_api::units::LayoutSideOffsets::new_all_same(sw);
        let details = webrender_api::BorderDetails::Normal(webrender_api::NormalBorder {
            top: border,
            right: border,
            bottom: border,
            left: border,
            radius: BorderRadius {
                top_left: LayoutSize::new(r, ry_val),
                top_right: LayoutSize::new(r, ry_val),
                bottom_left: LayoutSize::new(r, ry_val),
                bottom_right: LayoutSize::new(r, ry_val),
            },
            do_aa: true,
        });
        ctx.wr.push_border(&info, bounds, widths, details);
    }
}
