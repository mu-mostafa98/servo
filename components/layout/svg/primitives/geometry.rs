/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pure path geometry and marker-placement angle math.
//!
//! Everything here is Servo-free: it operates on `tiny_skia_path`, `kurbo` and
//! `svgtypes` values only, so it is the sole unit-testable leaf of the SVG layer.

use resvg::usvg::{ApproxEqUlps, tiny_skia_path};
use svgtypes::{Align, PointsParser, SimplePathSegment, SimplifyingPathParser};

/// Computes the aligned origin for a `preserveAspectRatio` fit, mirroring usvg's
/// `crate::aligned_pos` (which is crate-private).
pub(crate) fn aligned_pos(align: Align, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    match align {
        Align::None | Align::XMinYMin => (x, y),
        Align::XMidYMin => (x + w / 2.0, y),
        Align::XMaxYMin => (x + w, y),
        Align::XMinYMid => (x, y + h / 2.0),
        Align::XMidYMid => (x + w / 2.0, y + h / 2.0),
        Align::XMaxYMid => (x + w, y + h / 2.0),
        Align::XMinYMax => (x, y + h),
        Align::XMidYMax => (x + w / 2.0, y + h),
        Align::XMaxYMax => (x + w, y + h),
    }
}

/// Builds a polygon/polyline path from a `points` attribute value.
pub(crate) fn polygon_points(value: &str, close: bool) -> Option<tiny_skia_path::Path> {
    let mut pb = tiny_skia_path::PathBuilder::new();
    let mut has_point = false;
    for (x, y) in PointsParser::from(value) {
        if !has_point {
            pb.move_to(x as f32, y as f32);
            has_point = true;
        } else {
            pb.line_to(x as f32, y as f32);
        }
    }
    if !has_point {
        return None;
    }
    if close {
        pb.close();
    }
    pb.finish()
}

pub(crate) fn rounded_rect(
    pb: &mut tiny_skia_path::PathBuilder,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rx: f32,
    ry: f32,
) {
    // Mirrors usvg's `convert_rect`: a plain rectangle for zero radii, otherwise
    // four corner arcs appended via the kurbo-backed [`arc_to`]. This reuses the
    // same correct 90° arc→cubic conversion as `SimplifyingPathParser` instead of
    // the old hand-rolled `K = 0.5522847498` control-point factor.
    if rx <= 0.0 || ry <= 0.0 {
        let Some(rect) = tiny_skia_path::Rect::from_xywh(x, y, w, h) else {
            return;
        };
        pb.push_rect(rect);
        return;
    }

    pb.move_to(x + rx, y);
    pb.line_to(x + w - rx, y);
    arc_to(pb, rx, ry, x + w, y + ry);

    pb.line_to(x + w, y + h - ry);
    arc_to(pb, rx, ry, x + w - rx, y + h);

    pb.line_to(x + rx, y + h);
    arc_to(pb, rx, ry, x, y + h - ry);

    pb.line_to(x, y + ry);
    arc_to(pb, rx, ry, x + rx, y);

    pb.close();
}

/// Appends a 90° corner arc from the current point to `(x, y)`, converting it to
/// cubic Béziers via `kurbo::Arc::from_svg_arc`. Mirrors usvg's `PathBuilderExt::arc_to`.
fn arc_to(pb: &mut tiny_skia_path::PathBuilder, rx: f32, ry: f32, x: f32, y: f32) {
    let Some(prev) = pb.last_point() else {
        return;
    };

    let svg_arc = kurbo::SvgArc {
        from: kurbo::Point::new(prev.x as f64, prev.y as f64),
        to: kurbo::Point::new(x as f64, y as f64),
        radii: kurbo::Vec2::new(rx as f64, ry as f64),
        x_rotation: 0.0,
        large_arc: false,
        sweep: true,
    };

    match kurbo::Arc::from_svg_arc(&svg_arc) {
        Some(arc) => {
            arc.to_cubic_beziers(0.1, |p1, p2, p| {
                pb.cubic_to(
                    p1.x as f32,
                    p1.y as f32,
                    p2.x as f32,
                    p2.y as f32,
                    p.x as f32,
                    p.y as f32,
                );
            });
        },
        None => {
            pb.line_to(x, y);
        },
    }
}

