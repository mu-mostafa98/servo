/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Programmatic construction of a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! This is the Phase 2 bridge: instead of serializing the SVG subtree back to
//! XML (which discards the CSS cascade) and letting usvg re-parse it, we walk
//! the DOM on the layout thread — where computed styles are available — and
//! build the usvg tree directly via usvg's public constructors.
//!
//! Reading [`ComputedValues`] means `fill`, `stroke`, `opacity`, and the shape
//! geometry properties (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are taken from the
//! post-cascade result, so stylesheets and presentation attributes both apply.

use std::collections::HashMap;
use std::sync::Arc;

use std::hash::{Hash, Hasher};

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use net_traits::image_cache::ImageCache;
use resvg::usvg::{self, tiny_skia_path};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::dom::{OpaqueNode, TNode};
use style::properties::ComputedValues;
use style::values::computed::{LengthPercentage, NonNegativeLengthPercentageOrAuto};
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use style::values::generics::svg::SVGLength;
use svgtypes::{SimplePathSegment, SimplifyingPathParser, TransformListParser, TransformListToken};
use webrender_api::units::DeviceIntSize;
use webrender_api::ImageKey;

use crate::context::LayoutContext;

/// Paint servers referenced by `url(#id)` and collected from `<linearGradient>`
/// and `<radialGradient>` elements before the main tree walk.
#[derive(Default)]
struct Gradients {
    linear: HashMap<String, Arc<usvg::LinearGradient>>,
    radial: HashMap<String, Arc<usvg::RadialGradient>>,
}

/// Builds a [`usvg::Tree`] from the `<svg>` element at `node`.
///
/// Returns the tree together with the parsed [`usvg::ViewBox`] (if any). The
/// viewBox transform is deliberately *not* baked into the tree: content stays in
/// viewBox coordinates, and the viewBox→device mapping is applied at raster time
/// (see [`rasterize_svg_tree`]) so `preserveAspectRatio` is honoured uniformly
/// rather than being distorted by a non-uniform CSS-box stretch.
///
/// Returns `None` when `node` is not an `<svg>` element or the tree would be
/// empty/invalid.
#[expect(unsafe_code)]
pub(crate) fn build_usvg_tree(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
) -> Option<(usvg::Tree, Option<usvg::ViewBox>)> {
    let element = node.as_element()?;
    if element.local_name() != &LocalName::from("svg") {
        return None;
    }

    let (size, view_box) = resolve_size_and_view_box(&element)?;

    // Collect gradient definitions up front so that `fill`/`stroke` referencing
    // them can be resolved during the main walk.
    let mut gradients = Gradients::default();
    for child in node.dom_children() {
        collect_gradients(child, context, &mut gradients);
    }

    // Collect every `id`'d element in the document so `<use href="#id">` can be
    // resolved to its referenced element (which may live in a sibling subtree).
    let document = unsafe { node.dangerous_style_node() }.owner_doc();
    let mut defs = HashMap::new();
    if let Some(root_element) = document.root_element() {
        collect_element_ids(root_element.as_node(), &mut defs);
    }

    // Children are built in viewBox (user) coordinates; the viewBox transform is
    // applied later, at raster time.
    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        if let Some(child_node) = convert_node(
            child,
            context,
            &gradients,
            &defs,
            usvg::Transform::identity(),
            None,
        ) {
            root.push_child(child_node);
        }
    }

    let mut tree = usvg::Tree::new(size, root);
    tree.finalize();
    Some((tree, view_box))
}

/// Determines the image size and view box from the root `<svg>` element.
fn resolve_size_and_view_box(
    element: &ServoLayoutElement<'_>,
) -> Option<(usvg::Size, Option<usvg::ViewBox>)> {
    let view_box = parse_view_box(element);

    let width = element
        .attribute_as_str(&ns!(), &LocalName::from("width"))
        .and_then(parse_length_attr);
    let height = element
        .attribute_as_str(&ns!(), &LocalName::from("height"))
        .and_then(parse_length_attr);

    let size = match (width, height, view_box.map(|vb| vb.rect)) {
        (Some(w), Some(h), _) => usvg::Size::from_wh(w, h),
        (Some(w), None, Some(vb)) => usvg::Size::from_wh(w, vb.height() * w / vb.width()),
        (None, Some(h), Some(vb)) => usvg::Size::from_wh(vb.width() * h / vb.height(), h),
        (None, None, Some(vb)) => usvg::Size::from_wh(vb.width(), vb.height()),
        _ => usvg::Size::from_wh(100.0, 100.0),
    }?;

    Some((size, view_box))
}

