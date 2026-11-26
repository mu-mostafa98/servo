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

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ColorF, CommonItemProperties, SpaceAndClipInfo};

use crate::model::document::{GradientDef, GradientStop, GradientUnits, SpreadMethod};
use crate::model::style::{ColorInterpolation, ColorRendering};
use crate::model::transform::TransformOp;
use crate::render::renderer::{
    RenderContext, ZERO_LENGTH_EPSILON, color_interpolation, shape_rendering_value, to_colorf,
};

// ======================= Shared color math =======================

/// Linearly interpolate between two colors in sRGB space.
pub(crate) fn lerp_color(a: &ColorF, b: &ColorF, t: f32) -> ColorF {
    ColorF::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Convert a single sRGB channel to linear light.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert a linear light channel back to sRGB.
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Linearly interpolate between two colors in linear RGB space.
///
/// Per the SVG 2 spec, `color-interpolation: linearRGB` uses linear light
/// color interpolation, which produces perceptually smoother gradients.
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

/// Number of sRGB samples generated for each stop interval when approximating
/// linear-light interpolation with a backend that can only interpolate in sRGB.
/// 128 keeps the approximation well within one 8-bit step everywhere.
const LINEAR_RGB_SUBDIVISIONS: usize = 128;

/// Expand a gradient's stops so that interpolating them in sRGB reproduces
/// linear-light (`linearRGB`) interpolation.
///
/// WebRender — and vello_cpu's color-ramp encoding — only interpolate gradient
/// stops in sRGB. For [`ColorInterpolation::LinearRGB`] we therefore sample the
/// true linear-light ramp at a fixed resolution here; interpolating those
/// samples in sRGB then stays within a fraction of an 8-bit step of the exact
/// result. For sRGB interpolation the stops are returned unchanged.
pub(crate) fn stops_for_srgb_interpolation(
    stops: &[GradientStop],
    color_interpolation: ColorInterpolation,
) -> Vec<(f32, ColorF)> {
    if !matches!(color_interpolation, ColorInterpolation::LinearRGB) {
        return stops
            .iter()
            .map(|s| (s.offset, to_colorf(&s.color)))
            .collect();
    }

    let mut out = Vec::with_capacity(stops.len().saturating_mul(LINEAR_RGB_SUBDIVISIONS + 1));
    let Some(first) = stops.first() else {
        return out;
    };
    out.push((first.offset, to_colorf(&first.color)));
    for pair in stops.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let ca = to_colorf(&a.color);
        let cb = to_colorf(&b.color);
        let range = b.offset - a.offset;
        if range <= 0.0 {
            out.push((b.offset, cb));
            continue;
        }
        for step in 1..=LINEAR_RGB_SUBDIVISIONS {
            let t = step as f32 / LINEAR_RGB_SUBDIVISIONS as f32;
            out.push((a.offset + range * t, lerp_color_linear(&ca, &cb, t)));
        }
    }
    out
}

/// Evaluate the color at position `t` (0.0–1.0) along the gradient stop list.
///
/// Uses sRGB interpolation by default, or linear RGB when `color_interpolation`
/// is [`ColorInterpolation::LinearRGB`].
pub(crate) fn color_at_t(stops: &[GradientStop], t: f32) -> ColorF {
    color_at_t_with_space(stops, t, ColorInterpolation::Srgb)
}

/// Normalize `t` according to the spread method.
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

/// Evaluate the color at position `t` using the specified interpolation space.
pub fn color_at_t_with_space(stops: &[GradientStop], t: f32, space: ColorInterpolation) -> ColorF {
    color_at_t_with_spread(stops, t, space, SpreadMethod::Pad)
}

/// Evaluate the color at position `t` using the specified interpolation and spread.
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

/// Project `(x, y)` onto a gradient line from `(gx1, gy1)` to `(gx2, gy2)`
/// and return the parametric position `t` in 0..1. Returns 0.0 for zero-length lines.
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