pub(crate) fn parse_path_d(d: &str) -> Option<tiny_skia_path::Path> {
    // Delegate to `SimplifyingPathParser`, the same parser usvg uses in its own
    // `convert_path`: it resolves relative→absolute coordinates, `S`/`T`
    // reflection, `H`/`V`→`L`, and converts elliptical arcs (`A`) to cubic
    // Béziers via `kurbo` (correct math, including the coincident-points and
    // radii-correction edge cases). Hand-rolling that here previously produced
    // distorted arcs (see `arc_to`'s incorrect control-point factor).
    let mut pb = tiny_skia_path::PathBuilder::new();
    for seg in SimplifyingPathParser::from(d) {
        let seg = seg.ok()?;
        match seg {
            SimplePathSegment::MoveTo { x, y } => {
                pb.move_to(x as f32, y as f32);
            },
            SimplePathSegment::LineTo { x, y } => {
                pb.line_to(x as f32, y as f32);
            },
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
            },
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                pb.cubic_to(
                    x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
                );
            },
            SimplePathSegment::ClosePath => {
                pb.close();
            },
        }
    }
    pb.finish()
}

// ===== Marker geometry =====

/// A marker path segment. `QuadTo` is resolved to `CubicTo` up front (mirroring
/// usvg's `parser::marker::Segment`), since the vertex-tangent math only handles
/// lines and cubics.
#[derive(Copy, Clone, Debug)]
pub(crate) enum MarkerSegment {
    MoveTo(tiny_skia_path::Point),
    LineTo(tiny_skia_path::Point),
    CubicTo(
        tiny_skia_path::Point,
        tiny_skia_path::Point,
        tiny_skia_path::Point,
    ),
    Close,
}

#[derive(Copy, Clone, PartialEq)]
pub(crate) enum MarkerKind {
    Start,
    Middle,
    End,
}

#[derive(Copy, Clone)]
pub(crate) enum MarkerOrientation {
    Auto,
    AutoStartReverse,
    Angle(f32),
}

/// Converts `path`'s segments into the marker-friendly [`MarkerSegment`] list.
pub(crate) fn build_marker_segments(path: &tiny_skia_path::Path) -> Vec<MarkerSegment> {
    let mut segments = Vec::new();
    let mut prev = tiny_skia_path::Point::zero();
    let mut prev_move = tiny_skia_path::Point::zero();
    for seg in path.segments() {
        match seg {
            tiny_skia_path::PathSegment::MoveTo(p) => {
                segments.push(MarkerSegment::MoveTo(p));
                prev = p;
                prev_move = p;
            },
            tiny_skia_path::PathSegment::LineTo(p) => {
                segments.push(MarkerSegment::LineTo(p));
                prev = p;
            },
            tiny_skia_path::PathSegment::QuadTo(p1, p) => {
                let (p1, p2, p) = quad_to_curve(prev, p1, p);
                segments.push(MarkerSegment::CubicTo(p1, p2, p));
                prev = p;
            },
            tiny_skia_path::PathSegment::CubicTo(p1, p2, p) => {
                segments.push(MarkerSegment::CubicTo(p1, p2, p));
                prev = p;
            },
            tiny_skia_path::PathSegment::Close => {
                segments.push(MarkerSegment::Close);
                prev = prev_move;
            },
        }
    }
    segments
}