fn parse_view_box(element: &ServoLayoutElement<'_>) -> Option<usvg::ViewBox> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from("viewBox"))?;
    let mut nums = value.split(|c: char| c.is_ascii_whitespace() || c == ',');
    let x = nums.next()?.trim().parse::<f32>().ok()?;
    let y = nums.next()?.trim().parse::<f32>().ok()?;
    let w = nums.next()?.trim().parse::<f32>().ok()?;
    let h = nums.next()?.trim().parse::<f32>().ok()?;
    let rect = usvg::NonZeroRect::from_xywh(x, y, w, h)?;

    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();

    Some(usvg::ViewBox { rect, aspect })
}

/// Collects `<linearGradient>`/`<radialGradient>` definitions (and their
/// `<stop>` children) into `gradients`.
fn collect_gradients(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &mut Gradients,
) {
    let Some(element) = node.as_element() else {
        return;
    };
    let name = element.local_name().clone();
    if name == LocalName::from("linearGradient") {
        if let (Some(id), Some(grad)) = (element_id(&element), build_linear_gradient(&element)) {
            gradients.linear.insert(id, Arc::new(grad));
        }
        return;
    }
    if name == LocalName::from("radialGradient") {
        if let (Some(id), Some(grad)) = (element_id(&element), build_radial_gradient(&element)) {
            gradients.radial.insert(id, Arc::new(grad));
        }
        return;
    }

    for child in node.dom_children() {
        collect_gradients(child, context, gradients);
    }
}

fn element_id(element: &ServoLayoutElement<'_>) -> Option<String> {
    let id = element.attribute_as_str(&ns!(), &LocalName::from("id"))?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Recursively collects every element with a non-empty `id` attribute into `map`,
/// keyed by that id. Used to resolve `<use href="#id">` references anywhere in the
/// document, not just within the current `<svg>` subtree.
fn collect_element_ids<'a>(
    node: ServoLayoutNode<'a>,
    map: &mut HashMap<String, ServoLayoutElement<'a>>,
) {
    if let Some(element) = node.as_element() &&
        let Some(id) = element_id(&element)
    {
        map.entry(id).or_insert(element);
    }
    for child in node.dom_children() {
        collect_element_ids(child, map);
    }
}

fn build_linear_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::LinearGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let x1 = length_attr(element, "x1", 0.0);
    let y1 = length_attr(element, "y1", 0.0);
    let x2 = length_attr(element, "x2", 1.0);
    let y2 = length_attr(element, "y2", 0.0);
    let base = build_base_gradient(element, id)?;
    Some(usvg::LinearGradient::new(base, x1, y1, x2, y2))
}

fn build_radial_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::RadialGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let cx = length_attr(element, "cx", 0.5);
    let cy = length_attr(element, "cy", 0.5);
    let r = usvg::PositiveF32::new(length_attr(element, "r", 0.5))?;
    let fx = length_attr(element, "fx", cx);
    let fy = length_attr(element, "fy", cy);
    let fr = usvg::PositiveF32::new(length_attr(element, "fr", 0.0))?;
    let base = build_base_gradient(element, id)?;
    Some(usvg::RadialGradient::new(base, cx, cy, r, fx, fy, fr))
}