/// The SVG "normalized diagonal" of a reference box: `sqrt((w² + h²) / 2)`.
///
/// Radial-gradient percentages on `r` (and `fr`) resolve against this value,
/// *not* the width, height, or their max (§13.2.2). For a square box it equals
/// the side length; for a non-square box it is the geometric mean of the axes,
/// so `r="50%"` yields a circle rather than an ellipse.
pub(crate) fn normalized_diagonal(w: f32, h: f32) -> f32 {
    ((w * w + h * h) / 2.0).sqrt()
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

/// Adjust gradient cell size based on the `color-rendering` hint.
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

// ======================= Linear strategy =======================

struct LinearStrategy<'a> {
    stops: &'a [GradientStop],
    gx1: f32,
    gy1: f32,
    gx2: f32,
    gy2: f32,
    /// Offset to add to (x, y) pixel positions before computing gradient
    /// projection. This converts render_gradient's area-relative coordinates
    /// into absolute layout coordinates.
    offset_x: f32,
    offset_y: f32,
    /// Normalization applied to the absolute point before projection: the
    /// point is mapped to `((px - norm_origin_x) * inv_scale_x,
    /// (py - norm_origin_y) * inv_scale_y)`. For `objectBoundingBox` this maps
    /// the point into the reference box's normalized unit-square space (the
    /// space the gradient fractions live in, matching Chrome); for
    /// `userSpaceOnUse` it is the identity (origin 0, scale 1), so the point
    /// is projected directly in absolute space.
    norm_origin_x: f32,
    norm_origin_y: f32,
    inv_scale_x: f32,
    inv_scale_y: f32,
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
        let px = (x + self.offset_x - self.norm_origin_x) * self.inv_scale_x;
        let py = (y + self.offset_y - self.norm_origin_y) * self.inv_scale_y;
        gradient_projection(px, py, self.gx1, self.gy1, self.gx2, self.gy2)
    }
}

// ======================= Radial strategy =======================

struct RadialStrategy<'a> {
    stops: &'a [GradientStop],
    fx: f32,
    fy: f32,
    /// Outer ellipse radii (absolute) of the end ellipse.
    rx: f32,
    ry: f32,
    /// Focal radius (`fr`): points within this distance of the focal point
    /// render the first stop color as a solid disk (§13.2.2).
    fr: f32,
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
        let base = shape_rendering_value(ctx, 1.0, 4.0, 2.0);
        apply_color_rendering_scale(base, ctx)
    }

    fn compute_t(&self, x: f32, y: f32, _bw: f32, _bh: f32) -> f32 {
        let dx = (x + self.offset_x) - self.fx;
        let dy = (y + self.offset_y) - self.fy;
        // Normalized elliptical distance: `1.0` on the outer ellipse, so the
        // non-uniform bounding-box scale of the conceptually-circular rings is
        // respected (§13.2.2).
        let d = ((dx / self.rx).powi(2) + (dy / self.ry).powi(2)).sqrt();
        // `fr` is a scalar radius; normalize it against the smaller axis (exact
        // for a circle, a close approximation for a non-square ellipse).
        let fr_norm = self.fr / self.rx.min(self.ry);
        if d <= fr_norm {
            0.0
        } else if fr_norm >= 1.0 {
            1.0
        } else {
            ((d - fr_norm) / (1.0 - fr_norm)).min(1.0)
        }
    }
}
// ======================= Public API =======================

/// Build the affine matrix for a gradient's `gradientTransform` op list.
/// SVG positive angle = CW rotation (y-down coords).
fn gradient_transform_matrix(ops: &[TransformOp]) -> euclid::Transform2D<f32, (), ()> {
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
                // CW in SVG y-down coords: [c, s; -s, c]
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
    m
}

/// Apply gradientTransform ops to a gradient point in the gradient's
/// own coordinate space.
fn apply_grad_transform(gx: &mut f32, gy: &mut f32, ops: &[TransformOp]) {
    let p = gradient_transform_matrix(ops).transform_point(euclid::Point2D::new(*gx, *gy));
    *gx = p.x;
    *gy = p.y;
}

pub(crate) fn fill_rect_with_gradient(
    def: &GradientDef,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    fill_area_with_gradient(def, bounds, bounds, ctx, opacity);
}

