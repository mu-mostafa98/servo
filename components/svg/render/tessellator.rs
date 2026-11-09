/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Polygon tessellation + scanline rasterization.
//!
//! Tessellates a polygon into triangles using [lyon](https://docs.rs/lyon/),
//! then renders each triangle via scanline rasterization: for each Y scanline,
//! the horizontal span inside the triangle is computed and drawn with a
//! [`push_rect`] call.
//!
//! This approach uses only [`push_rect`] which is known to work correctly
//! in WebRender, avoiding `define_clip_image_mask` which requires a
//! valid `ImageKey` to function as a polygon clip.

use lyon::math::Point as LyonPoint;
use lyon::path::polygon::Polygon;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, VertexBuffers,
};
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ColorF, CommonItemProperties, SpaceAndClipInfo};

use crate::model::document::{GradientStop, SpreadMethod};
use crate::model::element::SvgNode;
use crate::model::style::{ColorInterpolation, FillRule};
use crate::render::renderer::gradient::{color_at_t_with_spread, gradient_projection};
use crate::render::renderer::{Render, RenderContext, clip_chain_option, shape_rendering_value};

// ======================= Fill Style =======================

/// How to color a pixel during tessellation fill.
pub(crate) enum FillStyle<'a> {
    /// Solid uniform color.
    Solid(ColorF),
    /// Linear gradient with absolute coordinates.
    LinearGradient {
        stops: &'a [GradientStop],
        /// Gradient line start (absolute) in the same space as pixel positions.
        gx1: f32,
        gy1: f32,
        /// Gradient line end (absolute).
        gx2: f32,
        gy2: f32,
        opacity: f32,
        /// Color interpolation space hint.
        color_interpolation: ColorInterpolation,
        /// How the gradient extends beyond its stop range.
        spread_method: SpreadMethod,
    },
    /// Radial gradient evaluated per pixel.
    RadialGradient {
        stops: &'a [GradientStop],
        /// Focal point (absolute coordinates in the same space as pixel positions).
        fx: f32,
        fy: f32,
        /// Outer ellipse radii (absolute space).
        rx: f32,
        ry: f32,
        /// Focal radius (`fr`), already clamped to the smaller outer radius.
        fr: f32,
        opacity: f32,
        /// Color interpolation space hint.
        color_interpolation: ColorInterpolation,
        /// How the gradient extends beyond its stop range.
        spread_method: SpreadMethod,
    },
    /// Pattern evaluated per pixel (shape hit-testing in tile-local coords).
    Pattern {
        /// The pattern's child subtree.
        root: &'a SvgNode,
        /// Tile dimensions in absolute space.
        tile_w: f32,
        tile_h: f32,
        /// Tile origin offset in absolute space.
        ox: f32,
        oy: f32,
    },
}

// ======================= Tessellation =======================

/// Tessellate a closed polygon into a list of triangles.
///
/// Returns `(vertices, indices)` where every three consecutive indices
/// form one triangle.  Returns `None` when the polygon has fewer than
/// 3 vertices or the tessellator fails.
fn tessellate_to_triangles(
    points: &[LyonPoint],
    fill_rule: FillRule,
    tolerance: f32,
) -> Option<(Vec<LyonPoint>, Vec<u32>)> {
    if points.len() < 3 {
        return None;
    }

    let polygon = Polygon {
        points,
        closed: true,
    };
    let mut tessellator = FillTessellator::new();
    let mut buffers: VertexBuffers<LyonPoint, u32> = VertexBuffers::new();

    let lyon_fill_rule = match fill_rule {
        FillRule::NonZero => lyon::path::FillRule::NonZero,
        FillRule::EvenOdd => lyon::path::FillRule::EvenOdd,
    };

    if tessellator
        .tessellate(
            polygon.path_events(),
            &FillOptions::default()
                .with_fill_rule(lyon_fill_rule)
                .with_tolerance(tolerance),
            &mut BuffersBuilder::new(&mut buffers, PosCtor),
        )
        .is_err()
    {
        return None;
    }

    Some((buffers.vertices, buffers.indices))
}

struct PosCtor;

impl FillVertexConstructor<LyonPoint> for PosCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> LyonPoint {
        vertex.position()
    }
}

// ======================= Public API =======================

