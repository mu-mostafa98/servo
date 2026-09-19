/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `clip-path` resolution: turns a `<clipPath>` element into a [`usvg::ClipPath`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg::{self, tiny_skia_path};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};

use crate::svg::primitives::attrs::{element_id, element_layout_type, length_attr_opt};
use crate::svg::primitives::shape::resolve_shape_path;
use crate::svg::usvg_builder::{SvgContext, build_text};

/// Monotonic counter for synthetic clip-path ids used by nested-`<svg>` viewport
/// clipping.
pub(crate) static SVG_CLIP_ID: AtomicU32 = AtomicU32::new(0);

/// Builds a synthetic `clipPath` containing a single rectangle, used to emulate a
/// nested `<svg>` viewport's `overflow` clipping.
pub(crate) fn rect_clip_path(rect: usvg::NonZeroRect) -> Option<Arc<usvg::ClipPath>> {
    let id = usvg::NonEmptyString::new(format!(
        "svg-clip-{}",
        SVG_CLIP_ID.fetch_add(1, Ordering::Relaxed)
    ))
    .expect("synthetic svg clip-path id is never empty");
    let mut clip_path = usvg::ClipPath::empty(id);

    let rect_path = tiny_skia_path::PathBuilder::from_rect(rect.to_rect());
    let mut p = usvg::Path::new_simple(Arc::new(rect_path))?;
    p.fill = Some(usvg::Fill::default());
    clip_path.root.children.push(usvg::Node::Path(Box::new(p)));

    Some(Arc::new(clip_path))
}

/// Returns the `id` referenced by an element's `clip-path="url(#id)"` attribute,
/// or `None` when the element has no clip path (or `clip-path="none"`).
fn clip_path_reference(element: &ServoLayoutElement<'_>) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from("clip-path"))?
        .trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = value.strip_prefix("url(")?.strip_suffix(')')?;
    let inner = inner.trim().trim_matches('"').trim_matches('\'');
    let id = inner.strip_prefix('#')?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Builds a [`usvg::ClipPath`] from a `<clipPath>` element, mirroring usvg's
/// `parser::clippath::convert`.
///
/// `object_bbox` is the bounding box of the element *being clipped* (used only by
/// `clipPathUnits="objectBoundingBox"`); `depth` guards against cyclic
/// `clip-path` references.
fn build_clip_path<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
    depth: usize,
) -> Option<Arc<usvg::ClipPath>> {
    if depth > 8 || element_layout_type(element) != LayoutElementType::SVGClipPathElement {
        return None;
    }
    let id_str = element_id(element)?;

    let mut transform = ctx.transform_attr(element);

    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("clipPathUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    if units == usvg::Units::ObjectBoundingBox {
        // The clip content is authored in `[0, 1]` bounding-box units; map it into
        // the object's bbox with a scale+translate transform.
        let object_bbox = object_bbox?;
        transform = transform.pre_concat(usvg::Transform::from_bbox(object_bbox));
    }

    // A `clipPath` may itself be clipped by another `clipPath` (`clip-path` on a
    // `clipPath`), producing an intersection of the two geometries.
    let clip_path = match clip_path_reference(element) {
        Some(ref_id) => {
            let linked = ctx.defs.get(&ref_id)?;
            Some(build_clip_path(linked, ctx, object_bbox, depth + 1)?)
        },
        None => None,
    };

    // Children are built in the clip path's own coordinate system (the document
    // user space for `userSpaceOnUse`, `[0, 1]` units for `objectBoundingBox`).
    // `transform` is applied at render time (`resvg::clip::apply`).
    let mut root = usvg::Group::empty();
    build_clip_path_children(element, ctx, usvg::Transform::identity(), &mut root);

    if !root.has_children() {
        return None;
    }

    Some(Arc::new(usvg::ClipPath::new(
        usvg::NonEmptyString::new(id_str)?,
        transform,
        clip_path,
        root,
    )))
}

/// Walks a `<clipPath>` element's children, building their geometry into `root`.
///
/// Clip-path children use *only* their geometry: shapes are filled black (their
/// `clip-rule` controlling the fill rule) and never stroked, mirroring usvg's
/// `resolve_fill`/`resolve_stroke` when `parent_clip_path` is set.
fn build_clip_path_children<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    root: &mut usvg::Group,
) {
    for child in element.as_node().dom_children() {
        for node in convert_clip_child(child, ctx, parent_abs_transform) {
            root.push_child(node);
        }
    }
}