/// Fill `area` with a gradient, resolving the gradient's own geometry against
/// `geometry_bounds`.
///
/// The two differ when the painted area is not the shape's own bounding box —
/// a centered stroke paints a band that reaches half the stroke width beyond
/// the shape's outline, while the gradient's `objectBoundingBox` units still
/// resolve against the shape's bounding box.
pub(crate) fn fill_area_with_gradient(
    def: &GradientDef,
    area: LayoutRect,
    geometry_bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    match def {
        GradientDef::Linear(lg) => {
            // SVG 1.1 §13.2.2 / SVG 2: if the gradient vector is degenerate
            // (`x1 == x2` and `y1 == y2`), the area is painted as a single
            // color using the last stop's color and opacity — not the first
            // stop's, which is what projecting onto a zero-length line would
            // otherwise yield.
            let (start, end) = resolve_linear_geometry(lg, geometry_bounds, ctx);
            if (start.x - end.x).abs() < ZERO_LENGTH_EPSILON &&
                (start.y - end.y).abs() < ZERO_LENGTH_EPSILON
            {
                let mut color = lg
                    .stops
                    .last()
                    .map(|stop| to_colorf(&stop.color))
                    .unwrap_or(ColorF::new(0.0, 0.0, 0.0, 1.0));
                color.a *= opacity;
                draw_cell(&area, 0.0, 0.0, area.size().width, area.size().height, color, ctx);
                return;
            }
            if !push_linear_native(lg, area, geometry_bounds, ctx, opacity) {
                render_linear(lg, area, geometry_bounds, ctx, opacity);
            }
        },
        GradientDef::Radial(rg) => {
            if !push_radial_native(rg, area, geometry_bounds, ctx, opacity) {
                render_radial(rg, area, geometry_bounds, ctx, opacity);
            }
        },
    }
}

/// The effective `color-interpolation` for a gradient: the value authored on the
/// gradient element wins; `auto` (the default) defers to the referencing shape's
/// `color-interpolation` property, read from `ctx.style`.
pub(crate) fn resolve_color_interpolation(
    gradient_ci: ColorInterpolation,
    ctx: &RenderContext,
) -> ColorInterpolation {
    match gradient_ci {
        ColorInterpolation::Auto => color_interpolation(ctx),
        other => other,
    }
}

// ======================= Native gradient resolution =======================

/// Map an SVG spread method to a WebRender `ExtendMode`, or `None` for
/// `reflect` (which WebRender cannot express natively).
fn spread_to_extend(spread: SpreadMethod) -> Option<webrender_api::ExtendMode> {
    match spread {
        SpreadMethod::Pad => Some(webrender_api::ExtendMode::Clamp),
        SpreadMethod::Repeat => Some(webrender_api::ExtendMode::Repeat),
        SpreadMethod::Reflect => None,
    }
}

/// Pad a stop list out to the full `[0, 1]` gradient vector with the clamped
/// end colors.
///
/// WebRender's `GradientBuilder` normalizes stops so they span `[0, 1]` and then
/// shortens the gradient line to the original stop range. That is harmless for
/// `pad` (which clamps anyway), but it also shrinks the *repeat period*: SVG
/// repeats over the whole gradient vector, not just the span covered by the
/// stops. Padding the list out to offsets 0 and 1 keeps the gradient line at
/// full length, so WebRender's repeat tile matches the vector.
fn pad_stops_to_unit_range(mut stops: Vec<(f32, ColorF)>) -> Vec<(f32, ColorF)> {
    let Some(&(first_offset, first_color)) = stops.first() else {
        return stops;
    };
    if first_offset > 0.0 {
        stops.insert(0, (0.0, first_color));
    }
    let Some(&(last_offset, last_color)) = stops.last() else {
        return stops;
    };
    if last_offset < 1.0 {
        stops.push((1.0, last_color));
    }
    stops
}

/// Convert SVG gradient stops to WebRender stops, folding `opacity` into each
/// stop's alpha (native gradient items have no item-level opacity).
///
/// WebRender always interpolates in sRGB, so `linearRGB` gradients are converted
/// to an equivalent dense sRGB stop list (see [`stops_for_srgb_interpolation`]).
///
/// For `repeat`, the stops are padded to the full `[0, 1]` vector so WebRender
/// does not shrink the repeat period to the stop range (see
/// [`pad_stops_to_unit_range`]).
fn stops_to_webrender(
    stops: &[GradientStop],
    color_interpolation: ColorInterpolation,
    opacity: f32,
    spread: SpreadMethod,
) -> Vec<webrender_api::GradientStop> {
    let mut stops = stops_for_srgb_interpolation(stops, color_interpolation);
    if matches!(spread, SpreadMethod::Repeat) {
        stops = pad_stops_to_unit_range(stops);
    }
    stops
        .into_iter()
        .map(|(offset, mut color)| {
            color.a *= opacity;
            webrender_api::GradientStop { offset, color }
        })
        .collect()
}