/// Fill an arbitrary polygon using lyon tessellation + scanline rasterization.
///
/// `points` must already be shifted into WebRender layout-space coordinates.
/// `fill_rule` controls how overlapping regions are resolved.
/// `fill` describes the color (solid or gradient) to apply.
pub(crate) fn tessellate_polygon(
    points: &[LyonPoint],
    fill_rule: FillRule,
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    let tol = shape_rendering_value(ctx, 0.001, 0.1, 0.01);

    if let Some((vertices, indices)) = tessellate_to_triangles(points, fill_rule, tol) {
        scanline_fill_triangles(&vertices, &indices, fill, ctx);
    }
}

/// Tessellate one or more closed contours — the first is the outer boundary and
/// any following ones are holes — and fill them via scanline rasterization.
///
/// This is the multi-contour variant of [`tessellate_polygon`], used for
/// `shape-rendering="crispEdges"` strokes where a ring (outer + inner outline)
/// must be filled with aliased, snapped spans.
pub(crate) fn tessellate_contours(
    contours: &[&[LyonPoint]],
    fill_rule: FillRule,
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    let tol = shape_rendering_value(ctx, 0.001, 0.1, 0.01);

    if let Some((vertices, indices)) = tessellate_contours_to_triangles(contours, fill_rule, tol) {
        scanline_fill_triangles(&vertices, &indices, fill, ctx);
    }
}

/// Build a multi-contour lyon [`Path`] and tessellate it into triangles.
fn tessellate_contours_to_triangles(
    contours: &[&[LyonPoint]],
    fill_rule: FillRule,
    tolerance: f32,
) -> Option<(Vec<LyonPoint>, Vec<u32>)> {
    let mut builder = lyon::path::Path::builder();
    let mut any = false;
    for contour in contours {
        if contour.len() < 3 {
            continue;
        }
        any = true;
        for (i, p) in contour.iter().enumerate() {
            if i == 0 {
                builder.begin(lyon::math::point(p.x, p.y));
            } else {
                builder.line_to(lyon::math::point(p.x, p.y));
            }
        }
        builder.close();
    }
    if !any {
        return None;
    }
    let path = builder.build();

    let lyon_fill_rule = match fill_rule {
        FillRule::NonZero => lyon::path::FillRule::NonZero,
        FillRule::EvenOdd => lyon::path::FillRule::EvenOdd,
    };

    let mut tessellator = FillTessellator::new();
    let mut buffers: VertexBuffers<LyonPoint, u32> = VertexBuffers::new();
    if tessellator
        .tessellate(
            path.iter(),
            &FillOptions::default()
                .with_fill_rule(lyon_fill_rule)
                .with_tolerance(tolerance),
            &mut BuffersBuilder::new(&mut buffers, PosCtor),
        )
        .is_err()
    {
        return None;
    }

    Some((buffers.vertices, buffers.indices))
}

// ======================= Scanline rasterization =======================

/// Render every triangle in the vertex/index buffer via scanline rasterization.
fn scanline_fill_triangles(
    vertices: &[LyonPoint],
    indices: &[u32],
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];
        scanline_fill_triangle(v0, v1, v2, fill, ctx);
    }
}

/// Fill a single triangle — for each Y scanline, emit a [`push_rect`] span.
///
/// Uses the classic edge-crossing scanline algorithm (see
/// [`triangle_span_at`]), which is robust against the degenerate
/// (nearly-flat) sliver triangles that lyon emits for curved rings: the
/// previous top/mid/bottom slope interpolation computed `1/(mid.y - bot.y)`
/// for two vertices whose Y differs only by floating-point rounding of
/// `sin(π)`, producing a near-infinite slope and a spurious full-width span.
fn scanline_fill_triangle(
    v0: LyonPoint,
    v1: LyonPoint,
    v2: LyonPoint,
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    let ymin = v0.y.min(v1.y).min(v2.y);
    let ymax = v0.y.max(v1.y).max(v2.y);

    // Rows whose center (`y + 0.5`) lies within `[ymin, ymax]`.
    let y_start = (ymin - 0.5).ceil() as i32;
    let y_end = (ymax - 0.5).floor() as i32;
    if y_start > y_end {
        return;
    }

    for y in y_start..=y_end {
        let center = y as f32 + 0.5;
        if let Some((x_left, x_right)) = triangle_span_at(v0, v1, v2, center) {
            emit_scanline_span(x_left, x_right, y as f32, fill, ctx);
        }
    }
}

