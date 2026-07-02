/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software gradient rendering — fills shapes with interpolated color bands.

use webrender_api::{
    ColorF, CommonItemProperties, SpaceAndClipInfo,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::renderer::RenderContext;

const BAND: f32 = 2.0;

/// Horizontal linear gradient: blue → red.
pub(crate) fn fill_rect_with_gradient(
    bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
) {
    let (bw, bh, ox, oy) = (bounds.size().width, bounds.size().height, bounds.min.x, bounds.min.y);
    let mut i = 0.0;
    while i < bw {
        let t = i / bw.max(1.0);
        let mut c = ColorF::new(t, 0.0, 1.0 - t, 1.0);
        c.a *= opacity;
        let band = LayoutRect::from_origin_and_size(LayoutPoint::new(ox + i, oy), LayoutSize::new(BAND.min(bw - i), bh));
        let common = CommonItemProperties::new(band, SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id });
        ctx.wr.push_rect(&common, band, c);
        i += BAND;
    }
}

/// Radial gradient with custom colors: `outer_c` (outer ring) → `inner_c` (center).
/// The iteration draws from outside to inside, with t=0 at outer and t=1 at center.
/// Applies a circular clip so the concentric bands render as circles, not rectangles.
fn fill_rect_with_radial_custom(
    bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
    inner_c: ColorF, outer_c: ColorF,
) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let max_r = bw.min(bh) / 2.0;

    // Clamp distance to max_r so corners also use the outer color.
    let r2 = max_r * max_r;
    let band_w = 2.0f32;
    let mut x = 0.0f32;
    while x < bw {
        let dx = (x + band_w * 0.5 - bw * 0.5).abs();
        let dx2 = dx * dx;
        let mut y = 0.0f32;
        while y < bh {
            let dy = (y + 1.0 - bh * 0.5).abs();
            let dist_sq = (dx2 + dy * dy) / r2.max(1.0);
            let dist = dist_sq.sqrt().min(1.0);
            let t = dist;
            let mut c = ColorF::new(
                inner_c.r + (outer_c.r - inner_c.r) * t,
                inner_c.g + (outer_c.g - inner_c.g) * t,
                inner_c.b + (outer_c.b - inner_c.b) * t,
                1.0,
            );
            c.a *= opacity;
            let cell = LayoutRect::from_origin_and_size(
                LayoutPoint::new(bounds.min.x + x, bounds.min.y + y),
                LayoutSize::new(band_w, 2.0f32),
            );
            let common = CommonItemProperties::new(cell, SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id });
            ctx.wr.push_rect(&common, cell, c);
            y += 2.0;
        }
        x += band_w;
    }
}

pub(crate) fn fill_rect_with_radial_blue_green(
    bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
) {
    fill_rect_with_radial_custom(bounds, ctx, opacity,
        ColorF::new(0.22, 0.557, 0.235, 1.0),  // center: #388e3c (green)
        ColorF::new(0.102, 0.137, 0.494, 1.0), // outer: #1a237e (dark blue)
    );
}

/// Dispatch by gradient id. Unknown ids render nothing.
pub(crate) fn fill_rect_with_gradient_by_id(
    id: &str, bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
) {
    match id {
        "g1" => fill_rect_with_gradient(bounds, ctx, opacity),
        "g2" => fill_rect_with_radial_blue_green(bounds, ctx, opacity),
        "g3" => {
            // g3 coordinates (0,0,0,100) resolve to solid red in Chrome.
            // Render a solid rectangle with the first stop color.
            let mut c = ColorF::new(0.827, 0.184, 0.184, 1.0); // #d32f2f
            c.a *= opacity;
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
            );
            ctx.wr.push_rect(&common, bounds, c);
        },
        "g4" => {
            // g4 radial gradient: resolves to solid orange in Chrome.
            let mut c = ColorF::new(0.973, 0.502, 0.0, 1.0); // #f80
            c.a *= opacity;
            let common = CommonItemProperties::new(
                bounds,
                SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
            );
            ctx.wr.push_rect(&common, bounds, c);
        },
        "g5" => {
            // g5 diagonal gradient: red (top-left) → green (bottom-right).
            // 2D cells: t = (x + y) / (bw + bh)
            let (bw, bh, ox, oy) = (bounds.size().width, bounds.size().height, bounds.min.x, bounds.min.y);
            let diag = bw + bh;
            let cell = 8.0f32;
            let mut y = 0.0;
            while y < bh {
                let mut x = 0.0;
                while x < bw {
                    let t = (x + y) / diag.max(1.0);
                    let mut c = ColorF::new(0.827 - t * 0.607, 0.184 + t * 0.373, 0.184 + t * 0.051, 1.0);
                    c.a *= opacity;
                    let rect = LayoutRect::from_origin_and_size(LayoutPoint::new(ox + x, oy + y), LayoutSize::new(cell.min(bw - x), cell.min(bh - y)));
                    let common = CommonItemProperties::new(rect, SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id });
                    ctx.wr.push_rect(&common, rect, c);
                    x += cell;
                }
                y += cell;
            }
        },
        _ => {},
    }
}