/// Whether two [`crate::model::document::GradientLength`]s denote the same
/// position in gradient space. `Number` and `Percentage` are distinct units, so
/// a mixed pair conservatively reports inequality (fallback).
fn gradient_lengths_equal(
    a: crate::model::document::GradientLength,
    b: crate::model::document::GradientLength,
) -> bool {
    use crate::model::document::GradientLength;
    match (a, b) {
        (GradientLength::Number(x), GradientLength::Number(y)) => (x - y).abs() < 1e-6,
        (GradientLength::Percentage(x), GradientLength::Percentage(y)) => (x - y).abs() < 1e-6,
        _ => false,
    }
}

/// Resolve a linear gradient's `objectBoundingBox` endpoints in the normalized
/// unit-square space: the raw `x1/y1/x2/y2` fractions with `gradientTransform`
/// applied (in fraction space, before the bounding box is scaled onto them).
///
/// Chrome projects object-bounding-box linear gradients in this normalized
/// space rather than the actual (non-uniformly scaled) bounding box — a
/// non-uniform scale does not preserve perpendicular projection, so projecting
/// in the raw scaled box skews diagonal gradients.
pub(crate) fn object_bbox_linear_fractions(
    lg: &crate::model::document::LinearGradient,
) -> (f32, f32, f32, f32) {
    let mut x1 = lg.x1.to_object_bbox();
    let mut y1 = lg.y1.to_object_bbox();
    let mut x2 = lg.x2.to_object_bbox();
    let mut y2 = lg.y2.to_object_bbox();
    apply_grad_transform(&mut x1, &mut y1, &lg.transform);
    apply_grad_transform(&mut x2, &mut y2, &lg.transform);
    (x1, y1, x2, y2)
}

/// Compute absolute-space endpoints whose standard perpendicular projection
/// reproduces the *normalized* object-bounding-box projection (Chrome's
/// behavior). Used by backends (vello/peniko) that take absolute endpoints and
/// perform the projection themselves.
///
/// `x1..y2` are the fraction-space endpoints (see
/// [`object_bbox_linear_fractions`]); `(bx, by, bw, bh)` is the reference box
/// (the object bounding box, in the backend's coordinate space).
pub(crate) fn object_bbox_linear_endpoints(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
) -> (f32, f32, f32, f32) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    // Degenerate vector: fall back to the plain scaled endpoints; the caller
    // handles the degenerate gradient.
    if len_sq <= ZERO_LENGTH_EPSILON {
        return (bx + x1 * bw, by + y1 * bh, bx + x2 * bw, by + y2 * bh);
    }
    // The normalized-space projection is an affine function of the absolute
    // point: t(P) = A·P + C. Rewrite it as a standard projection
    // t(P) = (P - S)·D / |D|² with D = A/|A|² and S = -C·D.
    let ax = dx / (bw * len_sq);
    let ay = dy / (bh * len_sq);
    let c = -(bx * ax + by * ay) - (x1 * dx + y1 * dy) / len_sq;
    let a_len_sq = ax * ax + ay * ay;
    let ddx = ax / a_len_sq;
    let ddy = ay / a_len_sq;
    let sx = -c * ddx;
    let sy = -c * ddy;
    (sx, sy, sx + ddx, sy + ddy)
}

/// Resolve a linear gradient's endpoints to absolute layout coordinates,
/// applying `gradientTransform` in gradient coordinate space.
fn resolve_linear_geometry(
    lg: &crate::model::document::LinearGradient,
    bounds: LayoutRect,
    ctx: &RenderContext,
) -> (LayoutPoint, LayoutPoint) {
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
            apply_grad_transform(&mut x1, &mut y1, &lg.transform);
            apply_grad_transform(&mut x2, &mut y2, &lg.transform);
            (bx + x1 * bw, by + y1 * bh, bx + x2 * bw, by + y2 * bh)
        },
        GradientUnits::UserSpaceOnUse => {
            let mut x1 = lg.x1.to_user_space(bw);
            let mut y1 = lg.y1.to_user_space(bh);
            let mut x2 = lg.x2.to_user_space(bw);
            let mut y2 = lg.y2.to_user_space(bh);
            apply_grad_transform(&mut x1, &mut y1, &lg.transform);
            apply_grad_transform(&mut x2, &mut y2, &lg.transform);
            (
                ctx.svg_origin.x + x1,
                ctx.svg_origin.y + y1,
                ctx.svg_origin.x + x2,
                ctx.svg_origin.y + y2,
            )
        },
    };
    (LayoutPoint::new(gx1, gy1), LayoutPoint::new(gx2, gy2))
}