/// Compute the horizontal span `(x_left, x_right)` of a triangle at the
/// scanline whose vertical center is `center`.  Returns `None` when the
/// scanline does not cross the triangle's interior (at or beyond a vertex).
///
/// Each edge is intersected with the horizontal line using a half-open test
/// `lo <= center < hi`: a vertex shared by two edges is counted exactly once,
/// and a zero-height (horizontal) edge never matches — so the interpolation
/// divisor is always well-conditioned and can never blow up into a spurious
/// full-width span.
fn triangle_span_at(
    v0: LyonPoint,
    v1: LyonPoint,
    v2: LyonPoint,
    center: f32,
) -> Option<(f32, f32)> {
    let mut crossings = [0.0f32; 2];
    let mut n = 0usize;
    for (p, q) in [(v0, v1), (v1, v2), (v2, v0)] {
        let (lo, hi, xlo, xhi) = if p.y <= q.y {
            (p.y, q.y, p.x, q.x)
        } else {
            (q.y, p.y, q.x, p.x)
        };
        if lo <= center && center < hi {
            let x = xlo + (xhi - xlo) * (center - lo) / (hi - lo);
            crossings[n] = x;
            n += 1;
            if n == 2 {
                break;
            }
        }
    }
    if n < 2 {
        return None;
    }
    let (x_left, x_right) = if crossings[0] <= crossings[1] {
        (crossings[0], crossings[1])
    } else {
        (crossings[1], crossings[0])
    };
    if x_right <= x_left {
        return None;
    }
    Some((x_left, x_right))
}

