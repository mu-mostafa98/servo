/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ColorF, CommonItemProperties, SpaceAndClipInfo};

use crate::renderer::{RenderContext, ZERO_LENGTH_EPSILON, shape_rendering_value, to_colorf};
use crate::style::gradient::{GradientDef, GradientStop, GradientUnits, SpreadMethod};
use crate::style::hints::{ColorInterpolation, ColorRendering};
use crate::style::transform_ops::TransformOp;

pub(crate) fn lerp_color(a: &ColorF, b: &ColorF, t: f32) -> ColorF {
    ColorF::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub(crate) fn lerp_color_linear(a: &ColorF, b: &ColorF, t: f32) -> ColorF {
    let ar = srgb_to_linear(a.r);
    let ag = srgb_to_linear(a.g);
    let ab = srgb_to_linear(a.b);
    let br = srgb_to_linear(b.r);
    let bg = srgb_to_linear(b.g);
    let bb = srgb_to_linear(b.b);
    ColorF::new(
        linear_to_srgb(ar + (br - ar) * t),
        linear_to_srgb(ag + (bg - ag) * t),
        linear_to_srgb(ab + (bb - ab) * t),
        a.a + (b.a - a.a) * t,
    )
}

pub(crate) fn color_at_t(stops: &[GradientStop], t: f32) -> ColorF {
    color_at_t_with_space(stops, t, ColorInterpolation::Srgb)
}

fn normalize_t(t: f32, spread: SpreadMethod) -> f32 {
    match spread {
        SpreadMethod::Pad => t.clamp(0.0, 1.0),
        SpreadMethod::Reflect => {
            let t = t.abs();
            let cycle = t as i32;
            if cycle % 2 == 0 {
                t.fract()
            } else {
                1.0 - t.fract()
            }
        },
        SpreadMethod::Repeat => {
            let t = t % 1.0;
            if t < 0.0 { t + 1.0 } else { t }
        },
    }
}

pub fn color_at_t_with_space(stops: &[GradientStop], t: f32, space: ColorInterpolation) -> ColorF {
    color_at_t_with_spread(stops, t, space, SpreadMethod::Pad)
}

pub(crate) fn color_at_t_with_spread(
    stops: &[GradientStop],
    t: f32,
    space: ColorInterpolation,
    spread: SpreadMethod,
) -> ColorF {
    let t = normalize_t(t, spread);
    if stops.is_empty() {
        return ColorF::new(0.0, 0.0, 0.0, 1.0);
    }
    if stops.len() == 1 || t <= stops[0].offset {
        return to_colorf(&stops[0].color);
    }
    if t >= stops[stops.len() - 1].offset {
        return to_colorf(&stops[stops.len() - 1].color);
    }
    let lerp: fn(&ColorF, &ColorF, f32) -> ColorF = match space {
        ColorInterpolation::Srgb | ColorInterpolation::Auto => lerp_color,
        ColorInterpolation::LinearRGB => lerp_color_linear,
    };
    for i in 1..stops.len() {
        if t < stops[i].offset {
            let range = stops[i].offset - stops[i - 1].offset;
            let local_t = if range > 0.0 {
                (t - stops[i - 1].offset) / range
            } else {
                0.0
            };
            return lerp(
                &to_colorf(&stops[i - 1].color),
                &to_colorf(&stops[i].color),
                local_t,
            );
        }
    }
    to_colorf(&stops[stops.len() - 1].color)
}

pub(crate) fn gradient_projection(x: f32, y: f32, gx1: f32, gy1: f32, gx2: f32, gy2: f32) -> f32 {
    let dx = gx2 - gx1;
    let dy = gy2 - gy1;
    let len_sq = dx * dx + dy * dy;
    if len_sq > ZERO_LENGTH_EPSILON {
        ((x - gx1) * dx + (y - gy1) * dy) / len_sq
    } else {
        0.0
    }
}

trait GradientStrategy {
    fn stops(&self) -> &[GradientStop];
    fn cell_size(&self, ctx: &RenderContext) -> f32;
    fn compute_t(&self, x: f32, y: f32, bw: f32, bh: f32) -> f32;
}

fn render_gradient(
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
    strategy: &dyn GradientStrategy,
    color_space: ColorInterpolation,
    spread: SpreadMethod,
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
            let mut c = color_at_t_with_spread(stops, t, color_space, spread);
            c.a *= opacity;
            let cw = cell.min(bw - x);
            let ch = cell.min(bh - y);
            draw_cell(&bounds, x, y, cw, ch, c, ctx);
            x += cell;
        }
        y += cell;
    }
}

fn apply_color_rendering_scale(base: f32, ctx: &RenderContext) -> f32 {
    match ctx
        .style
        .render_hints
        .as_ref()
        .and_then(|h| h.color_rendering)
    {
        Some(ColorRendering::OptimizeSpeed) => base * 2.0,
        Some(ColorRendering::OptimizeQuality) => base * 0.5,
        _ => base,
    }
}

struct LinearStrategy<'a> {
    stops: &'a [GradientStop],
    gx1: f32,
    gy1: f32,
    gx2: f32,
    gy2: f32,
    offset_x: f32,
    offset_y: f32,
}

impl GradientStrategy for LinearStrategy<'_> {
    fn stops(&self) -> &[GradientStop] {
        self.stops
    }

    fn cell_size(&self, ctx: &RenderContext) -> f32 {
        let base = shape_rendering_value(ctx, 2.0, 8.0, 4.0);
        apply_color_rendering_scale(base, ctx)
    }

    fn compute_t(&self, x: f32, y: f32, _bw: f32, _bh: f32) -> f32 {
        gradient_projection(
            x + self.offset_x,
            y + self.offset_y,
            self.gx1,
            self.gy1,
            self.gx2,
            self.gy2,
        )
    }
}

