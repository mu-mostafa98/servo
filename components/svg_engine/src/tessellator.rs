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
use lyon::tessellation::{
    FillTessellator, FillOptions, FillVertex, FillVertexConstructor,
    VertexBuffers, BuffersBuilder,
};
use lyon::path::polygon::Polygon;
use webrender_api::{
    ColorF, CommonItemProperties, SpaceAndClipInfo,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::renderer::{RenderContext, shape_rendering_value};
use crate::renderer::gradient::{color_at_t, gradient_projection};
use crate::style::FillRule;
use crate::style::gradient::{GradientStop, GradientUnits};

// ======================= Fill Style =======================

/// How to color a pixel during tessellation fill.
pub(crate) enum FillStyle<'a> {
    /// Solid uniform color.
    Solid(ColorF),
    /// Linear gradient with absolute coordinates (in shifted-pts space).
    LinearGradient {
        stops: &'a [GradientStop],
        x1: f32, y1: f32, x2: f32, y2: f32,
        units: GradientUnits,
        bx: f32, by: f32, bw: f32, bh: f32,
        opacity: f32,
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

    let polygon = Polygon { points, closed: true };
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

// ======================= Scanline rasterization =======================

/// Render every triangle in the vertex/index buffer via scanline rasterization.
fn scanline_fill_triangles(
    vertices: &[LyonPoint],
    indices: &[u32],
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    for tri in indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];
        scanline_fill_triangle(v0, v1, v2, fill, ctx);
    }
}

/// Fill a single triangle — for each Y scanline, emit a [`push_rect`] span.
fn scanline_fill_triangle(
    v0: LyonPoint, v1: LyonPoint, v2: LyonPoint,
    fill: &FillStyle,
    ctx: &mut RenderContext,
) {
    let (top, mid, bot) = sort_vertices_by_y(v0, v1, v2);

    let top_y = top.y.ceil();
    let bot_y = bot.y.floor();
    if top_y > bot_y { return; }

    let inv_dy_tm = if mid.y != top.y { 1.0 / (mid.y - top.y) } else { 0.0 };
    let inv_dy_tb = if bot.y != top.y { 1.0 / (bot.y - top.y) } else { 0.0 };
    let inv_dy_mb = if bot.y != mid.y { 1.0 / (bot.y - mid.y) } else { 0.0 };

    let dx_tm = (mid.x - top.x) * inv_dy_tm;
    let dx_tb = (bot.x - top.x) * inv_dy_tb;
    let dx_mb = (bot.x - mid.x) * inv_dy_mb;

    let y_start = top_y as i32;
    let y_end = bot_y as i32;
    const CELL: f32 = 4.0;

    for y in y_start..=y_end {
        let yf = y as f32;
        let center = yf + 0.5;

        let (x_left, x_right) = if center < mid.y {
            let a = top.x + dx_tm * (center - top.y);
            let b = top.x + dx_tb * (center - top.y);
            (a.min(b), a.max(b))
        } else {
            let a = top.x + dx_tb * (center - top.y);
            let b = mid.x + dx_mb * (center - mid.y);
            (a.min(b), a.max(b))
        };

        let width = x_right - x_left;
        if width <= 0.0 { continue; }

        match fill {
            FillStyle::Solid(c) => {
                let rect = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(x_left, yf), LayoutSize::new(width, 1.0),
                );
                let common = CommonItemProperties::new(
                    rect, SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
                );
                ctx.wr.push_rect(&common, rect, *c);
            },
            FillStyle::LinearGradient { stops, x1, y1, x2, y2, units, bx, by, bw, bh, opacity } => {
                let ry = center - by;
                let (gx1, gy1, gx2, gy2) = match units {
                    GradientUnits::ObjectBoundingBox =>
                        (x1 * bw, y1 * bh, x2 * bw, y2 * bh),
                    GradientUnits::UserSpaceOnUse =>
                        (x1 - bx, y1 - by, x2 - bx, y2 - by),
                };
                let mut cx = x_left;
                while cx < x_right {
                    let cw = CELL.min(x_right - cx);
                    let rx = (cx + cw / 2.0) - bx;
                    let t = gradient_projection(rx, ry, gx1, gy1, gx2, gy2);
                    let mut c = color_at_t(stops, t);
                    c.a *= opacity;
                    let cell_rect = LayoutRect::from_origin_and_size(
                        LayoutPoint::new(cx, yf), LayoutSize::new(cw, 1.0),
                    );
                    let common = CommonItemProperties::new(
                        cell_rect,
                        SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
                    );
                    ctx.wr.push_rect(&common, cell_rect, c);
                    cx += CELL;
                }
            },
        }
    }
}

/// Sort three points by their Y coordinate (ascending).
fn sort_vertices_by_y(
    a: LyonPoint, b: LyonPoint, c: LyonPoint,
) -> (LyonPoint, LyonPoint, LyonPoint) {
    let mut pts = [a, b, c];
    pts.sort_by(|p, q| p.y.partial_cmp(&q.y).unwrap_or(std::cmp::Ordering::Equal));
    (pts[0], pts[1], pts[2])
}
