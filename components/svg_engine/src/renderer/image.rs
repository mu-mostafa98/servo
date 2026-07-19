/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<image>` element renderer.
//!
//! Currently renders a placeholder (gray rect with an X) to indicate the
//! image area.  A full implementation would:
//! 1. Fetch the image URL via the network/image cache
//! 2. Decode the image via `pixels::load_from_memory()`
//! 3. Push it as a WebRender image via the `ImageKey`

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ColorF, CommonItemProperties, SpaceAndClipInfo};

use crate::renderer::{Render, RenderContext};
use crate::image::SvgImage;

impl Render for SvgImage {
    fn render(&self, ctx: &mut RenderContext) {
        let x = ctx.svg_origin.x + self.x;
        let y = ctx.svg_origin.y + self.y;
        let w = self.width;
        let h = self.height;

        // Placeholder rect to indicate image area.
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x, y),
            LayoutSize::new(w, h),
        );
        let placeholder = ColorF::new(0.85, 0.85, 0.85, 0.4);
        let common = CommonItemProperties::new(
            bounds,
            SpaceAndClipInfo {
                spatial_id: ctx.spatial_id,
                clip_chain_id: ctx.clip_chain_id,
            },
        );
        ctx.wr.push_rect(&common, bounds, placeholder);

        // Diagonal X to indicate missing/broken image.
        let line_color = ColorF::new(0.6, 0.6, 0.6, 0.8);
        for (sx, sy, ex, _ey) in [(x, y, x + w, y + h), (x + w, y, x, y + h)] {
            let lb = LayoutRect::from_origin_and_size(
                LayoutPoint::new(sx.min(ex), sy),
                LayoutSize::new((ex - sx).abs().max(1.0), 1.0),
            );
            let lc = CommonItemProperties::new(lb, SpaceAndClipInfo {
                spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id,
            });
            ctx.wr.push_rect(&lc, lb, line_color);
        }
    }
}
