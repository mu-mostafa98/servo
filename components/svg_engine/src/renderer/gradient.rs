/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software gradient rendering — fills shapes with interpolated color bands.
//!
//! Uses the **Strategy** pattern: a shared loop structure ([`render_gradient`])
//! delegates the per-pixel `t` computation to a [`GradientStrategy`] impl.
//! This eliminates the duplicated while-while loops between linear and radial
//! gradients.  Adding a new gradient type (e.g. conic, mesh) requires only a
//! new strategy — no structural code changes.

use webrender_api::{
    ColorF, CommonItemProperties, SpaceAndClipInfo,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::renderer::{RenderContext, shape_rendering_value, ZERO_LENGTH_EPSILON};
use crate::renderer::to_colorf;
use crate::style::gradient::{GradientDef, GradientStop, GradientUnits};

// ======================= Shared color math =======================

/// Linearly interpolate between two colors.
pub(crate) fn lerp_color(a: &ColorF, b: &ColorF, t: f32) -> ColorF {
    ColorF::new(
        a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t,
    )
}

/// Evaluate the color at position `t` (0.0–1.0) along the gradient stop list.
pub(crate) fn color_at_t(stops: &[GradientStop], t: f32) -> ColorF {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() { return ColorF::new(0.0, 0.0, 0.0, 1.0); }
    if stops.len() == 1 || t <= stops[0].offset { return to_colorf(&stops[0].color); }
    if t >= stops[stops.len() - 1].offset { return to_colorf(&stops[stops.len() - 1].color); }
    for i in 1..stops.len() {
        if t < stops[i].offset {
            let range = stops[i].offset - stops[i - 1].offset;
            let local_t = if range > 0.0 { (t - stops[i - 1].offset) / range } else { 0.0 };
            return lerp_color(&to_colorf(&stops[i - 1].color), &to_colorf(&stops[i].color), local_t);
        }
    }
    to_colorf(&stops[stops.len() - 1].color)
}

/// Project `(x, y)` onto a gradient line from `(gx1, gy1)` to `(gx2, gy2)`
/// and return the parametric position `t` in 0..1. Returns 0.0 for zero-length lines.
pub(crate) fn gradient_projection(x: f32, y: f32, gx1: f32, gy1: f32, gx2: f32, gy2: f32) -> f32 {
    let dx = gx2 - gx1;
    let dy = gy2 - gy1;
    let len_sq = dx * dx + dy * dy;
    if len_sq > ZERO_LENGTH_EPSILON { ((x - gx1) * dx + (y - gy1) * dy) / len_sq } else { 0.0 }
}

// ======================= Strategy trait =======================

/// Strategy for computing the parametric position `t` at a pixel `(x, y)`
/// during gradient rendering.
trait GradientStrategy {
    /// The gradient stops for color interpolation.
    fn stops(&self) -> &[GradientStop];
    /// Cell size for this gradient type, influenced by shape-rendering hints.
    fn cell_size(&self, ctx: &RenderContext) -> f32;
    /// Compute the parametric position `t` in 0..1 at pixel `(x, y)` within
    /// the bounding box `(bw, bh)`.
    fn compute_t(&self, x: f32, y: f32, bw: f32, bh: f32) -> f32;
}

// ======================= Shared render loop =======================

/// Fill `bounds` with a gradient using the given strategy.
fn render_gradient(
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
    strategy: &dyn GradientStrategy,
) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let cell = strategy.cell_size(ctx);
    let stops = strategy.stops();

    let mut y = 0.0;
    while y < bh {
        let mut x = 0.0;
        while x < bw {
            let t = strategy.compute_t(x, y, bw, bh);
            let mut c = color_at_t(stops, t);
            c.a *= opacity;
            let cw = cell.min(bw - x);
            let ch = cell.min(bh - y);
            draw_cell(&bounds, x, y, cw, ch, c, ctx);
            x += cell;
        }
        y += cell;
    }
}

// ======================= Linear strategy =======================

struct LinearStrategy<'a> {
    stops: &'a [GradientStop],
    gx1: f32,
    gy1: f32,
    gx2: f32,
    gy2: f32,
    /// Offset to add to (x, y) pixel positions before computing gradient
    /// projection.  This converts render_gradient's relative coordinates
    /// into the absolute coordinate space that the gradient line uses.
    offset_x: f32,
    offset_y: f32,
}