/// Resolve a radial gradient's center and (possibly elliptical) radius in
/// absolute layout coordinates, applying `gradientTransform`. Returns `None`
/// for a non-positive radius.
fn resolve_radial_center_radius(
    rg: &crate::model::document::RadialGradient,
    bounds: LayoutRect,
    ctx: &RenderContext,
) -> Option<(LayoutPoint, LayoutSize)> {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let bx = bounds.min.x;
    let by = bounds.min.y;

    // Center and radius in the gradient's own coordinate space, after applying
    // gradientTransform (which may further turn the ellipse into another ellipse).
    let m = gradient_transform_matrix(&rg.transform);
    // `r` is a fraction of the object bounding box in "object bounding box
    // space" (a unit square). The non-uniform scaling of that space to the
    // actual bounding box turns the conceptually-circular rings into ellipses
    // (§13.2.2): horizontal radii scale by `bw`, vertical radii by `bh`.
    let nd = normalized_diagonal(bw, bh);
    let (cx, cy, r) = match rg.units {
        GradientUnits::ObjectBoundingBox => (
            rg.cx.to_object_bbox(),
            rg.cy.to_object_bbox(),
            rg.r.to_object_bbox(),
        ),
        GradientUnits::UserSpaceOnUse => (
            rg.cx.to_user_space(bw),
            rg.cy.to_user_space(bh),
            rg.r.to_user_space(nd),
        ),
    };
    let center = m.transform_point(euclid::Point2D::new(cx, cy));
    let rx = m.transform_vector(euclid::Vector2D::new(r, 0.0)).length();
    let ry = m.transform_vector(euclid::Vector2D::new(0.0, r)).length();

    let (cx_abs, cy_abs, rx_abs, ry_abs) = match rg.units {
        GradientUnits::ObjectBoundingBox => {
            (bx + center.x * bw, by + center.y * bh, rx * bw, ry * bh)
        },
        GradientUnits::UserSpaceOnUse => (
            ctx.svg_origin.x + center.x,
            ctx.svg_origin.y + center.y,
            rx,
            ry,
        ),
    };

    if rx_abs <= 0.0 || ry_abs <= 0.0 {
        return None;
    }
    Some((
        LayoutPoint::new(cx_abs, cy_abs),
        LayoutSize::new(rx_abs, ry_abs),
    ))
}

/// Resolve a radial gradient's focal point (`fx`, `fy`) to absolute layout
/// coordinates, applying `gradientTransform` like [`resolve_radial_center_radius`].
fn resolve_radial_focal(
    rg: &crate::model::document::RadialGradient,
    bounds: LayoutRect,
    ctx: &RenderContext,
) -> LayoutPoint {
    let bw = bounds.size().width;
    let bh = bounds.size().height;
    let bx = bounds.min.x;
    let by = bounds.min.y;

    let m = gradient_transform_matrix(&rg.transform);
    let (fx, fy) = match rg.units {
        GradientUnits::ObjectBoundingBox => (rg.fx.to_object_bbox(), rg.fy.to_object_bbox()),
        GradientUnits::UserSpaceOnUse => (rg.fx.to_user_space(bw), rg.fy.to_user_space(bh)),
    };
    let p = m.transform_point(euclid::Point2D::new(fx, fy));
    match rg.units {
        GradientUnits::ObjectBoundingBox => LayoutPoint::new(bx + p.x * bw, by + p.y * bh),
        GradientUnits::UserSpaceOnUse => {
            LayoutPoint::new(ctx.svg_origin.x + p.x, ctx.svg_origin.y + p.y)
        },
    }
}

