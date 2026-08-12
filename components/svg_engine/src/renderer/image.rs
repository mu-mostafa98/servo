/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<image>` element renderer.
//!
//! When the layout layer has resolved the `href` to a WebRender [`ImageKey`]
//! (image already loaded), the image is drawn with `push_image`. The rendered
//! rect is computed by applying `preserveAspectRatio` (default `xMidYMid meet`)
//! to fit the image's natural dimensions within the viewport — avoiding
//! distortion. Otherwise — image not yet loaded, failed to load, or a vector
//! image — a placeholder (gray rect with an X) marks the image area. Once a
//! pending image loads, script triggers a reflow that re-resolves the key to
//! `Some` and the real image is drawn on the next paint.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    AlphaType, ColorF, CommonItemProperties, ImageRendering, SpaceAndClipInfo,
};

use crate::image::SvgImage;
use crate::renderer::{Render, RenderContext};
use crate::traversal::compute_viewbox_transform;

impl Render for SvgImage {
    fn render(&self, ctx: &mut RenderContext) {
        let x = ctx.svg_origin.x + self.x;
        let y = ctx.svg_origin.y + self.y;
        let vp_w = self.width;
        let vp_h = self.height;

        let bounds = if let (Some(nw), Some(nh)) =
            (self.natural_width, self.natural_height)
        {
            // Apply preserveAspectRatio: fit the image's natural size into the
            // viewport rect (x, y, vp_w, vp_h) according to the alignment/slice.
            let (sx, sy, ox, oy) = compute_viewbox_transform(
                nw as f32,
                nh as f32,
                vp_w,
                vp_h,
                Some(&self.preserve_aspect_ratio),
            );
            LayoutRect::from_origin_and_size(
                LayoutPoint::new(x + ox, y + oy),
                LayoutSize::new(nw as f32 * sx, nh as f32 * sy),
            )
        } else {
            // No natural dimensions (pending / vector / missing metadata) —
            // fall back to filling the full viewport rect.
            LayoutRect::from_origin_and_size(
                LayoutPoint::new(x, y),
                LayoutSize::new(vp_w, vp_h),
            )
        };

        let common = CommonItemProperties::new(
            bounds,
            SpaceAndClipInfo {
                spatial_id: ctx.spatial_id,
                clip_chain_id: ctx.clip_chain_id,
            },
        );

        if let Some(image_key) = self.image_key {
            // Image is loaded — draw the raster image, filling the fitted rect.
            ctx.wr.push_image(
                &common,
                bounds,
                ImageRendering::Auto,
                AlphaType::PremultipliedAlpha,
                image_key,
                ColorF::WHITE,
            );
            return;
        }

        // No image key yet (pending / failed / vector) — placeholder.
        let placeholder = ColorF::new(0.85, 0.85, 0.85, 0.4);
        ctx.wr.push_rect(&common, bounds, placeholder);

        // Diagonal X to indicate missing/broken image.
        let line_color = ColorF::new(0.6, 0.6, 0.6, 0.8);
        for (sx, sy, ex, _ey) in [
            (bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y),
            (bounds.max.x, bounds.min.y, bounds.min.x, bounds.max.y),
        ] {
            let lb = LayoutRect::from_origin_and_size(
                LayoutPoint::new(sx.min(ex), sy),
                LayoutSize::new((ex - sx).abs().max(1.0), 1.0),
            );
            let lc = CommonItemProperties::new(
                lb,
                SpaceAndClipInfo {
                    spatial_id: ctx.spatial_id,
                    clip_chain_id: ctx.clip_chain_id,
                },
            );
            ctx.wr.push_rect(&lc, lb, line_color);
        }
    }
}
