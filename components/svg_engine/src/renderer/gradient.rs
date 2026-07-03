/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software gradient rendering — fills shapes with interpolated color bands.
//!
//! Reads gradient definitions from [`RenderContext::gradients`] and renders
//! them by dividing the fill area into small cells, each drawn with a
//! single [`push_rect`] call at an interpolated color.

use webrender_api::{
    ColorF, CommonItemProperties, SpaceAndClipInfo,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::renderer::RenderContext;
use crate::style::gradient::{GradientDef, GradientStop};

const CELL_SIZE: f32 = 4.0;
const CELL_SIZE_RADIAL: f32 = 2.0;

/// Linearly interpolate between two colors.
fn lerp_color(a: &ColorF, b: &ColorF, t: f32) -> ColorF {
    ColorF::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Evaluate the color at position `t` (0.0–1.0) along the gradient stop list.
fn color_at_t(stops: &[GradientStop], t: f32) -> ColorF {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return ColorF::new(0.0, 0.0, 0.0, 1.0);
    }
    if stops.len() == 1 || t <= stops[0].offset {
        return stops[0].color;
    }
    if t >= stops[stops.len() - 1].offset {
        return stops[stops.len() - 1].color;
    }
    for i in 1..stops.len() {
        if t < stops[i].offset {
            let range = stops[i].offset - stops[i - 1].offset;
            let local_t = if range > 0.0 { (t - stops[i - 1].offset) / range } else { 0.0 };
            return lerp_color(&stops[i - 1].color, &stops[i].color, local_t);
        }
    }
    stops[stops.len() - 1].color
}

/// Draw a single cell at (x,y) with the given color.
fn draw_cell(bounds: &LayoutRect, x: f32, y: f32, w: f32, h: f32, color: ColorF, ctx: &mut RenderContext) {
    let rect = LayoutRect::from_origin_and_size(
        LayoutPoint::new(bounds.min.x + x, bounds.min.y + y),
        LayoutSize::new(w, h),
    );
    let common = CommonItemProperties::new(
        rect,
        SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
    );
    ctx.wr.push_rect(&common, rect, color);
}

/// Fill bounds with a linear gradient defined by `lg`.
fn render_linear(lg: &crate::style::gradient::LinearGradient, bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;

    // Determine gradient vector direction from the gradient coordinates.
    // For objectBoundingBox (default), x1/x2 are in the 0-1 range.
    // If coordinates look like percentages (0-100), normalize them.
    let gx1 = lg.x1;
    let gy1 = lg.y1;
    let gx2 = lg.x2;
    let gy2 = lg.y2;

    let dx = gx2 - gx1;
    let dy = gy2 - gy1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.001 {
        // Zero-length gradient — use first stop color.
        let c = color_at_t(&lg.stops, 0.0);
        let mut c2 = c;
        c2.a *= opacity;
        draw_cell(&bounds, 0.0, 0.0, bw, bh, c2, ctx);
        return;
    }

    let mut y = 0.0;
    while y < bh {
        let mut x = 0.0;
        while x < bw {
            // Project (x,y) onto the gradient vector to get t.
            let px = x / bw.max(1.0);
            let py = y / bh.max(1.0);
            // Dot product of (px - gx1, py - gy1) with normalized gradient direction
            let t = ((px - gx1) * dx + (py - gy1) * dy) / len_sq;
            let mut c = color_at_t(&lg.stops, t);
            c.a *= opacity;
            let cw = CELL_SIZE.min(bw - x);
            let ch = CELL_SIZE.min(bh - y);
            draw_cell(&bounds, x, y, cw, ch, c, ctx);
            x += CELL_SIZE;
        }
        y += CELL_SIZE;
    }
}

/// Fill bounds with a radial gradient defined by `rg`.
fn render_radial(rg: &crate::style::gradient::RadialGradient, bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;

    // Normalize coordinates (0-100 → 0-1 if they look like percentages)
    // Radial gradients use cx/cy/r in the objectBoundingBox coordinate system.
    let cx_norm = rg.cx / 100.0;
    let cy_norm = rg.cy / 100.0;
    let r_norm = rg.r / 100.0;

    let cx = cx_norm * bw;
    let cy = cy_norm * bh;
    let max_r = r_norm * bw.max(bh);
    let r2 = max_r * max_r;

    if max_r <= 0.0 {
        return;
    }

    let cell = CELL_SIZE_RADIAL;
    let mut y = 0.0;
    while y < bh {
        let mut x = 0.0;
        while x < bw {
            let dx = x - cx;
            let dy = y - cy;
            let dist_sq = (dx * dx + dy * dy) / r2.max(1.0);
            let dist = dist_sq.sqrt().min(1.0);
            let t = dist; // 0 at center, 1 at edge
            let mut c = color_at_t(&rg.stops, t);
            c.a *= opacity;
            let cw = cell.min(bw - x);
            let ch = cell.min(bh - y);
            draw_cell(&bounds, x, y, cw, ch, c, ctx);
            x += cell;
        }
        y += cell;
    }
}

/// Main entry point — fills `bounds` with the gradient identified by `id`.
/// Looks up the gradient definition from `ctx.gradients`.
pub(crate) fn fill_rect_with_gradient_by_id(
    id: &str, bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
) {
    let def = match ctx.gradients.get(id) {
        Some(d) => d,
        None => {
            // Gradient ID not found in definitions — render nothing.
            return;
        },
    };

    match def {
        GradientDef::Linear(lg) => render_linear(lg, bounds, ctx, opacity),
        GradientDef::Radial(rg) => render_radial(rg, bounds, ctx, opacity),
    }
}