/// Emit the horizontal span `[x_left, x_right)` on row `yf` with the given fill.
fn emit_scanline_span(
    x_left: f32,
    x_right: f32,
    yf: f32,
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    let center = yf + 0.5;
    const CELL: f32 = 4.0;

    match fill {
        FillStyle::Solid(c) => {
            let rect = LayoutRect::from_origin_and_size(
                LayoutPoint::new(x_left, yf),
                LayoutSize::new(x_right - x_left, 1.0),
            );
            let common = CommonItemProperties::new(
                rect,
                SpaceAndClipInfo {
                    spatial_id: ctx.spatial_id,
                    clip_chain_id: ctx.clip_chain_id,
                },
            );
            ctx.wr.push_rect(&common, rect, *c);
        },
        FillStyle::LinearGradient {
            stops,
            gx1,
            gy1,
            gx2,
            gy2,
            opacity,
            color_interpolation,
            spread_method,
            ..
        } => {
            let mut cx = x_left;
            let mut pending: Option<(ColorF, f32, f32)> = None;
            while cx < x_right {
                let cw = CELL.min(x_right - cx);
                let rx = cx + cw / 2.0;
                let ry = center;
                let t = gradient_projection(rx, ry, *gx1, *gy1, *gx2, *gy2);
                let mut c =
                    color_at_t_with_spread(stops, t, *color_interpolation, *spread_method);
                c.a *= opacity;

                match pending {
                    Some((ref prev_c, ref mut seg_start, ref mut seg_end))
                        if colors_nearly_equal(&c, prev_c) =>
                    {
                        // Same color — extend current segment.
                        *seg_end = cx + cw;
                    },
                    _ => {
                        // Color changed — flush pending segment, start new one.
                        if let Some((ref pc, ps, pe)) = pending.take() {
                            emit_gradient_rect(ps, pe, yf, pc, ctx);
                        }
                        pending = Some((c, cx, cx + cw));
                    },
                }
                cx += CELL;
            }
            if let Some((ref c, start, end)) = pending {
                emit_gradient_rect(start, end, yf, c, ctx);
            }
        },
        FillStyle::RadialGradient {
            stops,
            fx,
            fy,
            rx,
            ry,
            fr,
            opacity,
            color_interpolation,
            spread_method,
            ..
        } => {
            let mut cx = x_left;
            let mut pending: Option<(ColorF, f32, f32)> = None;
            while cx < x_right {
                let cw = CELL.min(x_right - cx);
                let cell_x = cx + cw / 2.0;
                let dx = cell_x - fx;
                let dy = center - fy;
                // Normalized elliptical distance: `1.0` on the outer ellipse
                // (§13.2.2 non-uniform bounding-box scaling).
                let d = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
                // `t` maps distance through the focal radius: points inside
                // the focal disk get the first stop color.
                let fr_norm = fr / rx.min(*ry);
                let t = if d <= fr_norm {
                    0.0
                } else if fr_norm >= 1.0 {
                    1.0
                } else {
                    ((d - fr_norm) / (1.0 - fr_norm)).min(1.0)
                };
                let mut c =
                    color_at_t_with_spread(stops, t, *color_interpolation, *spread_method);
                c.a *= opacity;

                match pending {
                    Some((ref prev_c, ref mut seg_start, ref mut seg_end))
                        if colors_nearly_equal(&c, prev_c) =>
                    {
                        *seg_end = cx + cw;
                    },
                    _ => {
                        if let Some((ref pc, ps, pe)) = pending.take() {
                            emit_gradient_rect(ps, pe, yf, pc, ctx);
                        }
                        pending = Some((c, cx, cx + cw));
                    },
                }
                cx += CELL;
            }
            if let Some((ref c, start, end)) = pending {
                emit_gradient_rect(start, end, yf, c, ctx);
            }
        },
        FillStyle::Pattern {
            root,
            tile_w,
            tile_h,
            ox,
            oy,
        } => {
            // Render pattern shapes using proper shape.render() calls,
            // grouped by tile column and clipped to the polygon
            // boundary per scanline.  This matches the quality of the
            // rect-based pattern path (pixel-perfect rounded rects for
            // circles) instead of the blocky per-pixel evaluation the
            // old code produced.
            let row = ((center - oy) / tile_h).floor() as i32;
            let col_start = ((x_left - ox) / tile_w).floor() as i32;
            let col_end = ((x_right - ox) / tile_w).ceil() as i32;

            for col in col_start..col_end {
                let tile_origin =
                    LayoutPoint::new(ox + col as f32 * tile_w, oy + row as f32 * tile_h);
                let tile_x0 = tile_origin.x;
                let tile_x1 = tile_origin.x + tile_w;

                let span_x0 = tile_x0.max(x_left);
                let span_x1 = tile_x1.min(x_right);
                if span_x0 >= span_x1 {
                    continue;
                }

                // Clip this tile's rendering to the scanline span
                // that falls inside the polygon.
                let clip_bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(span_x0, yf),
                    LayoutSize::new(span_x1 - span_x0, 1.0),
                );
                let clip_id = ctx.wr.define_clip_rect(ctx.spatial_id, clip_bounds);
                let tile_chain = ctx
                    .wr
                    .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [clip_id]);

                root.for_each_shape_leaf(&mut |shape, shape_style| {
                    if !shape_style.is_visible() {
                        return;
                    }
                    let mut shape_ctx = RenderContext {
                        style: shape_style,
                        svg_origin: tile_origin,
                        spatial_id: ctx.spatial_id,
                        clip_chain_id: tile_chain,
                        wr: &mut *ctx.wr,
                        accumulated_scale: ctx.accumulated_scale,
                        viewbox_scale: ctx.viewbox_scale,
                        device_scale: ctx.device_scale,
                        raster_offset: ctx.raster_offset,
                        native_rendering: ctx.native_rendering,
                        sink: ctx.sink,
                        defs: ctx.defs,
                        def_depth: ctx.def_depth + 1,
                    };
                    shape.render(&mut shape_ctx);
                });
            }
        },
    }
}

// ======================= RLE Optimization Helpers =======================

/// Epsilon for comparing two colors as "close enough to merge" in the RLE
/// scanline optimization.  Prevents excessive `push_rect` calls when gradient
/// colors barely change between adjacent 4px cells.
const COLOR_MERGE_EPSILON: f32 = 1.0 / 256.0;

/// Return `true` if two colors are close enough to merge into a single rect.
fn colors_nearly_equal(a: &ColorF, b: &ColorF) -> bool {
    (a.r - b.r).abs() <= COLOR_MERGE_EPSILON &&
        (a.g - b.g).abs() <= COLOR_MERGE_EPSILON &&
        (a.b - b.b).abs() <= COLOR_MERGE_EPSILON &&
        (a.a - b.a).abs() <= COLOR_MERGE_EPSILON
}