pub(crate) fn calc_vertex_angle(segments: &[MarkerSegment], idx: usize) -> f32 {
    if idx == 0 {
        // First segment.

        debug_assert!(segments.len() > 1);

        let seg1 = segments[0];
        let seg2 = segments[1];

        match (seg1, seg2) {
            (MarkerSegment::MoveTo(pm), MarkerSegment::LineTo(p)) => {
                calc_line_angle(pm.x, pm.y, p.x, p.y)
            },
            (MarkerSegment::MoveTo(pm), MarkerSegment::CubicTo(p1, _, p)) => {
                if pm.x.approx_eq_ulps(&p1.x, 4) && pm.y.approx_eq_ulps(&p1.y, 4) {
                    calc_line_angle(pm.x, pm.y, p.x, p.y)
                } else {
                    calc_line_angle(pm.x, pm.y, p1.x, p1.y)
                }
            },
            _ => 0.0,
        }
    } else if idx == segments.len() - 1 {
        // Last segment.

        let seg1 = segments[idx - 1];
        let seg2 = segments[idx];

        match (seg1, seg2) {
            (_, MarkerSegment::MoveTo(_)) => 0.0, // unreachable
            (_, MarkerSegment::LineTo(p)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_line_angle(prev.x, prev.y, p.x, p.y)
            },
            (_, MarkerSegment::CubicTo(p1, p2, p)) => {
                if p2.x.approx_eq_ulps(&p.x, 4) && p2.y.approx_eq_ulps(&p.y, 4) {
                    calc_line_angle(p1.x, p1.y, p.x, p.y)
                } else {
                    calc_line_angle(p2.x, p2.y, p.x, p.y)
                }
            },
            (MarkerSegment::LineTo(p), MarkerSegment::Close) => {
                let next = get_subpath_start(segments, idx);
                calc_line_angle(p.x, p.y, next.x, next.y)
            },
            (MarkerSegment::CubicTo(_, p2, p), MarkerSegment::Close) => {
                let prev = get_prev_vertex(segments, idx);
                let next = get_subpath_start(segments, idx);
                calc_curves_angle(
                    prev.x, prev.y, p2.x, p2.y, p.x, p.y, next.x, next.y, next.x, next.y,
                )
            },
            (_, MarkerSegment::Close) => 0.0,
        }
    } else {
        // Middle segments.

        let seg1 = segments[idx];
        let seg2 = segments[idx + 1];

        match (seg1, seg2) {
            (MarkerSegment::MoveTo(pm), MarkerSegment::LineTo(p)) => {
                calc_line_angle(pm.x, pm.y, p.x, p.y)
            },
            (MarkerSegment::MoveTo(pm), MarkerSegment::CubicTo(p1, _, _)) => {
                calc_line_angle(pm.x, pm.y, p1.x, p1.y)
            },
            (MarkerSegment::LineTo(p1), MarkerSegment::LineTo(p2)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_angle(prev.x, prev.y, p1.x, p1.y, p1.x, p1.y, p2.x, p2.y)
            },
            (MarkerSegment::CubicTo(_, c1_p2, c1_p), MarkerSegment::CubicTo(c2_p1, _, c2_p)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_curves_angle(
                    prev.x, prev.y, c1_p2.x, c1_p2.y, c1_p.x, c1_p.y, c2_p1.x, c2_p1.y, c2_p.x,
                    c2_p.y,
                )
            },
            (MarkerSegment::LineTo(pl), MarkerSegment::CubicTo(p1, _, p)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_curves_angle(
                    prev.x, prev.y, prev.x, prev.y, pl.x, pl.y, p1.x, p1.y, p.x, p.y,
                )
            },
            (MarkerSegment::CubicTo(_, p2, p), MarkerSegment::LineTo(pl)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_curves_angle(prev.x, prev.y, p2.x, p2.y, p.x, p.y, pl.x, pl.y, pl.x, pl.y)
            },
            (MarkerSegment::LineTo(p), MarkerSegment::MoveTo(_)) => {
                let prev = get_prev_vertex(segments, idx);
                calc_line_angle(prev.x, prev.y, p.x, p.y)
            },
            (MarkerSegment::CubicTo(_, p2, p), MarkerSegment::MoveTo(_)) => {
                if p.x.approx_eq_ulps(&p2.x, 4) && p.y.approx_eq_ulps(&p2.y, 4) {
                    let prev = get_prev_vertex(segments, idx);
                    calc_line_angle(prev.x, prev.y, p.x, p.y)
                } else {
                    calc_line_angle(p2.x, p2.y, p.x, p.y)
                }
            },
            (MarkerSegment::LineTo(p), MarkerSegment::Close) => {
                let prev = get_prev_vertex(segments, idx);
                let next = get_subpath_start(segments, idx);
                calc_angle(prev.x, prev.y, p.x, p.y, p.x, p.y, next.x, next.y)
            },
            (_, MarkerSegment::Close) => {
                let prev = get_prev_vertex(segments, idx);
                let next = get_subpath_start(segments, idx);
                calc_line_angle(prev.x, prev.y, next.x, next.y)
            },
            (_, MarkerSegment::MoveTo(_)) | (MarkerSegment::Close, _) => 0.0,
        }
    }
}

