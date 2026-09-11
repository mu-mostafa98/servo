/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Basic shape elements (`rect`/`circle`/`ellipse`/`line`/`polyline`/`polygon`/
//! `path`) and marker placement.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg::{self, ApproxZeroUlps, tiny_skia_path};
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;
use style::values::computed::Length;
use style::values::generics::svg::SVGLength;

use crate::svg::builder::{SvgContext, convert_node};
use crate::svg::effects::clip::{ClipPathOutcome, resolve_clip_path};
use crate::svg::effects::filter::{FilterOutcome, resolve_filter};
use crate::svg::effects::mask::{MaskOutcome, resolve_mask};
use crate::svg::effects::paint::{build_fill, build_stroke};
use crate::svg::primitives::attrs::{
    element_has_explicit_fill, element_has_explicit_stroke, element_id, length_attr, parse_view_box,
};
use crate::svg::primitives::geometry::{
    MarkerKind, MarkerOrientation, MarkerSegment, build_marker_segments, calc_vertex_angle,
    get_subpath_start,
};
use crate::svg::primitives::shape::build_shape_path;

/// Monotonic id counter for the synthetic clip paths that `<marker>` overflow
/// clipping produces. The id only matters for SVG re-serialization; resvg keys clip
/// paths by pointer identity, so it just needs to be a valid non-empty string.
static MARKER_CLIP_ID: AtomicU32 = AtomicU32::new(0);

/// Builds the path geometry + paint + markers for a basic shape element.
pub(crate) fn build_shape_node<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    host: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(data) = build_shape_path(element, ty, computed) else {
        return Vec::new();
    };

    let transform = ctx.transform_attr(element);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let id = element_id(element).unwrap_or_default();
    let visible = computed
        .map(|c| {
            !matches!(
                c.get_inherited_box().visibility,
                style::computed_values::visibility::T::Hidden |
                    style::computed_values::visibility::T::Collapse
            )
        })
        .unwrap_or(true);

    // When this shape is reachable through a `<use>`, its `fill` and `stroke`
    // inherit from the `<use>` host independently: each is taken from the host
    // unless this element explicitly sets it (attribute or inline `style`). This
    // matters for `<use href="#path" stroke="…">` guides, where the referenced
    // path may set `fill="none"` but leave `stroke` to be inherited.
    let fill_computed = match host {
        Some(_) if !element_has_explicit_fill(element) => host,
        _ => computed,
    };
    let stroke_computed = match host {
        Some(_) if !element_has_explicit_stroke(element) => host,
        _ => computed,
    };
    let fill = fill_computed.and_then(|c| build_fill(c, ctx.gradients));
    let stroke = stroke_computed.and_then(|c| build_stroke(c, ctx.gradients, ctx.diagonal));

    // Marker scaling uses the stroke width in `markerUnits="strokeWidth"` mode
    // (the default). This is the raw stroke width, resolved independently of
    // whether a stroke is actually painted (`stroke="none"` still scales markers
    // at the default 1.0).
    let stroke_width = stroke_computed
        .map(|c| {
            let inherited = c.get_inherited_svg();
            match &inherited.stroke_width {
                SVGLength::LengthPercentage(nn_lp) => {
                    nn_lp.0.resolve(Length::new(ctx.diagonal)).px()
                },
                _ => 1.0,
            }
        })
        .unwrap_or(1.0);

    let Some(path_node) = usvg::Path::new(
        id,
        visible,
        fill,
        stroke,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::default(),
        Arc::new(data.clone()),
        abs_transform,
    )
    .map(|p| usvg::Node::Path(Box::new(p))) else {
        return Vec::new();
    };

    let mut nodes = vec![path_node];
    nodes.extend(build_markers(
        element,
        &data,
        stroke_width,
        ctx,
        abs_transform,
    ));

    // A shape's `transform` and `opacity` attributes cannot be folded into the
    // path: `usvg::Path` has no local `transform` field, and resvg positions path
    // geometry with the *accumulated group* transform (it never reads
    // `path.abs_transform()` for positioning). So when a shape sets its own
    // transform (or an `opacity < 1`), wrap the path + markers in a group that
    // carries them — mirroring usvg's `convert_group` wrapper. The path keeps its
    // id; the wrapper group is anonymous (usvg only ids the element for `<g>`/`<use>`).
    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let object_bbox = data.bounds().to_non_zero_rect();
    let clip_path = match resolve_clip_path(element, ctx, object_bbox) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return Vec::new(),
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, ctx, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    let filter = match resolve_filter(element, ctx, object_bbox) {
        FilterOutcome::Filter(filter) => Some(filter),
        FilterOutcome::Invalid => return Vec::new(),
        FilterOutcome::None => None,
    };
    if !transform.is_identity() ||
        element_opacity < 1.0 ||
        clip_path.is_some() ||
        mask.is_some() ||
        filter.is_some()
    {
        let mut group = usvg::Group::empty();
        group.transform = transform;
        group.abs_transform = abs_transform;
        group.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        group.clip_path = clip_path;
        group.mask = mask;
        if let Some(filter) = filter {
            group.filters.push(filter);
        }
        for node in nodes {
            group.push_child(node);
        }
        vec![usvg::Node::Group(Box::new(group))]
    } else {
        nodes
    }
}

/// Whether `element` is a `<marker>` element. Markers are only referenced by `id`
/// (they are not rendered on their own and have no dedicated DOM type), so they are
/// discriminated by local name.
fn is_marker_element(element: &ServoLayoutElement<'_>) -> bool {
    element.local_name() == &LocalName::from("marker")
}

/// Builds the marker groups for `element`'s shape `path`, returning them as sibling
/// [`usvg::Node`]s to be placed after the path (the default paint order draws
/// markers last).
fn build_markers<'a, 'dom>(
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

        let mut g = usvg::Group::empty();
        g.transform = ts;
        g.abs_transform = shape_abs_transform.pre_concat(ts);
        g.clip_path = clip_path.clone();

        // Marker content is converted in the marker's local coordinate system.
        for child in marker.as_node().dom_children() {
            for child_node in convert_node(child, ctx, g.abs_transform, None) {
                g.push_child(child_node);
            }
        }

        if g.has_children() {
            out.push(usvg::Node::Group(Box::new(g)));
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