/// Emit a single `push_rect` for a merged gradient segment (one or more
/// adjacent cells with the same evaluated color).  Reduces WebRender
/// draw-call count compared to emitting one rect per 4px cell.
fn emit_gradient_rect(x_start: f32, x_end: f32, y: f32, color: &ColorF, ctx: &mut RenderContext) {
    let w = x_end - x_start;
    if w <= 0.0 {
        return;
    }
    let rect =
        LayoutRect::from_origin_and_size(LayoutPoint::new(x_start, y), LayoutSize::new(w, 1.0));
    let common = CommonItemProperties::new(
        rect,
        SpaceAndClipInfo {
            spatial_id: ctx.spatial_id,
            clip_chain_id: ctx.clip_chain_id,
        },
    );
    ctx.wr.push_rect(&common, rect, *color);
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use lyon::math::Point as LyonPoint;

    use super::{tessellate_contours_to_triangles, triangle_span_at};
    use crate::model::style::FillRule;

    /// A normal triangle: `triangle_span_at` must return the correct span
    /// (a left/right crossing pair) for a scanline through its interior.
    #[test]
    fn span_through_triangle_interior() {
        let v0 = LyonPoint::new(0.0, 0.0);
        let v1 = LyonPoint::new(10.0, 0.0);
        let v2 = LyonPoint::new(5.0, 10.0);
        // At y-center 5.0 the triangle spans from x=2.5 to x=7.5.
        let (l, r) = triangle_span_at(v0, v1, v2, 5.0).expect("span exists");
        assert!((l - 2.5).abs() < 1e-4, "left = {l}");
        assert!((r - 7.5).abs() < 1e-4, "right = {r}");
    }

    /// The degenerate sliver triangle lyon emits for a ring: two vertices share
    /// a Y differing only by `sin(π)` rounding. `triangle_span_at` must never
    /// blow up into a huge span — the scanline beyond its bottom is `None`, and
    /// the span stays within the triangle's x-extent.
    #[test]
    fn sliver_triangle_does_not_explode() {
        let v0 = LyonPoint::new(34.1011, 47.9416);
        let v1 = LyonPoint::new(30.0, 50.0);
        let v2 = LyonPoint::new(34.0, 50.0);
        // Row 50 (center 50.5) is below the triangle's bottom edge (y=50.0).
        assert!(triangle_span_at(v0, v1, v2, 50.5).is_none());
        // Row 49 (center 49.5) is inside; the span must stay within [30, 34.1].
        let (l, r) = triangle_span_at(v0, v1, v2, 49.5).expect("span exists");
        assert!(l >= 30.0 && r <= 34.2, "span = ({l}, {r})");
    }

    /// A concentric-circle ring (outer r=25, inner r=21) tessellates to finite
    /// triangles fully inside the ring's bounding box, and no scanline span
    /// produced by `triangle_span_at` leaves that box.
    #[test]
    fn ring_tessellation_stays_in_bounds() {
        let circle = |cx: f32, cy: f32, r: f32, n: usize| -> Vec<LyonPoint> {
            (0..n)
                .map(|i| {
                    let a = (i as f32 / n as f32) * std::f32::consts::TAU;
                    LyonPoint::new(cx + r * a.cos(), cy + r * a.sin())
                })
                .collect()
        };
        let outer = circle(55.0, 50.0, 25.0, 64);
        let inner = circle(55.0, 50.0, 21.0, 64);

        let (vertices, indices) =
            tessellate_contours_to_triangles(&[&outer, &inner], FillRule::EvenOdd, 0.01)
                .expect("ring tessellates");

        for v in &vertices {
            assert!(v.x.is_finite() && v.y.is_finite());
            assert!(
                v.x >= 29.0 && v.x <= 81.0 && v.y >= 24.0 && v.y <= 76.0,
                "vertex out of bounds: ({}, {})",
                v.x,
                v.y
            );
        }

        for tri in indices.chunks(3) {
            let v0 = vertices[tri[0] as usize];
            let v1 = vertices[tri[1] as usize];
            let v2 = vertices[tri[2] as usize];
            let ymin = v0.y.min(v1.y).min(v2.y);
            let ymax = v0.y.max(v1.y).max(v2.y);
            for y in ((ymin - 0.5).ceil() as i32)..=((ymax - 0.5).floor() as i32) {
                let center = y as f32 + 0.5;
                if let Some((l, r)) = triangle_span_at(v0, v1, v2, center) {
                    assert!(
                        l >= 29.0 && r <= 81.0,
                        "span ({l}, {r}) escapes the ring bbox at y={y}"
                    );
                }
            }
        }
    }
}