fn build_base_gradient(
    element: &ServoLayoutElement<'_>,
    id: usvg::NonEmptyString,
) -> Option<usvg::BaseGradient> {
    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("gradientUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("gradientTransform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let spread_method = match element
        .attribute_as_str(&ns!(), &LocalName::from("spreadMethod"))
        .unwrap_or("pad")
    {
        "reflect" => usvg::SpreadMethod::Reflect,
        "repeat" => usvg::SpreadMethod::Repeat,
        _ => usvg::SpreadMethod::Pad,
    };

    let mut stops = Vec::new();
    for child in element.as_node().dom_children() {
        if let Some(stop) = child.as_element().and_then(|e| build_stop(&e)) {
            stops.push(stop);
        }
    }

    Some(usvg::BaseGradient::new(
        id,
        units,
        transform,
        spread_method,
        stops,
    ))
}

fn build_stop(element: &ServoLayoutElement<'_>) -> Option<usvg::Stop> {
    if element.local_name() != &LocalName::from("stop") {
        return None;
    }
    let offset_raw = element.attribute_as_str(&ns!(), &LocalName::from("offset"))?;
    let offset = if let Some(pct) = offset_raw.strip_suffix('%') {
        pct.parse::<f32>().ok()? / 100.0
    } else {
        offset_raw.parse::<f32>().ok()?
    };
    let offset = usvg::StopOffset::new(offset.clamp(0.0, 1.0))?;

    let color = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-color"))
        .map(parse_color)
        .unwrap_or_else(usvg::Color::black);
    let opacity = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-opacity"))
        .and_then(|s| s.parse::<f32>().ok())
        .and_then(usvg::Opacity::new)
        .unwrap_or(usvg::Opacity::ONE);

    Some(usvg::Stop::new(offset, color, opacity))
}

/// Converts a DOM node (and its subtree) into a [`usvg::Node`].
///
/// `defs` is a document-wide `id`→element map used to resolve `<use>` references.
/// `host` is the computed style of the enclosing `<use>` element (if any): it is
/// used as the inheritance parent for paint, since `<use>` shadow content inherits
/// from the `<use>` host rather than from its location in the `<defs>`.
fn convert_node(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let name = element.local_name().clone();

    if is_group_element(&name) {
        return convert_group(node, context, gradients, defs, parent_abs_transform, host);
    }

    if name.as_ref() == "use" {
        return convert_use(node, context, gradients, defs, parent_abs_transform);
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    if let Some(shape) = build_shape_node(
        &element,
        &name,
        computed.as_deref(),
        host,
        gradients,
        parent_abs_transform,
    ) {
        return Some(shape);
    }

    // Elements we don't handle yet (text, image, …) are silently skipped.
    None
}

fn is_group_element(name: &LocalName) -> bool {
    matches!(
        name.as_ref(),
        "svg" | "g" | "defs" | "symbol" | "a" | "clipPath" | "mask" | "pattern"
    )
}

fn convert_group(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    if let Some(computed) = computed.as_deref() {
        group.opacity = usvg::Opacity::new(computed.get_effects().opacity)
            .unwrap_or(usvg::Opacity::ONE);
    }

    for child in node.dom_children() {
        if let Some(child_node) =
            convert_node(child, context, gradients, defs, abs_transform, host)
        {
            group.push_child(child_node);
        }
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Converts a `<use href="#id">` element by cloning the referenced element's
/// subtree into a group that carries the `<use>` element's transform and opacity.
/// The referenced content inherits paint from the `<use>` host (shadow-tree
/// semantics) via the `host` parameter passed down to [`build_shape_node`].
fn convert_use(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;

    let href = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))?;
    let id = href.trim_start_matches('#');
    if id.is_empty() {
        return None;
    }

    let referenced = defs.get(id)?;
    if referenced.as_node().opaque() == node.opaque() {
        return None;
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let mut transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        transform = transform.pre_concat(usvg::Transform::from_translate(x, y));
    }
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    if let Some(computed) = computed.as_deref() {
        group.opacity = usvg::Opacity::new(computed.get_effects().opacity)
            .unwrap_or(usvg::Opacity::ONE);
    }

    let referenced_node = referenced.as_node();
    if let Some(child_node) = convert_node(
        referenced_node,
        context,
        gradients,
        defs,
        abs_transform,
        computed.as_deref(),
    ) {
        group.push_child(child_node);
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Whether an element explicitly sets its own paint, in which case it should not
/// inherit fill/stroke from an enclosing `<use>` host.
fn element_has_explicit_paint(element: &ServoLayoutElement<'_>) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("fill"))
        .is_some()
        || element
            .attribute_as_str(&ns!(), &LocalName::from("stroke"))
            .is_some()
        || element
            .attribute_as_str(&ns!(), &LocalName::from("style"))
            .is_some()
}

fn build_shape_node(
    element: &ServoLayoutElement<'_>,
    name: &LocalName,
    computed: Option<&ComputedValues>,
    host: Option<&ComputedValues>,
    gradients: &Gradients,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let data = build_shape_path(element, name, computed)?;

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
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

    // When this shape is reachable through a `<use>`, its paint inherits from the
    // `<use>` host unless it explicitly sets `fill`/`stroke` (or inline `style`).
    let paint_computed = match host {
        Some(_) if !element_has_explicit_paint(element) => host,
        _ => computed,
    };
    let fill = paint_computed.and_then(|c| build_fill(c, gradients));
    let stroke = paint_computed.and_then(|c| build_stroke(c, gradients));

    usvg::Path::new(
        id,
        visible,
        fill,
        stroke,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::default(),
        Arc::new(data),
        abs_transform,
    )
    .map(|p| usvg::Node::Path(Box::new(p)))
}

/// Builds the path geometry for a shape element. Geometry properties that are
/// CSS longhands (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are read from `computed`;
/// the rest (`width`/`height`/`x1`/`y1`/`x2`/`y2`/`points`/`d`) fall back to
/// attributes.
fn build_shape_path(
    element: &ServoLayoutElement<'_>,
    name: &LocalName,
    computed: Option<&ComputedValues>,
) -> Option<tiny_skia_path::Path> {
    match name.as_ref() {
        "rect" => {
            let (x, y, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_x()),
                        lp_to_f32(&svg.clone_y()),
                        lp_or_auto_to_f32(&svg.clone_rx()),
                        lp_or_auto_to_f32(&svg.clone_ry()),
                    )
                },
                None => (
                    length_attr(element, "x", 0.0),
                    length_attr(element, "y", 0.0),
                    length_attr_opt(element, "rx"),
                    length_attr_opt(element, "ry"),
                ),
            };
            let w = length_attr(element, "width", 0.0);
            let h = length_attr(element, "height", 0.0);
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            let (rx, ry) = match (rx, ry) {
                (Some(rx), Some(ry)) => (rx.min(w / 2.0), ry.min(h / 2.0)),
                (Some(rx), None) => {
                    let r = rx.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, Some(ry)) => {
                    let r = ry.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, None) => (0.0, 0.0),
            };
            rounded_rect(&mut pb, x, y, w, h, rx, ry);
            pb.finish()
        },
        "circle" => {
            let (cx, cy, r) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_to_f32(&svg.clone_r().0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "r", 0.0),
                ),
            };
            if r <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            ellipse(&mut pb, cx, cy, r, r);
            pb.finish()
        },
        "ellipse" => {
            let (cx, cy, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_or_auto_to_f32(&svg.clone_rx()).unwrap_or(0.0),
                        lp_or_auto_to_f32(&svg.clone_ry()).unwrap_or(0.0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "rx", 0.0),
                    length_attr(element, "ry", 0.0),
                ),
            };
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            ellipse(&mut pb, cx, cy, rx, ry);
            pb.finish()
        },
        "line" => {
            let x1 = length_attr(element, "x1", 0.0);
            let y1 = length_attr(element, "y1", 0.0);
            let x2 = length_attr(element, "x2", 0.0);
            let y2 = length_attr(element, "y2", 0.0);
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            pb.finish()
        },
        "polyline" => polygon_points(element, "points", false),
        "polygon" => polygon_points(element, "points", true),
        "path" => element
            .attribute_as_str(&ns!(), &LocalName::from("d"))
            .and_then(parse_path_d),
        _ => None,
    }
}

