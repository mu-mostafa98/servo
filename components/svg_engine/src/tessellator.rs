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

use crate::renderer::{RenderContext, shape_rendering_value, to_colorf};
use crate::renderer::gradient::{color_at_t, gradient_projection};
use crate::shapes::Shape;
use crate::style::FillRule;
use crate::style::gradient::GradientStop;
use crate::style::NodeStyle;

// ======================= Fill Style =======================

/// How to color a pixel during tessellation fill.
pub(crate) enum FillStyle<'a> {
    /// Solid uniform color.
    Solid(ColorF),
    /// Linear gradient with absolute coordinates.
    LinearGradient {
        stops: &'a [GradientStop],
        /// Gradient line start (absolute) in the same space as pixel positions.
        gx1: f32, gy1: f32,
        /// Gradient line end (absolute).
        gx2: f32, gy2: f32,
        opacity: f32,
    },
    /// Radial gradient evaluated per pixel.
    RadialGradient {
        stops: &'a [GradientStop],
        /// Focal point (absolute coordinates in the same space as pixel positions).
        fx: f32, fy: f32,
        /// Radius squared (absolute space).
        r2: f32,
        opacity: f32,
    },
    /// Pattern evaluated per pixel (shape hit-testing in tile-local coords).
    Pattern {
        /// The pattern's child shapes and their styles.
        shapes: &'a [(Shape, NodeStyle)],
        /// Tile dimensions in absolute space.
        tile_w: f32, tile_h: f32,
        /// Tile origin offset in absolute space.
        ox: f32, oy: f32,
        opacity: f32,
    },
}

// ======================= Point-in-Shape Helpers =======================

/// Check whether a point `(px, py)` (in shape-local coordinates) falls inside
/// a shape.  Used for per-pixel pattern evaluation.
///
/// Only rect, circle, and ellipse are supported — polyline/polygon/path
/// return `false` (they do not contribute to the pattern color at this
/// position, which is a simplification for common pattern shapes).
fn point_in_shape(px: f32, py: f32, shape: &Shape) -> bool {
    match shape {
        Shape::Rect(r) => {
            px >= r.x && px <= r.x + r.width &&
            py >= r.y && py <= r.y + r.height
        },
        Shape::Circle(c) => {
            let dx = px - c.cx;
            let dy = py - c.cy;
            dx * dx + dy * dy <= c.r * c.r
        },
        Shape::Ellipse(e) => {
            if e.rx <= 0.0 || e.ry <= 0.0 { return false; }
            let dx = (px - e.cx) / e.rx;
            let dy = (py - e.cy) / e.ry;
            dx * dx + dy * dy <= 1.0
        },
        // Line / polyline / polygon / path — no fill contribution or too complex.
        Shape::Line(_) | Shape::Polyline(_) | Shape::Polygon(_) | Shape::Path(_) => false,
    }
}

/// Evaluate the pattern at a tile-local position `(tx, ty)` using the painter's
/// model (last shape in the list is on top).  Returns `Some(ColorF)` if a shape
/// with a solid fill covers the point, `None` if the point is transparent.
fn pattern_color_at(
    tx: f32, ty: f32,
    shapes: &[(Shape, NodeStyle)],
) -> Option<ColorF> {
    // Iterate in reverse (painter's order: last drawn = on top).
    for (shape, style) in shapes.iter().rev() {
        if point_in_shape(tx, ty, shape) {
            if let Some(ref fill) = style.fill {
                if let Some(ref svg_color) = fill.color {
                    return Some(to_colorf(svg_color));
                }
                // Gradient/pattern fills inside patterns — skip (complex).
            }
        }
    }
    None
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
            FillStyle::LinearGradient { stops, gx1, gy1, gx2, gy2, opacity } => {
                let mut cx = x_left;
                while cx < x_right {
                    let cw = CELL.min(x_right - cx);
                    let rx = cx + cw / 2.0;
                    let ry = center;
                    let t = gradient_projection(rx, ry, *gx1, *gy1, *gx2, *gy2);
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
            FillStyle::RadialGradient { stops, fx, fy, r2, opacity } => {
                let mut cx = x_left;
                while cx < x_right {
                    let cw = CELL.min(x_right - cx);
                    let rx = cx + cw / 2.0;
                    let dx = rx - fx;
                    let dy = center - fy;
                    let dist_sq = (dx * dx + dy * dy) / r2.max(1.0);
                    let t = dist_sq.sqrt().min(1.0);
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
            FillStyle::Pattern { shapes, tile_w, tile_h, ox, oy, opacity } => {
                let mut cx = x_left;
                while cx < x_right {
                    // Use a smaller cell (2px) for pattern fills so that
                    // small pattern shapes (e.g. a 5px circle) don't look
                    // overly blocky.
                    let cw = 2.0f32.min(x_right - cx);
                    let rx = cx + cw / 2.0;
                    let ry = center;

                    // Compute tile-local position (handle negative coordinates).
                    let tile_x = ((rx - ox) % tile_w + tile_w) % tile_w;
                    let tile_y = ((ry - oy) % tile_h + tile_h) % tile_h;

                    if let Some(mut color) = pattern_color_at(tile_x, tile_y, shapes) {
                        color.a *= opacity;
                        let cell_rect = LayoutRect::from_origin_and_size(
                            LayoutPoint::new(cx, yf), LayoutSize::new(cw, 1.0),
                        );
                        let common = CommonItemProperties::new(
                            cell_rect,
                            SpaceAndClipInfo { spatial_id: ctx.spatial_id, clip_chain_id: ctx.clip_chain_id },
                        );
                        ctx.wr.push_rect(&common, cell_rect, color);
                    }
                    cx += cw;
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
