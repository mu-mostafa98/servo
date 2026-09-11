/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Marker placement: turns a shape's `marker-start`/`marker-mid`/`marker-end`
//! references into marker groups positioned at each vertex of the shape's path.
//!
//! The vertex/angle math lives in [`crate::svg::primitives::marker`]; the actual
//! construction of each marker group is delegated back to the builder via
//! [`crate::svg::usvg_builder::build_marker`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use html5ever::{LocalName, ns};
use layout_api::LayoutElement;
use resvg::usvg::{self, ApproxZeroUlps, tiny_skia_path};
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::attrs::{length_attr, parse_view_box};
use crate::svg::primitives::marker::{
    MarkerKind, MarkerOrientation, MarkerSegment, build_marker_segments, calc_vertex_angle,
    get_subpath_start,
};
use crate::svg::usvg_builder::{SvgContext, build_marker};

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
pub(crate) fn build_markers<'a, 'dom>(
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
