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
use layout_api::{LayoutElement, LayoutElementType, LayoutNode, LayoutNodeType};
use net_traits::image_cache::ImageCache;
use resvg::usvg::{self, tiny_skia_path};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::dom::{OpaqueNode, TNode};
use style::properties::ComputedValues;
use style::values::computed::{Length, LengthPercentage, NonNegativeLengthPercentageOrAuto};
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray, VectorEffect,
};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use style::values::generics::svg::SVGLength;
use svgtypes::{
    LengthUnit, PointsParser, SimplePathSegment, SimplifyingPathParser, TransformListParser,
    TransformListToken,
};
use webrender_api::units::DeviceIntSize;
use webrender_api::ImageKey;

use crate::context::LayoutContext;

/// Paint servers referenced by `url(#id)` and collected from
/// `<linearGradient>`, `<radialGradient>` and `<pattern>` elements before the
/// main tree walk.
#[derive(Default)]
struct Gradients {
    linear: HashMap<String, Arc<usvg::LinearGradient>>,
    radial: HashMap<String, Arc<usvg::RadialGradient>>,
    pattern: HashMap<String, Arc<usvg::Pattern>>,
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
    if element_layout_type(&element) != LayoutElementType::SVGSVGElement {
        return None;
    }

    let (size, view_box) = resolve_size_and_view_box(&element)?;

    // Reference length for `<percentage>` stroke-width/dash values: the SVG
    // "normalized diagonal" of the viewport, √(w² + h²) / √2.
    let diagonal = normalized_diagonal(size);

    // Collect every `id`'d element in the document so `<use href="#id">` can be
    // resolved to its referenced element (which may live in a sibling subtree).
    let document = unsafe { node.dangerous_style_node() }.owner_doc();
    let mut defs = HashMap::new();
    if let Some(root_element) = document.root_element() {
        collect_element_ids(root_element.as_node(), &mut defs);
    }