struct RadialStrategy<'a> {
    stops: &'a [GradientStop],
    fx: f32,
    fy: f32,
    r2: f32,
    offset_x: f32,
    offset_y: f32,
}

impl GradientStrategy for RadialStrategy<'_> {
    fn stops(&self) -> &[GradientStop] {
        self.stops
    }

    fn cell_size(&self, ctx: &RenderContext) -> f32 {
        let base = shape_rendering_value(ctx, 1.0, 4.0, 2.0);
        apply_color_rendering_scale(base, ctx)
    }

    fn compute_t(&self, x: f32, y: f32, _bw: f32, _bh: f32) -> f32 {
        let dx = (x + self.offset_x) - self.fx;
        let dy = (y + self.offset_y) - self.fy;
        let dist_sq = (dx * dx + dy * dy) / self.r2.max(1.0);
        dist_sq.sqrt().min(1.0)
    }
}

fn apply_grad_transform(gx: &mut f32, gy: &mut f32, ops: &[TransformOp]) {
    use euclid::Transform2D;
    let mut m = Transform2D::<f32, (), ()>::identity();
    for op in ops {
        match op {
            TransformOp::Translate(tx, ty) => {
                m = m.then(&Transform2D::translation(*tx, *ty));
            },
            TransformOp::Scale(sx, sy) => {
                m = m.then(&Transform2D::scale(*sx, *sy));
            },
            TransformOp::Rotate(a, cx, cy) => {
                let rad = a.to_radians();
                let (s, c) = rad.sin_cos();
                let r: Transform2D<f32, (), ()> = Transform2D::new(c, s, -s, c, 0.0, 0.0);
                m = m
                    .then(&Transform2D::translation(-*cx, -*cy))
                    .then(&r)
                    .then(&Transform2D::translation(*cx, *cy));
            },
            TransformOp::SkewX(a) => {
                let rad = a.to_radians();
                m = m.then(&Transform2D::new(1.0, 0.0, rad.tan(), 1.0, 0.0, 0.0));
            },
            TransformOp::SkewY(a) => {
                let rad = a.to_radians();
                m = m.then(&Transform2D::new(1.0, rad.tan(), 0.0, 1.0, 0.0, 0.0));
            },
            TransformOp::Matrix(v) => {
                m = m.then(&Transform2D::new(v[0], v[1], v[2], v[3], v[4], v[5]));
            },
        }
    }
    let p = m.transform_point(euclid::Point2D::new(*gx, *gy));
    *gx = p.x;
    *gy = p.y;
}

pub(crate) fn fill_rect_with_gradient_by_id(
    id: &str,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    let def = match ctx.paints.gradient(id) {
        Some(d) => d,
        None => {
            log::warn!("SVG gradient \"{} not found in definitions", id);
            return;
        },
    };
    match def {
        GradientDef::Linear(lg) => render_linear(lg, bounds, ctx, opacity),
        GradientDef::Radial(rg) => render_radial(rg, bounds, ctx, opacity),
    }
}

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

    let (gx1, gy1, gx2, gy2) = match lg.units {
        GradientUnits::ObjectBoundingBox => {
            let mut x1 = lg.x1.to_object_bbox();
            let mut y1 = lg.y1.to_object_bbox();
            let mut x2 = lg.x2.to_object_bbox();
            let mut y2 = lg.y2.to_object_bbox();
            for op in &lg.transform {
                apply_grad_transform(&mut x1, &mut y1, std::slice::from_ref(op));
                apply_grad_transform(&mut x2, &mut y2, std::slice::from_ref(op));
            }
            (bx + x1 * bw, by + y1 * bh, bx + x2 * bw, by + y2 * bh)
        },
        GradientUnits::UserSpaceOnUse => {
            let mut x1 = lg.x1.to_user_space(bw);
            let mut y1 = lg.y1.to_user_space(bh);
            let mut x2 = lg.x2.to_user_space(bw);
            let mut y2 = lg.y2.to_user_space(bh);
            for op in &lg.transform {
                apply_grad_transform(&mut x1, &mut y1, std::slice::from_ref(op));
                apply_grad_transform(&mut x2, &mut y2, std::slice::from_ref(op));
            }
            (
                ctx.svg_origin.x + x1,
                ctx.svg_origin.y + y1,
                ctx.svg_origin.x + x2,
                ctx.svg_origin.y + y2,
            )
        },
    };

    let strategy = LinearStrategy {
        stops: &lg.stops,
        gx1,
        gy1,
        gx2,
        gy2,
        offset_x: bx,
        offset_y: by,
    };
    let space = ctx
        .style
        .render_hints
        .as_ref()
        .and_then(|h| h.color_interpolation)
        .unwrap_or(ColorInterpolation::Srgb);
    render_gradient(bounds, ctx, opacity, &strategy, space, lg.spread_method);
}

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
        fx,
        fy,
        r2: radius * radius,
        offset_x: bx,
        offset_y: by,
    };
    let space = ctx
        .style
        .render_hints
        .as_ref()
        .and_then(|h| h.color_interpolation)
        .unwrap_or(ColorInterpolation::Srgb);
    render_gradient(bounds, ctx, opacity, &strategy, space, rg.spread_method);
}

fn draw_cell(
    bounds: &LayoutRect,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: ColorF,
    ctx: &mut RenderContext,
) {
    let rect = LayoutRect::from_origin_and_size(
        LayoutPoint::new(bounds.min.x + x, bounds.min.y + y),
        LayoutSize::new(w, h),
    );
    let common = CommonItemProperties::new(
        rect,
        SpaceAndClipInfo {
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
        },
    );
    ctx.wr.push_rect(&common, rect, color);
}
