/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<marker>` placement: turns a shape's `marker-start`/`marker-mid`/`marker-end`
//! references into marker groups positioned and oriented at each vertex of the
//! shape's path.
//!
//! Markers are *not* an effect like clip/mask/filter — they don't wrap a node, they
//! emit sibling groups next to a shape's path. The whole subsystem (vertex/angle
//! math, reference resolution, and group assembly) therefore lives together here,
//! called only from [`super::path::build_shape`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use resvg::usvg::{self, ApproxEqUlps, ApproxZeroUlps, tiny_skia_path};
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::attrs::{length_attr, parse_view_box};

use super::{SvgContext, build_usvg_node};

/// A marker path segment. `QuadTo` is resolved to `CubicTo` up front (mirroring
/// usvg's `parser::marker::Segment`), since the vertex-tangent math only handles
/// lines and cubics.
#[derive(Copy, Clone, Debug)]
enum MarkerSegment {
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
enum MarkerKind {
    Start,
    Middle,
    End,
}

#[derive(Copy, Clone)]
enum MarkerOrientation {
    Auto,
    AutoStartReverse,
    Angle(f32),
}

/// Converts `path`'s segments into the marker-friendly [`MarkerSegment`] list.
fn build_marker_segments(path: &tiny_skia_path::Path) -> Vec<MarkerSegment> {
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

fn calc_vertex_angle(segments: &[MarkerSegment], idx: usize) -> f32 {
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

fn get_subpath_start(segments: &[MarkerSegment], idx: usize) -> tiny_skia_path::Point {
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

/// Monotonic id counter for the synthetic clip paths that `<marker>` overflow
/// clipping produces. The id only matters for SVG re-serialization; resvg keys clip
/// paths by pointer identity, so it just needs to be a valid non-empty string.
static MARKER_CLIP_ID: AtomicU32 = AtomicU32::new(0);

/// Whether `element` is a `<marker>` element. Markers are only referenced by `id`
/// (they are not rendered on their own and have no dedicated DOM type), so they are
/// discriminated by local name.
fn is_marker_element(element: &ServoLayoutElement<'_>) -> bool {
    element.local_name() == &LocalName::from("marker")
}

/// Builds the marker groups for `element`'s shape `path`, returning them as sibling
/// [`usvg::Node`]s to be placed after the path (the default paint order draws
/// markers last).
pub(super) fn build_markers<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    path: &tiny_skia_path::Path,
    stroke_width: f32,
    ctx: &SvgContext<'a, 'dom>,
    shape_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let kinds = [
        ("marker-start", MarkerKind::Start),
        ("marker-mid", MarkerKind::Middle),
        ("marker-end", MarkerKind::End),
    ];

    // Resolve the three marker references up front so the shared segment list is
    // only built when at least one marker is actually referenced.
    let mut refs: Vec<(MarkerKind, ServoLayoutElement<'dom>)> = Vec::new();
    for (attr, kind) in kinds {
        if let Some(marker) = marker_reference(element, attr)
            .and_then(|id| ctx.defs.get(&id))
            .copied()
            .filter(is_marker_element)
        {
            refs.push((kind, marker));
        }
    }

    if refs.is_empty() {
        return Vec::new();
    }

    let segments = build_marker_segments(path);

    let mut nodes = Vec::new();
    for (kind, marker) in refs {
        resolve_marker(
            &segments,
            marker,
            kind,
            stroke_width,
            ctx,
            shape_abs_transform,
            &mut nodes,
        );
    }
    nodes
}

/// Returns the `url(#fragment)` target of `attr` on `element`, falling back to the
/// `marker` shorthand attribute (which sets all three marker kinds at once).
fn marker_reference(element: &ServoLayoutElement<'_>, attr: &str) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .or_else(|| element.attribute_as_str(&ns!(), &LocalName::from("marker")))?;
    let value = value.trim();
    let inner = value.strip_prefix("url(")?.strip_suffix(")")?;
    let fragment = inner.trim().trim_start_matches('#');
    if fragment.is_empty() {
        None
    } else {
        Some(fragment.to_string())
    }
}

/// Places one `<marker>` at each vertex of `kind`, appending the resulting groups
/// to `out`. This mirrors usvg's `parser::marker::resolve`.
fn resolve_marker<'a, 'dom>(
    segments: &[MarkerSegment],
    marker: ServoLayoutElement<'dom>,
    kind: MarkerKind,
    stroke_width: f32,
    ctx: &SvgContext<'a, 'dom>,
    shape_abs_transform: usvg::Transform,
    out: &mut Vec<usvg::Node>,
) {
    let stroke_scale = match marker.attribute_as_str(&ns!(), &LocalName::from("markerUnits")) {
        Some("userSpaceOnUse") => 1.0,
        _ => stroke_width,
    };
    if stroke_scale <= 0.0 {
        return;
    }

    let Some(r) = marker_rect(&marker) else {
        return;
    };

    let view_box = parse_view_box(&marker);

    let has_overflow = match marker.attribute_as_str(&ns!(), &LocalName::from("overflow")) {
        Some("visible") | Some("auto") => false,
        _ => true,
    };

    let clip_path = if has_overflow {
        let clip_rect = view_box
            .map(|vb| vb.rect)
            .unwrap_or_else(|| r.size().to_non_zero_rect(0.0, 0.0));

        let id = usvg::NonEmptyString::new(format!(
            "marker-clip-{}",
            MARKER_CLIP_ID.fetch_add(1, Ordering::Relaxed)
        ))
        .expect("synthetic marker clip-path id is never empty");
        let mut clip_path = usvg::ClipPath::empty(id);

        let rect_path = tiny_skia_path::PathBuilder::from_rect(clip_rect.to_rect());
        let Some(mut p) = usvg::Path::new_simple(Arc::new(rect_path)) else {
            return;
        };
        p.fill = Some(usvg::Fill::default());
        clip_path.root.children.push(usvg::Node::Path(Box::new(p)));

        Some(Arc::new(clip_path))
    } else {
        None
    };

    let orientation = marker_orientation(&marker);

    let draw_marker = |p: tiny_skia_path::Point, idx: usize| {
        let mut ts = usvg::Transform::from_translate(p.x, p.y);

        let angle = match orientation {
            MarkerOrientation::AutoStartReverse if idx == 0 => {
                (calc_vertex_angle(segments, idx) + 180.0) % 360.0
            },
            MarkerOrientation::Auto | MarkerOrientation::AutoStartReverse => {
                calc_vertex_angle(segments, idx)
            },
            MarkerOrientation::Angle(angle) => angle,
        };

        if !angle.approx_zero_ulps(4) {
            ts = ts.pre_rotate(angle);
        }

        if let Some(vbox) = view_box {
            let size = usvg::Size::from_wh(r.width() * stroke_scale, r.height() * stroke_scale)
                .expect("marker size is positive and non-zero");
            let vbox_ts = vbox.to_transform(size);
            let (sx, sy) = vbox_ts.get_scale();
            ts = ts.pre_scale(sx, sy);
        } else {
            ts = ts.pre_scale(stroke_scale, stroke_scale);
        }

        ts = ts.pre_translate(-r.x(), -r.y());

        let abs_transform = shape_abs_transform.pre_concat(ts);
        if let Some(node) = build_marker(&marker, ts, abs_transform, clip_path.clone(), ctx) {
            out.push(node);
        }
    };

    draw_markers(segments, kind, draw_marker);
}

/// The marker `refX`/`refY`/`markerWidth`/`markerHeight` rectangle, which defines
/// the marker's viewport and its alignment point.
fn marker_rect(element: &ServoLayoutElement<'_>) -> Option<usvg::NonZeroRect> {
    usvg::NonZeroRect::from_xywh(
        length_attr(element, "refX", 0.0),
        length_attr(element, "refY", 0.0),
        length_attr(element, "markerWidth", 3.0),
        length_attr(element, "markerHeight", 3.0),
    )
}

fn marker_orientation(element: &ServoLayoutElement<'_>) -> MarkerOrientation {
    let Some(orient) = element.attribute_as_str(&ns!(), &LocalName::from("orient")) else {
        return MarkerOrientation::Angle(0.0);
    };
    match orient {
        "auto" => MarkerOrientation::Auto,
        "auto-start-reverse" => MarkerOrientation::AutoStartReverse,
        _ => orient
            .parse::<svgtypes::Angle>()
            .map(|a| MarkerOrientation::Angle(a.to_degrees() as f32))
            .unwrap_or(MarkerOrientation::Angle(0.0)),
    }
}

fn draw_markers<P>(segments: &[MarkerSegment], kind: MarkerKind, mut draw_marker: P)
where
    P: FnMut(tiny_skia_path::Point, usize),
{
    match kind {
        MarkerKind::Start => {
            if let Some(MarkerSegment::MoveTo(p)) = segments.first().copied() {
                draw_marker(p, 0);
            }
        },
        MarkerKind::Middle => {
            let total = segments.len().saturating_sub(1);
            let mut i = 1;
            while i < total {
                let p = match segments[i] {
                    MarkerSegment::MoveTo(p) => p,
                    MarkerSegment::LineTo(p) => p,
                    MarkerSegment::CubicTo(_, _, p) => p,
                    _ => {
                        i += 1;
                        continue;
                    },
                };

                draw_marker(p, i);

                i += 1;
            }
        },
        MarkerKind::End => {
            let idx = segments.len().saturating_sub(1);
            match segments.last().copied() {
                Some(MarkerSegment::LineTo(p)) => draw_marker(p, idx),
                Some(MarkerSegment::CubicTo(_, _, p)) => draw_marker(p, idx),
                Some(MarkerSegment::Close) => {
                    let p = get_subpath_start(segments, idx);
                    draw_marker(p, idx);
                },
                _ => {},
            }
        },
    }
}

/// Builds one `<marker>` group at the given transform, converting its children in
/// the marker's local coordinate system. Called from [`resolve_marker`] for each
/// placed vertex.
fn build_marker<'a, 'dom>(
    marker: &ServoLayoutElement<'dom>,
    ts: usvg::Transform,
    abs_transform: usvg::Transform,
    clip_path: Option<Arc<usvg::ClipPath>>,
    ctx: &SvgContext<'a, 'dom>,
) -> Option<usvg::Node> {
    let mut group = usvg::Group::empty();
    group.transform = ts;
    group.abs_transform = abs_transform;
    group.clip_path = clip_path;

    for child in marker.as_node().dom_children() {
        for child_node in build_usvg_node(child, ctx, group.abs_transform, None) {
            group.push_child(child_node);
        }
    }

    if group.has_children() {
        Some(usvg::Node::Group(Box::new(group)))
    } else {
        None
    }
}