fn polygon_points(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    close: bool,
) -> Option<tiny_skia_path::Path> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from(attr))?;
    let mut nums = value.split(|c: char| c.is_ascii_whitespace() || c == ',');
    let mut pb = tiny_skia_path::PathBuilder::new();
    let mut first = true;
    let mut has_point = false;
    loop {
        let x = nums.next().and_then(|s| s.trim().parse::<f32>().ok());
        let y = nums.next().and_then(|s| s.trim().parse::<f32>().ok());
        match (x, y) {
            (Some(x), Some(y)) => {
                if first {
                    pb.move_to(x, y);
                    first = false;
                } else {
                    pb.line_to(x, y);
                }
                has_point = true;
            },
            _ => break,
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

fn ellipse(pb: &mut tiny_skia_path::PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32) {
    const K: f32 = 0.5522847498;
    let ox = rx * K;
    let oy = ry * K;
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.close();
}

fn rounded_rect(
    pb: &mut tiny_skia_path::PathBuilder,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rx: f32,
    ry: f32,
) {
    if rx <= 0.0 || ry <= 0.0 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return;
    }
    const K: f32 = 0.5522847498;
    pb.move_to(x + rx, y);
    pb.line_to(x + w - rx, y);
    pb.cubic_to(x + w - rx + rx * K, y, x + w, y + ry - ry * K, x + w, y + ry);
    pb.line_to(x + w, y + h - ry);
    pb.cubic_to(x + w, y + h - ry + ry * K, x + w - rx + rx * K, y + h, x + w - rx, y + h);
    pb.line_to(x + rx, y + h);
    pb.cubic_to(x + rx - rx * K, y + h, x, y + h - ry + ry * K, x, y + h - ry);
    pb.line_to(x, y + ry);
    pb.cubic_to(x, y + ry - ry * K, x + rx - rx * K, y, x + rx, y);
    pb.close();
}

fn parse_path_d(d: &str) -> Option<tiny_skia_path::Path> {
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

fn build_fill(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.fill, computed, gradients)?;

    let opacity = match inherited.fill_opacity {
        SVGOpacity::Opacity(op) => op,
        _ => 1.0,
    };
    let rule = match inherited.fill_rule {
        style::computed_values::fill_rule::T::Evenodd => usvg::FillRule::EvenOdd,
        _ => usvg::FillRule::NonZero,
    };

    let mut fill = usvg::Fill::new(paint);
    fill.opacity = usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE);
    fill.rule = rule;
    Some(fill)
}