/// Resolve a gradient definition to a native WebRender gradient, or `None` when
/// it cannot be expressed natively (reflect spread, an offset-focal radial, or
/// a diagonal linear gradient).
/// `linearRGB` interpolation is supported by converting the stops to an
/// equivalent sRGB stop list (see [`stops_for_srgb_interpolation`]).
pub(crate) fn resolve_gradient(
    def: &GradientDef,
    bounds: LayoutRect,
    ctx: &RenderContext,
    opacity: f32,
) -> Option<crate::GradientKind> {
    let color_interpolation = resolve_color_interpolation(def.color_interpolation(), ctx);
    match def {
        GradientDef::Linear(lg) => {
            let extend_mode = spread_to_extend(lg.spread_method)?;
            let stops =
                stops_to_webrender(&lg.stops, color_interpolation, opacity, lg.spread_method);
            let (start, end) = resolve_linear_geometry(lg, bounds, ctx);
            // A non-axis-aligned gradient must be rasterized by vello: the native
            // WebRender path projects in the raw scaled bounding box, which skews
            // diagonals (non-uniform bbox scaling does not preserve perpendicular
            // projection). Returning `None` here routes the shape to the vello
            // raster fallback, which projects in the normalized
            // object-bounding-box space (see [`object_bbox_linear_fractions`]).
            let horizontal = (start.y - end.y).abs() < ZERO_LENGTH_EPSILON;
            let vertical = (start.x - end.x).abs() < ZERO_LENGTH_EPSILON;
            if !horizontal && !vertical {
                return None;
            }
            Some(crate::GradientKind::Linear {
                start,
                end,
                stops,
                extend_mode,
            })
        },
        GradientDef::Radial(rg) => {
            // WebRender radial gradients are concentric only (no focal offset),
            // and have no focal radius (`fr`), so fall back when either is set.
            if !gradient_lengths_equal(rg.fx, rg.cx) ||
                !gradient_lengths_equal(rg.fy, rg.cy) ||
                !rg.fr.is_zero()
            {
                return None;
            }
            let extend_mode = spread_to_extend(rg.spread_method)?;
            let stops =
                stops_to_webrender(&rg.stops, color_interpolation, opacity, rg.spread_method);
            let (center, radius) = resolve_radial_center_radius(rg, bounds, ctx)?;
            Some(crate::GradientKind::Radial {
                center,
                radius,
                stops,
                extend_mode,
            })
        },
    }
}

/// Push a native linear gradient, returning `false` when it can't be expressed
/// natively (caller falls back to the software renderer).
fn push_linear_native(
    lg: &crate::model::document::LinearGradient,
    area: LayoutRect,
    geometry_bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) -> bool {
    let Some(extend_mode) = spread_to_extend(lg.spread_method) else {
        return false;
    };
    let stops = stops_to_webrender(
        &lg.stops,
        resolve_color_interpolation(lg.color_interpolation, ctx),
        opacity,
        lg.spread_method,
    );
    let (start, end) = resolve_linear_geometry(lg, geometry_bounds, ctx);
    // Route non-axis-aligned (diagonal) linear gradients through the software
    // renderer, which projects in the *normalized* object-bounding-box space
    // (see [`object_bbox_linear_fractions`]) — the reference-correct behavior.
    // The native WebRender path projects in the raw scaled bounding box, which
    // skews diagonals (non-uniform bbox scaling does not preserve perpendicular
    // projection). The check runs on the resolved geometry, so it also catches
    // a `gradientTransform` that turns an axis-aligned gradient diagonal.
    let horizontal = (start.y - end.y).abs() < ZERO_LENGTH_EPSILON;
    let vertical = (start.x - end.x).abs() < ZERO_LENGTH_EPSILON;
    if !horizontal && !vertical {
        return false;
    }
    // WebRender stores gradient geometry relative to the primitive origin (it
    // adds `bounds.min` back in the shader), while `resolve_*_geometry` returns
    // absolute layout coordinates. Subtract the primitive origin to avoid the
    // double offset.
    let origin = area.min.to_vector();
    let start = start - origin;
    let end = end - origin;
    let gradient = ctx.wr.create_gradient(start, end, stops, extend_mode);
    let common = CommonItemProperties::new(
        area,
        SpaceAndClipInfo {
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
        },
    );
    ctx.wr.push_gradient(
        &common,
        area,
        gradient,
        area.size(),
        LayoutSize::new(0.0, 0.0),
    );
    true
}