fn calc_line_angle(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    calc_angle(x1, y1, x2, y2, x1, y1, x2, y2)
}

fn calc_curves_angle(
    px: f32,
    py: f32, // previous vertex
    cx1: f32,
    cy1: f32, // previous control point
    x: f32,
    y: f32, // current vertex
    cx2: f32,
    cy2: f32, // next control point
    nx: f32,
    ny: f32, // next vertex
) -> f32 {
    if cx1.approx_eq_ulps(&x, 4) && cy1.approx_eq_ulps(&y, 4) {
        calc_angle(px, py, x, y, x, y, cx2, cy2)
    } else if x.approx_eq_ulps(&cx2, 4) && y.approx_eq_ulps(&cy2, 4) {
        calc_angle(cx1, cy1, x, y, x, y, nx, ny)
    } else {
        calc_angle(cx1, cy1, x, y, x, y, cx2, cy2)
    }
}

fn calc_angle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, x4: f32, y4: f32) -> f32 {
    use std::f32::consts::*;

    fn normalize(rad: f32) -> f32 {
        let v = rad % (PI * 2.0);
        if v < 0.0 { v + PI * 2.0 } else { v }
    }

    fn vector_angle(vx: f32, vy: f32) -> f32 {
        let rad = vy.atan2(vx);
        if rad.is_nan() { 0.0 } else { normalize(rad) }
    }

    let in_a = vector_angle(x2 - x1, y2 - y1);
    let out_a = vector_angle(x4 - x3, y4 - y3);
    let d = (out_a - in_a) * 0.5;

    let mut angle = in_a + d;
    if FRAC_PI_2 < d.abs() {
        angle -= PI;
    }

    normalize(angle).to_degrees()
}

pub(crate) fn get_subpath_start(segments: &[MarkerSegment], idx: usize) -> tiny_skia_path::Point {
    let offset = segments.len() - idx;
    for seg in segments.iter().rev().skip(offset) {
        if let MarkerSegment::MoveTo(p) = *seg {
            return p;
        }
    }

    tiny_skia_path::Point::zero()
}

fn get_prev_vertex(segments: &[MarkerSegment], idx: usize) -> tiny_skia_path::Point {
    match segments[idx - 1] {
        MarkerSegment::MoveTo(p) => p,
        MarkerSegment::LineTo(p) => p,
        MarkerSegment::CubicTo(_, _, p) => p,
        MarkerSegment::Close => get_subpath_start(segments, idx),
    }
}

fn quad_to_curve(
    prev: tiny_skia_path::Point,
    p1: tiny_skia_path::Point,
    p: tiny_skia_path::Point,
) -> (
    tiny_skia_path::Point,
    tiny_skia_path::Point,
    tiny_skia_path::Point,
) {
    fn calc(n1: f32, n2: f32) -> f32 {
        (n1 + n2 * 2.0) / 3.0
    }

    (
        tiny_skia_path::Point::from_xy(calc(prev.x, p1.x), calc(prev.y, p1.y)),
        tiny_skia_path::Point::from_xy(calc(p.x, p1.x), calc(p.y, p1.y)),
        p,
    )
}