fn build_stroke(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.stroke, computed, gradients)?;

    let width = match &inherited.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0),
        _ => 1.0,
    };
    if width <= 0.0 {
        return None;
    }

    let mut stroke = usvg::Stroke::new(paint);
    stroke.width = usvg::StrokeWidth::new(width).unwrap_or(usvg::StrokeWidth::new(1.0).unwrap());
    stroke.opacity = match inherited.stroke_opacity {
        SVGOpacity::Opacity(op) => usvg::Opacity::new(op).unwrap_or(usvg::Opacity::ONE),
        _ => usvg::Opacity::ONE,
    };
    stroke.linecap = match inherited.stroke_linecap {
        style::computed_values::stroke_linecap::T::Round => usvg::LineCap::Round,
        style::computed_values::stroke_linecap::T::Square => usvg::LineCap::Square,
        _ => usvg::LineCap::Butt,
    };
    stroke.linejoin = match inherited.stroke_linejoin {
        style::computed_values::stroke_linejoin::T::Round => usvg::LineJoin::Round,
        style::computed_values::stroke_linejoin::T::Bevel => usvg::LineJoin::Bevel,
        _ => usvg::LineJoin::Miter,
    };
    stroke.miterlimit = usvg::StrokeMiterlimit::new(inherited.stroke_miterlimit.0);
    if let SVGStrokeDashArray::Values(vs) = &inherited.stroke_dasharray {
        if !vs.is_empty() {
            stroke.dasharray = Some(
                vs.iter()
                    .map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0))
                    .collect(),
            );
        }
    }
    stroke.dashoffset = match &inherited.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => lp.to_length().map(|l| l.px()).unwrap_or(0.0),
        _ => 0.0,
    };

    Some(stroke)
}

fn resolve_paint(
    svg_paint: &SVGPaint,
    computed: &ComputedValues,
    gradients: &Gradients,
) -> Option<usvg::Paint> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(usvg::Paint::Color(usvg::Color::new_rgb(
                (srgb.components.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.2.clamp(0.0, 1.0) * 255.0).round() as u8,
            )))
        },
        SVGPaintKind::None => None,
        SVGPaintKind::PaintServer(url) => {
            let fragment = match url {
                style::url::ComputedUrl::Valid(u) => u.fragment().map(|s| s.to_string()),
                style::url::ComputedUrl::Invalid(s) => {
                    let trimmed = s.trim_start_matches('#');
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                },
            };
            let fragment = fragment?;
            if let Some(g) = gradients.linear.get(&fragment) {
                Some(usvg::Paint::LinearGradient(g.clone()))
            } else if let Some(g) = gradients.radial.get(&fragment) {
                Some(usvg::Paint::RadialGradient(g.clone()))
            } else {
                // Unresolved paint server: fall back to black, matching usvg's
                // behaviour when a referenced gradient is missing.
                Some(usvg::Paint::Color(usvg::Color::black()))
            }
        },
        _ => None,
    }
}

fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