impl GradientStrategy for LinearStrategy<'_> {
    fn stops(&self) -> &[GradientStop] {
        self.stops
    }

    fn cell_size(&self, ctx: &RenderContext) -> f32 {
        shape_rendering_value(ctx, 2.0, 8.0, 4.0)
    }

    fn compute_t(&self, x: f32, y: f32, _bw: f32, _bh: f32) -> f32 {
        gradient_projection(
            x + self.offset_x, y + self.offset_y,
            self.gx1, self.gy1, self.gx2, self.gy2,
        )
    }
}

// ======================= Radial strategy =======================

struct RadialStrategy<'a> {
    stops: &'a [GradientStop],
    fx: f32,
    fy: f32,
    r2: f32, // radius squared
    /// Offset to add to (x, y) pixel positions before computing distance
    /// from the focal point.  Converts relative coords to absolute.
    offset_x: f32,
    offset_y: f32,
}

impl GradientStrategy for RadialStrategy<'_> {
    fn stops(&self) -> &[GradientStop] {
        self.stops
    }

    fn cell_size(&self, ctx: &RenderContext) -> f32 {
        shape_rendering_value(ctx, 1.0, 4.0, 2.0)
    }

    fn compute_t(&self, x: f32, y: f32, _bw: f32, _bh: f32) -> f32 {
        let dx = (x + self.offset_x) - self.fx;
        let dy = (y + self.offset_y) - self.fy;
        let dist_sq = (dx * dx + dy * dy) / self.r2.max(1.0);
        dist_sq.sqrt().min(1.0)
    }
}

// ======================= Public API =======================

/// Fill `bounds` with the gradient identified by `id`.
///
/// Looks up the gradient definition via the paint resource provider.
pub(crate) fn fill_rect_with_gradient_by_id(
    id: &str, bounds: LayoutRect, ctx: &mut RenderContext, opacity: f32,
) {
    let def = match ctx.paints.gradient(id) {
        Some(d) => d,
        None => {
            log::warn!("SVG gradient \"{}\" not found in definitions", id);
            return;
        },
    };

    match def {
        GradientDef::Linear(lg) => render_linear(lg, bounds, ctx, opacity),
        GradientDef::Radial(rg) => render_radial(rg, bounds, ctx, opacity),
    }
}

/// Render a linear gradient.
fn render_linear(
    lg: &crate::style::gradient::LinearGradient,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let bx = bounds.min.x;
    let by = bounds.min.y;

    // Convert all gradient coordinates to absolute space and set
    // offset = (bx, by) so that pixel positions from render_gradient
    // are also interpreted as absolute.  This makes userSpaceOnUse
    // correct regardless of the shape's position (bug #4 fix).
    let (gx1, gy1, gx2, gy2) = match lg.units {
        GradientUnits::ObjectBoundingBox => (
            bx + lg.x1.to_object_bbox() * bw,
            by + lg.y1.to_object_bbox() * bh,
            bx + lg.x2.to_object_bbox() * bw,
            by + lg.y2.to_object_bbox() * bh,
        ),
        GradientUnits::UserSpaceOnUse => (
            ctx.svg_origin.x + lg.x1.to_user_space(bw),
            ctx.svg_origin.y + lg.y1.to_user_space(bh),
            ctx.svg_origin.x + lg.x2.to_user_space(bw),
            ctx.svg_origin.y + lg.y2.to_user_space(bh),
        ),
    };

    let strategy = LinearStrategy {
        stops: &lg.stops,
        gx1, gy1, gx2, gy2,
        offset_x: bx,
        offset_y: by,
    };
    render_gradient(bounds, ctx, opacity, &strategy);
}

/// Render a radial gradient.
fn render_radial(
    rg: &crate::style::gradient::RadialGradient,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let bx = bounds.min.x;
    let by = bounds.min.y;

    // Convert all coordinates to absolute space with (bx, by) offset
    // for correct userSpaceOnUse behavior (bug #4 fix).
    let (fx, fy, radius) = match rg.units {
        GradientUnits::ObjectBoundingBox => (
            bx + rg.fx.to_object_bbox() * bw,
            by + rg.fy.to_object_bbox() * bh,
            rg.r.to_object_bbox() * bw.max(bh),
        ),
        GradientUnits::UserSpaceOnUse => (
            ctx.svg_origin.x + rg.fx.to_user_space(bw),
            ctx.svg_origin.y + rg.fy.to_user_space(bh),
            rg.r.to_user_space(bw.max(bh)),
        ),
    };

    if radius <= 0.0 {
        return;
    }

    let strategy = RadialStrategy {
        stops: &rg.stops,
        fx, fy,
        r2: radius * radius,
        offset_x: bx,
        offset_y: by,
    };
    render_gradient(bounds, ctx, opacity, &strategy);
}

// ======================= Helpers =======================

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
