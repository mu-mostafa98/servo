/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pure path geometry and alignment math.
//!
//! Everything here is Servo-free: it operates on `tiny_skia_path`, `kurbo` and
//! `svgtypes` values only, so it is the sole unit-testable leaf of the SVG layer.

use resvg::usvg::{self, tiny_skia_path};
use svgtypes::{Align, PointsParser, SimplePathSegment, SimplifyingPathParser};

/// The SVG "normalized diagonal" of a viewport, used as the reference length for
/// `<percentage>` values of `stroke-width`, `stroke-dasharray` and `stroke-dashoffset`.
pub(crate) fn normalized_diagonal(size: usvg::Size) -> f32 {
    (size.width() * size.width() + size.height() * size.height()).sqrt() / std::f32::consts::SQRT_2
}

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