fn lp_or_auto_to_f32(lp: &NonNegativeLengthPercentageOrAuto) -> Option<f32> {
    match lp {
        GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
            Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0))
        },
        GenericLengthPercentageOrAuto::Auto => None,
    }
}

/// Parses a length attribute, returning `default` when missing or unparseable.
fn length_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    length_attr_opt(element, attr).unwrap_or(default)
}

fn length_attr_opt(element: &ServoLayoutElement<'_>, attr: &str) -> Option<f32> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(parse_length_attr)
}

fn parse_length_attr(value: &str) -> Option<f32> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value.parse::<f32>().ok()
}

fn parse_transform(value: &str) -> usvg::Transform {
    let mut transform = usvg::Transform::identity();
    for token in TransformListParser::from(value) {
        let Ok(token) = token else {
            continue;
        };
        let t = match token {
            TransformListToken::Matrix { a, b, c, d, e, f } => usvg::Transform::from_row(
                a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
            ),
            TransformListToken::Translate { tx, ty } => {
                usvg::Transform::from_translate(tx as f32, ty as f32)
            },
            TransformListToken::Scale { sx, sy } => usvg::Transform::from_scale(sx as f32, sy as f32),
            TransformListToken::Rotate { angle } => {
                usvg::Transform::from_rotate(angle as f32)
            },
            TransformListToken::SkewX { angle } => {
                usvg::Transform::from_row(1.0, 0.0, (angle as f32).to_radians().tan(), 1.0, 0.0, 0.0)
            },
            TransformListToken::SkewY { angle } => {
                usvg::Transform::from_row(1.0, (angle as f32).to_radians().tan(), 0.0, 1.0, 0.0, 0.0)
            },
        };
        transform = transform.pre_concat(t);
    }
    transform
}

fn parse_color(value: &str) -> usvg::Color {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            return usvg::Color::new_rgb(r, g, b);
        }
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            return usvg::Color::new_rgb(r, g, b);
        }
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let mut nums = inner.split(',');
        let r = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        let g = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        let b = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        return usvg::Color::new_rgb(r, g, b);
    }
    match value {
        "black" => usvg::Color::black(),
        "white" => usvg::Color::white(),
        "red" => usvg::Color::new_rgb(255, 0, 0),
        "green" => usvg::Color::new_rgb(0, 128, 0),
        "blue" => usvg::Color::new_rgb(0, 0, 255),
        "none" | "transparent" => usvg::Color::new_rgb(0, 0, 0),
        _ => usvg::Color::black(),
    }
}

/// Rasterizes `tree` into an RGBA8 pixmap at `raster_size`, uploads the raw pixels to
/// WebRender keyed by `(node, size)`, and returns the resulting [`ImageKey`].
///
/// This runs synchronously on the layout thread, replacing the asynchronous
/// vector-image cache rasterization path for inline SVGs built from the DOM.
///
/// When `view_box` is present the content is in viewBox coordinates, so the
/// viewBox→device transform (`preserveAspectRatio`-aware) is applied directly.
/// Otherwise the content is in viewport coordinates and a simple non-uniform
/// stretch from the tree's natural size to the device box is used.
pub(crate) fn rasterize_svg_tree(
    image_cache: &dyn ImageCache,
    tree: &usvg::Tree,
    node: OpaqueNode,
    raster_size: DeviceIntSize,
    view_box: Option<usvg::ViewBox>,
) -> Option<ImageKey> {
    const MAX_SVG_PIXMAP_DIMENSION: i32 = 5000;

    let width = raster_size.width.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let height = raster_size.height.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let img_size = usvg::Size::from_wh(width as f32, height as f32)?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

    let transform = match view_box {
        Some(vb) => vb.to_transform(img_size),
        None => {
            let natural_size = tree.size().to_int_size();
            if natural_size.width() == 0 || natural_size.height() == 0 {
                return None;
            }
            usvg::Transform::from_scale(
                width as f32 / natural_size.width() as f32,
                height as f32 / natural_size.height() as f32,
            )
        },
    };

    resvg::render(tree, transform, &mut pixmap.as_mut());
    let bytes = pixmap.take();

    // Key by (DOM node, raster size) so a re-layout of the same SVG after a mutation
    // updates the pixels in place rather than leaking a new WebRender image key.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.id().hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    let hash = hasher.finish();

    image_cache.upload_raw_pixels(hash, bytes, width, height);
    image_cache.raw_pixel_image_key(hash)
}