    // Collect paint-server definitions up front so that `fill`/`stroke`
    // referencing them can be resolved during the main walk. Gradients are
    // collected first; patterns are built second so their content can reference
    // the already-collected gradients.
    let mut gradients = Gradients::default();
    let mut pattern_elements = Vec::new();
    for child in node.dom_children() {
        collect_paint_servers(child, &mut gradients, &mut pattern_elements);
    }
    for element in &pattern_elements {
        if let Some(id) = element_id(element) {
            if let Some(pattern) = build_pattern(element, context, &gradients, &defs, diagonal) {
                gradients.pattern.insert(id, Arc::new(pattern));
            }
        }
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
            diagonal,
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

/// The SVG "normalized diagonal" of a viewport, used as the reference length for
/// `<percentage>` values of `stroke-width`, `stroke-dasharray` and `stroke-dashoffset`.
fn normalized_diagonal(size: usvg::Size) -> f32 {
    (size.width() * size.width() + size.height() * size.height()).sqrt()
        / std::f32::consts::SQRT_2
}

fn parse_view_box(element: &ServoLayoutElement<'_>) -> Option<usvg::ViewBox> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from("viewBox"))?;
    let vb = value.parse::<svgtypes::ViewBox>().ok()?;
    let rect = usvg::NonZeroRect::from_xywh(
        vb.x as f32,
        vb.y as f32,
        vb.w as f32,
        vb.h as f32,
    )?;

    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();

    Some(usvg::ViewBox { rect, aspect })
}

/// Collects `<linearGradient>`/`<radialGradient>` definitions (and their
/// `<stop>` children) into `gradients`, and records `<pattern>` elements into
/// `pattern_elements` for a second build pass.
fn collect_paint_servers<'a>(
    node: ServoLayoutNode<'a>,
    gradients: &mut Gradients,
    pattern_elements: &mut Vec<ServoLayoutElement<'a>>,
) {
    let Some(element) = node.as_element() else {
        return;
    };
    match element_layout_type(&element) {
        LayoutElementType::SVGLinearGradientElement => {
            if let (Some(id), Some(grad)) = (element_id(&element), build_linear_gradient(&element)) {
                gradients.linear.insert(id, Arc::new(grad));
            }
            return;
        },
        LayoutElementType::SVGRadialGradientElement => {
            if let (Some(id), Some(grad)) = (element_id(&element), build_radial_gradient(&element)) {
                gradients.radial.insert(id, Arc::new(grad));
            }
            return;
        },
        LayoutElementType::SVGPatternElement => {
            if element_id(&element).is_some() {
                pattern_elements.push(element);
            }
            return;
        },
        _ => {},
    }

    for child in node.dom_children() {
        collect_paint_servers(child, gradients, pattern_elements);
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

/// Returns the [`LayoutElementType`] for `element`, the layout-standard way to
/// discriminate element kinds (as opposed to matching the tag name). Falls back to
/// the generic [`LayoutElementType::Element`] for pseudo-elements, which have no
/// type id.
fn element_layout_type(element: &ServoLayoutElement<'_>) -> LayoutElementType {
    match element.type_id() {
        Some(LayoutNodeType::Element(ty)) => ty,
        _ => LayoutElementType::Element,
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
    let x1 = number_or_percentage_attr(element, "x1", 0.0);
    let y1 = number_or_percentage_attr(element, "y1", 0.0);
    let x2 = number_or_percentage_attr(element, "x2", 1.0);
    let y2 = number_or_percentage_attr(element, "y2", 0.0);
    let base = build_base_gradient(element, id)?;
    Some(usvg::LinearGradient::new(base, x1, y1, x2, y2))
}

fn build_radial_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::RadialGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let cx = number_or_percentage_attr(element, "cx", 0.5);
    let cy = number_or_percentage_attr(element, "cy", 0.5);
    let r = usvg::PositiveF32::new(number_or_percentage_attr(element, "r", 0.5))?;
    let fx = number_or_percentage_attr(element, "fx", cx);
    let fy = number_or_percentage_attr(element, "fy", cy);
    let fr = usvg::PositiveF32::new(number_or_percentage_attr(element, "fr", 0.0))?;
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
    if element_layout_type(element) != LayoutElementType::SVGStopElement {
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
        .and_then(|s| s.parse::<svgtypes::Color>().ok())
        .unwrap_or_else(svgtypes::Color::black);

    let stop_opacity = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-opacity"))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);
    // Fold any alpha carried by `stop-color` (`rgba()`/8-digit hex) into the stop
    // opacity, mirroring usvg's `split_alpha`.
    let opacity = (stop_opacity * color.alpha as f32 / 255.0).clamp(0.0, 1.0);

    Some(usvg::Stop::new(
        offset,
        usvg::Color::new_rgb(color.red, color.green, color.blue),
        usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE),
    ))
}

/// Builds a [`usvg::Pattern`] from a `<pattern>` element.
fn build_pattern(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
) -> Option<usvg::Pattern> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;

    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("patternUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let content_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("patternContentUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("patternTransform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);

    let x = length_or_percentage_attr(element, "x", 0.0);
    let y = length_or_percentage_attr(element, "y", 0.0);
    let width = length_or_percentage_attr(element, "width", 0.0);
    let height = length_or_percentage_attr(element, "height", 0.0);
    let rect = usvg::NonZeroRect::from_xywh(x, y, width, height)?;

    let view_box = parse_view_box(element);

    let mut root = usvg::Group::empty();
    for child in element.as_node().dom_children() {
        if let Some(child_node) = convert_node(
            child,
            context,
            gradients,
            defs,
            diagonal,
            usvg::Transform::identity(),
            None,
        ) {
            root.push_child(child_node);
        }
    }

    Some(usvg::Pattern::new(
        id,
        units,
        content_units,
        transform,
        rect,
        view_box,
        root,
    ))
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
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let ty = element_layout_type(&element);

    if is_group_element(ty) {
        return convert_group(node, context, gradients, defs, diagonal, parent_abs_transform, host);
    }

    if ty == LayoutElementType::SVGUseElement {
        return convert_use(node, context, gradients, defs, diagonal, parent_abs_transform);
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    if let Some(shape) = build_shape_node(
        &element,
        ty,
        computed.as_deref(),
        host,
        gradients,
        diagonal,
        parent_abs_transform,
    ) {
        return Some(shape);
    }

    // Elements we don't handle yet (text, image, …) are silently skipped.
    None
}

fn is_group_element(ty: LayoutElementType) -> bool {
    matches!(
        ty,
        LayoutElementType::SVGSVGElement
            | LayoutElementType::SVGGElement
            | LayoutElementType::SVGDefsElement
            | LayoutElementType::SVGSymbolElement
            | LayoutElementType::SVGAElement
            | LayoutElementType::SVGClipPathElement
            | LayoutElementType::SVGMaskElement
    )
}

fn convert_group(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
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
            convert_node(child, context, gradients, defs, diagonal, abs_transform, host)
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
    diagonal: f32,
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
        diagonal,
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
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    host: Option<&ComputedValues>,
    gradients: &Gradients,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let data = build_shape_path(element, ty, computed)?;

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
    let stroke = paint_computed.and_then(|c| build_stroke(c, gradients, diagonal));

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
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
) -> Option<tiny_skia_path::Path> {
    match ty {
        LayoutElementType::SVGRectElement => {
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
        LayoutElementType::SVGCircleElement => {
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
            pb.push_circle(cx, cy, r);
            pb.finish()
        },
        LayoutElementType::SVGEllipseElement => {
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
            let Some(oval) = tiny_skia_path::Rect::from_xywh(cx - rx, cy - ry, rx + rx, ry + ry)
            else {
                return None;
            };
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.push_oval(oval);
            pb.finish()
        },
        LayoutElementType::SVGLineElement => {
            let x1 = length_attr(element, "x1", 0.0);
            let y1 = length_attr(element, "y1", 0.0);
            let x2 = length_attr(element, "x2", 0.0);
            let y2 = length_attr(element, "y2", 0.0);
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            pb.finish()
        },
        LayoutElementType::SVGPolylineElement => polygon_points(element, "points", false),
        LayoutElementType::SVGPolygonElement => polygon_points(element, "points", true),
        LayoutElementType::SVGPathElement => element
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

fn rounded_rect(
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

fn build_stroke(
    computed: &ComputedValues,
    gradients: &Gradients,
    diagonal: f32,
) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.stroke, computed, gradients)?;

    // A negative `stroke-width` is invalid CSS and is rejected by the parser
    // (`SVGWidth = NonNegativeLengthPercentage`), so it falls back to the
    // initial value `1` here in the computed style. We faithfully pass that
    // through to usvg/resvg, which renders a 1px stroke — matching Chrome/Edge.
    let width = match &inherited.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => nn_lp.0.resolve(Length::new(diagonal)).px(),
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
            let mut dasharray: Vec<f32> = vs
                .iter()
                .map(|v| v.0.resolve(Length::new(diagonal)).px())
                .collect();

            // Per SVG2, an odd-length dash array is repeated to yield an even
            // length, so dashes and gaps alternate correctly. usvg's own XML
            // parser does the same (`parser::style::conv_dasharray`); since we
            // build the tree programmatically we must replicate it here,
            // otherwise `tiny_skia_path::StrokeDash::new` rejects the odd list
            // and the stroke renders solid.
            if dasharray.len() % 2 != 0 {
                let mut doubled = dasharray.clone();
                doubled.extend_from_slice(&dasharray);
                dasharray = doubled;
            }

            stroke.dasharray = Some(dasharray);
        }
    }
    stroke.dashoffset = match &inherited.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => lp.resolve(Length::new(diagonal)).px(),
        _ => 0.0,
    };

    // `vector-effect: non-scaling-stroke` keeps the stroke width in the outermost
    // SVG coordinate space, unaffected by the element's transform. resvg
    // compensates for this at render time using the path's `abs_transform`.
    stroke.non_scaling_stroke = computed
        .get_svg()
        .vector_effect
        .contains(VectorEffect::NON_SCALING_STROKE);

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
            } else if let Some(p) = gradients.pattern.get(&fragment) {
                Some(usvg::Paint::Pattern(p.clone()))
            } else {
                // Unresolved paint server: fall back to black, matching usvg's
                // behaviour when a referenced paint server is missing.
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

/// Parses a gradient coordinate attribute (`x1`, `cx`, `r`, …).
///
/// Per SVG these accept either a bare `<number>` or a `<percentage>`; a
/// percentage is the same fraction expressed in the 0–100 range. Returns
/// `default` when missing or unparseable.
fn number_or_percentage_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from(attr)) else {
        return default;
    };
    let value = value.trim();
    if let Some(pct) = value.strip_suffix('%') {
        pct.parse::<f32>().ok().map(|v| v / 100.0).unwrap_or(default)
    } else {
        parse_length_attr(value).unwrap_or(default)
    }
}

/// Parses a `<length>|<percentage>` attribute value (used for a `pattern`'s
/// `x`/`y`/`width`/`height`). A percentage becomes a 0–1 fraction; any other
/// value is taken as a raw number (font-relative units are not resolved here).
fn length_or_percentage_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from(attr)) else {
        return default;
    };
    match value.trim().parse::<svgtypes::Length>() {
        Ok(length) if length.unit == LengthUnit::Percent => length.number as f32 / 100.0,
        Ok(length) => length.number as f32,
        Err(_) => default,
    }
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