/// Converts a single clip-path child (shape, group, `<use>`, or text) into clip
/// geometry nodes.
fn convert_clip_child<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let ty = element_layout_type(&element);

    if ty == LayoutElementType::SVGUseElement {
        return convert_clip_use(&element, ctx, parent_abs_transform);
    }

    let computed = ctx.computed_style(&element);

    // Per the SVG spec a `<clipPath>` may only contain basic shapes, `text` and
    // `use` (referencing those). A `<g>`/`<a>`/`<svg>` container is not a
    // conforming child: usvg's parser skips it (`is_graphic` excludes `G`/`A`/`Svg`),
    // leaving the clip path empty, and so do Chrome/Edge/ServoShell. Skip it here to
    // match — the referencing element will then be dropped by the caller.
    if matches!(
        ty,
        LayoutElementType::SVGGElement |
            LayoutElementType::SVGAElement |
            LayoutElementType::SVGSVGElement
    ) {
        return Vec::new();
    }

    // Text contributes its flattened glyph outlines (fill color is irrelevant for
    // the alpha-only clip mask).
    if ty == LayoutElementType::SVGTextElement {
        return build_text(&element, computed.as_deref(), ctx, parent_abs_transform)
            .into_iter()
            .collect();
    }

    // Shapes: geometry + a black fill (rule from `clip-rule`), never stroked.
    if let Some(path) = resolve_shape_path(&element, ty, computed.as_deref()) {
        let transform = ctx.transform_attr(&element);
        let abs_transform = parent_abs_transform.pre_concat(transform);

        let clip_rule = match element.attribute_as_str(&ns!(), &LocalName::from("clip-rule")) {
            Some(v) if v.eq_ignore_ascii_case("evenodd") => usvg::FillRule::EvenOdd,
            _ => usvg::FillRule::NonZero,
        };

        let mut fill = usvg::Fill::new(usvg::Paint::Color(usvg::Color::black()));
        fill.rule = clip_rule;

        let Some(path_node) = usvg::Path::new(
            element_id(&element).unwrap_or_default(),
            true,
            Some(fill),
            None,
            usvg::PaintOrder::default(),
            usvg::ShapeRendering::default(),
            Arc::new(path),
            abs_transform,
        )
        .map(|p| usvg::Node::Path(Box::new(p))) else {
            return Vec::new();
        };

        // A shape's own transform is carried by a wrapper group (resvg positions
        // path geometry with the accumulated group transform).
        if !transform.is_identity() {
            let mut group = usvg::Group::empty();
            group.transform = transform;
            group.abs_transform = abs_transform;
            group.push_child(path_node);
            return vec![usvg::Node::Group(Box::new(group))];
        }
        return vec![path_node];
    }

    Vec::new()
}

/// Resolves a `<use>` reference inside a clip path, appending the referenced
/// element's geometry with the `<use>` transform applied.
fn convert_clip_use<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(href) = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))
    else {
        return Vec::new();
    };
    let id = href.trim_start_matches('#');
    let Some(referenced) = ctx.defs.get(id) else {
        return Vec::new();
    };
    if referenced.as_node().opaque() == element.as_node().opaque() {
        return Vec::new();
    }

    let mut transform = ctx.transform_attr(element);
    let x = length_attr_opt(element, "x").unwrap_or(0.0);
    let y = length_attr_opt(element, "y").unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        transform = transform.pre_concat(usvg::Transform::from_translate(x, y));
    }
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let mut nodes = convert_clip_child(referenced.as_node(), ctx, abs_transform);

    if !transform.is_identity() {
        let mut group = usvg::Group::empty();
        group.transform = transform;
        group.abs_transform = abs_transform;
        for node in nodes.drain(..) {
            group.push_child(node);
        }
        nodes.push(usvg::Node::Group(Box::new(group)));
    }
    nodes
}

/// The outcome of resolving an element's `clip-path` reference.
pub(crate) enum ClipPathOutcome {
    /// No clip path: the attribute is absent, `none`, malformed, or a dangling
    /// reference — render the element normally (usvg's `attribute::<SvgNode>`
    /// returns `None` here, so no clip is applied).
    None,
    /// A valid clip path to apply.
    Clip(Arc<usvg::ClipPath>),
    /// The `clip-path` references a `clipPath` element that is empty or otherwise
    /// invalid — the element must be dropped entirely (usvg's `convert_group`
    /// returns `None` when `clippath::convert` fails).
    Invalid,
}

/// Resolves an element's `clip-path` reference into a [`usvg::ClipPath`], mirroring
/// usvg's `convert_group`/`clippath::convert` handshake: a dangling reference is
/// ignored (no clip), while a present-but-invalid `clipPath` drops the element.
///
/// `object_bbox` is the bounding box of the clipped element, required only by
/// `clipPathUnits="objectBoundingBox"` clip paths.
pub(crate) fn resolve_clip_path<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> ClipPathOutcome {
    let Some(ref_id) = clip_path_reference(element) else {
        return ClipPathOutcome::None;
    };
    let Some(linked) = ctx.defs.get(&ref_id) else {
        // Dangling id: not in the document, so treat as "no clip" like usvg.
        return ClipPathOutcome::None;
    };
    match build_clip_path(linked, ctx, object_bbox, 0) {
        Some(clip) => ClipPathOutcome::Clip(clip),
        None => ClipPathOutcome::Invalid,
    }
}