/// Push a native radial gradient, returning `false` when it can't be expressed
/// natively (caller falls back to the software renderer).
fn push_radial_native(
    rg: &crate::model::document::RadialGradient,
    area: LayoutRect,
    geometry_bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) -> bool {
    if !gradient_lengths_equal(rg.fx, rg.cx) ||
        !gradient_lengths_equal(rg.fy, rg.cy) ||
        !rg.fr.is_zero()
    {
        return false;
    }
    let Some(extend_mode) = spread_to_extend(rg.spread_method) else {
        return false;
    };
    let stops = stops_to_webrender(
        &rg.stops,
        resolve_color_interpolation(rg.color_interpolation, ctx),
        opacity,
        rg.spread_method,
    );
    let Some((center, radius)) = resolve_radial_center_radius(rg, geometry_bounds, ctx) else {
        return false;
    };
    // See `push_linear_native`: WebRender adds `bounds.min` back in the shader,
    // so store the center relative to the primitive origin.
    let center = center - area.min.to_vector();
    let gradient = ctx
        .wr
        .create_radial_gradient(center, radius, stops, extend_mode);
    let common = CommonItemProperties::new(
        area,
        SpaceAndClipInfo {
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
        },
    );
    ctx.wr.push_radial_gradient(
        &common,
        area,
        gradient,
        area.size(),
        LayoutSize::new(0.0, 0.0),
    );
    true
}

/// Render a linear gradient.
/// gradientTransform is applied in gradient coordinate space (normed bbox or user space).
fn render_linear(
    lg: &crate::model::document::LinearGradient,
    area: LayoutRect,
    geometry_bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    // For `objectBoundingBox`, project in the normalized unit-square space of
    // the reference box (Chrome's behavior); see [`object_bbox_linear_fractions`].
    let (gx1, gy1, gx2, gy2, norm_origin_x, norm_origin_y, inv_scale_x, inv_scale_y) =
        match lg.units {
            GradientUnits::ObjectBoundingBox => {
                let (gx1, gy1, gx2, gy2) = object_bbox_linear_fractions(lg);
                (
                    gx1,
                    gy1,
                    gx2,
                    gy2,
                    geometry_bounds.min.x,
                    geometry_bounds.min.y,
                    1.0 / geometry_bounds.size().width,
                    1.0 / geometry_bounds.size().height,
                )
            },
            GradientUnits::UserSpaceOnUse => {
                let (start, end) = resolve_linear_geometry(lg, geometry_bounds, ctx);
                (start.x, start.y, end.x, end.y, 0.0, 0.0, 1.0, 1.0)
            },
        };

    let strategy = LinearStrategy {
        stops: &lg.stops,
        gx1,
        gy1,
        gx2,
        gy2,
        offset_x: area.min.x,
        offset_y: area.min.y,
        norm_origin_x,
        norm_origin_y,
        inv_scale_x,
        inv_scale_y,
    };
    render_gradient(
        area,
        ctx,
        opacity,
        &strategy,
        resolve_color_interpolation(lg.color_interpolation, ctx),
        lg.spread_method,
    );
}

/// Render a radial gradient.
fn render_radial(
    rg: &crate::model::document::RadialGradient,
    area: LayoutRect,
    geometry_bounds: LayoutRect,
    ctx: &mut RenderContext,
    opacity: f32,
) {
    let Some((_center, radius)) = resolve_radial_center_radius(rg, geometry_bounds, ctx) else {
        return;
    };

    // Honor an offset focal point (fx/fy); `center` coincides with the focal
    // only when the gradient is concentric.
    let focal = resolve_radial_focal(rg, geometry_bounds, ctx);

    // Resolve the focal radius (`fr`) to the same absolute space as `r`. It is
    // a radial distance, so its percentage resolves against the normalized
    // diagonal (kept scalar; the focal disk is approximated as a circle, which
    // is exact for the common `fr=0` case).
    let nd = normalized_diagonal(
        geometry_bounds.size().width,
        geometry_bounds.size().height,
    );
    let fr = match rg.units {
        GradientUnits::ObjectBoundingBox => rg.fr.to_object_bbox().max(0.0) * nd,
        GradientUnits::UserSpaceOnUse => rg.fr.to_user_space(nd).max(0.0),
    };

    let strategy = RadialStrategy {
        stops: &rg.stops,
        fx: focal.x,
        fy: focal.y,
        rx: radius.width,
        ry: radius.height,
        fr,
        offset_x: area.min.x,
        offset_y: area.min.y,
    };
    render_gradient(
        area,
        ctx,
        opacity,
        &strategy,
        resolve_color_interpolation(rg.color_interpolation, ctx),
        rg.spread_method,
    );
}

// ======================= Helpers =======================

/// Draw a single cell at (x,y) with the given color.
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
