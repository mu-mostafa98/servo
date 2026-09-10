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

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use std::hash::{Hash, Hasher};

use data_url::DataUrl;
use html5ever::{LocalName, ns};
use image::ImageReader;
use layout_api::{LayoutElement, LayoutElementType, LayoutNode, LayoutNodeType};
use net_traits::image_cache::{FontResolver, ImageCache};
use resvg::usvg::{self, filter, fontdb, tiny_skia_path, ApproxEqUlps, ApproxZeroUlps};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use script::SvgFontResolver;
use style::color::ColorSpace;
use style::dom::{NodeInfo, OpaqueNode, TNode};
use style::properties::ComputedValues;
use style::values::computed::font::SingleFontFamily;
use style::values::computed::{
    FontStyle as ServoFontStyle, Length, LengthPercentage, NonNegativeLengthPercentageOrAuto,
};
use style::values::specified::font::FontStretchKeyword;
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray,
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

/// Font database and resolver used to lay out `<text>` into glyph outlines.
///
/// usvg lays text out at build time into `Text.flattened` (a group of glyph
/// outlines), reusing its own shaping/positioning engine rather than
/// reimplementing it. The resolver pulls fonts from the script thread's
/// `FontContext` on demand, exactly like the existing image-cache SVG path.
struct SvgFonts {
    resolver: usvg::FontResolver<'static>,
    cache: RefCell<usvg::Cache>,
}

impl SvgFonts {
    fn new(context: &LayoutContext) -> Self {
        let servo_resolver: Arc<dyn FontResolver> =
            Arc::new(SvgFontResolver::new(context.font_context.clone()));
        let resolver = {
            let select_font = servo_resolver.clone();
            let select_fallback = servo_resolver.clone();
            usvg::FontResolver {
                select_font: Box::new(move |font, database| select_font.resolve(font, database)),
                select_fallback: Box::new(move |ch, ids, database| {
                    select_fallback.resolve_fallback(ch, ids, database)
                }),
            }
        };
        SvgFonts {
            resolver,
            cache: RefCell::new(usvg::Cache::new(Arc::new(fontdb::Database::new()))),
        }
    }
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

    // Font database + resolver shared by every `<text>` subtree.
    let fonts = SvgFonts::new(context);

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
            if let Some(pattern) =
                build_pattern(element, context, &gradients, &defs, diagonal, &fonts)
            {
                gradients.pattern.insert(id, Arc::new(pattern));
            }
        }
    }

    // Children are built in viewBox (user) coordinates; the viewBox transform is
    // applied later, at raster time.
    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        for child_node in convert_node(
            child,
            context,
            &gradients,
            &defs,
            diagonal,
            usvg::Transform::identity(),
            None,
            &fonts,
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
    fonts: &SvgFonts,
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
        for child_node in convert_node(
            child,
            context,
            gradients,
            defs,
            diagonal,
            usvg::Transform::identity(),
            None,
            fonts,
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

/// Converts a DOM node (and its subtree) into a list of [`usvg::Node`]s.
///
/// A shape normally produces a single node, but a shape carrying markers emits the
/// path plus one group per placed marker (markers are siblings of the path in usvg,
/// not a child node). Callers must therefore push every returned node.
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
    fonts: &SvgFonts,
) -> Vec<usvg::Node> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let ty = element_layout_type(&element);

    // A nested `<svg>` establishes a new viewport (x/y + viewBox transform), so it
    // needs dedicated handling rather than the plain `<g>` group path.
    if ty == LayoutElementType::SVGSVGElement {
        return convert_svg(
            node,
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            host,
            fonts,
        )
        .into_iter()
        .collect();
    }

    // `<defs>` and `<symbol>` are definition-only containers: their content never
    // renders directly, only when instantiated via `<use href="#id">`. Skip them in
    // the normal walk (they remain resolvable through the `defs` id map).
    if matches!(
        ty,
        LayoutElementType::SVGDefsElement | LayoutElementType::SVGSymbolElement
    ) {
        return Vec::new();
    }

    if is_group_element(ty) {
        return convert_group(
            node,
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            host,
            fonts,
        )
        .into_iter()
        .collect();
    }

    if ty == LayoutElementType::SVGUseElement {
        return convert_use(
            node,
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            fonts,
        )
        .into_iter()
        .collect();
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    if ty == LayoutElementType::SVGTextElement {
        return convert_text(
            &element,
            computed.as_deref(),
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            fonts,
        );
    }

    if ty == LayoutElementType::SVGImageElement {
        return convert_image(
            &element,
            computed.as_deref(),
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            fonts,
        );
    }

    build_shape_node(
        &element,
        ty,
        computed.as_deref(),
        host,
        context,
        gradients,
        defs,
        diagonal,
        parent_abs_transform,
        fonts,
    )
}

fn is_group_element(ty: LayoutElementType) -> bool {
    matches!(
        ty,
        LayoutElementType::SVGSVGElement
            | LayoutElementType::SVGGElement
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
    fonts: &SvgFonts,
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
        for child_node in
            convert_node(child, context, gradients, defs, diagonal, abs_transform, host, fonts)
        {
            group.push_child(child_node);
        }
    }

    // Compute the object bounding box before resolving `clip-path`/`mask`, since
    // either may use `objectBoundingBox` units that depend on it.
    let object_bbox = group.compute_object_bbox();

    match resolve_clip_path(&element, context, gradients, defs, diagonal, fonts, object_bbox) {
        ClipPathOutcome::Clip(clip) => group.clip_path = Some(clip),
        ClipPathOutcome::Invalid => return None,
        ClipPathOutcome::None => {}
    }

    match resolve_mask(&element, context, gradients, defs, diagonal, fonts, object_bbox) {
        MaskOutcome::Mask(mask) => group.mask = Some(mask),
        MaskOutcome::Invalid => return None,
        MaskOutcome::None => {}
    }

    match resolve_filter(&element, defs, object_bbox) {
        FilterOutcome::Filter(filter) => group.filters.push(filter),
        FilterOutcome::Invalid => return None,
        FilterOutcome::None => {}
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Converts a nested `<svg>` element, which establishes a new viewport: its `x`/`y`
/// position plus a `viewBox`→viewport transform (`preserveAspectRatio`-aware). When
/// `overflow` is not `visible` (and explicit `width`/`height` form a rectangle), a
/// synthetic clip path limits rendering to the new viewport — mirroring the parser's
/// `use_node::convert_svg` in usvg.
///
/// The structure matches usvg: an outer group carries the `transform` attribute and
/// (optionally) the clip path, and an inner group carries the viewport transform
/// (`translate(x, y) · viewBox`) so that it participates correctly in bounding-box
/// and clipping calculations.
fn convert_svg(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
    fonts: &SvgFonts,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);

    // The `transform` attribute operates in the parent user space, before the new
    // viewport is established, so it stays on the outer group.
    let orig_ts = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);

    // Viewport transform: translate to (x, y), then map the `viewBox` onto the
    // viewport rectangle.
    let mut viewport_ts = usvg::Transform::from_translate(x, y);
    if let Some(vb) = parse_view_box(&element) {
        let w = length_attr_opt(&element, "width").unwrap_or(100.0);
        let h = length_attr_opt(&element, "height").unwrap_or(100.0);
        if let Some(size) = usvg::Size::from_wh(w, h) {
            viewport_ts = viewport_ts.pre_concat(vb.to_transform(size));
        }
    }

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();
    group.transform = orig_ts;
    let abs_transform = parent_abs_transform.pre_concat(orig_ts);
    group.abs_transform = abs_transform;

    if let Some(computed) = computed.as_deref() {
        group.opacity = usvg::Opacity::new(computed.get_effects().opacity)
            .unwrap_or(usvg::Opacity::ONE);
    }

    // A nested `svg` with explicit `width`/`height` is clipped to its viewport
    // rectangle unless `overflow` is `visible` (or `auto`).
    let overflow_visible = matches!(
        element.attribute_as_str(&ns!(), &LocalName::from("overflow")),
        Some("visible") | Some("auto")
    );
    if !overflow_visible {
        if let (Some(w), Some(h)) = (
            length_attr_opt(&element, "width"),
            length_attr_opt(&element, "height"),
        ) {
            if let Some(clip_rect) = usvg::NonZeroRect::from_xywh(x, y, w, h) {
                group.clip_path = rect_clip_path(clip_rect);
            }
        }
    }

    // Wrap the children in an inner group carrying the viewport transform, so it
    // applies as a proper transform in bounding-box and clip calculations.
    let mut inner = usvg::Group::empty();
    inner.transform = viewport_ts;
    inner.abs_transform = abs_transform.pre_concat(viewport_ts);

    for child in node.dom_children() {
        for child_node in convert_node(
            child,
            context,
            gradients,
            defs,
            diagonal,
            inner.abs_transform,
            host,
            fonts,
        ) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}

/// Builds a synthetic `clipPath` containing a single rectangle, used to emulate a
/// nested `<svg>` viewport's `overflow` clipping.
fn rect_clip_path(rect: usvg::NonZeroRect) -> Option<Arc<usvg::ClipPath>> {
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
fn build_clip_path(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    object_bbox: Option<usvg::NonZeroRect>,
    depth: usize,
) -> Option<Arc<usvg::ClipPath>> {
    if depth > 8 || element_layout_type(element) != LayoutElementType::SVGClipPathElement {
        return None;
    }
    let id_str = element_id(element)?;

    let mut transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);

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
            let linked = defs.get(&ref_id)?;
            Some(build_clip_path(
                linked,
                context,
                gradients,
                defs,
                diagonal,
                fonts,
                object_bbox,
                depth + 1,
            )?)
        },
        None => None,
    };

    // Children are built in the clip path's own coordinate system (the document
    // user space for `userSpaceOnUse`, `[0, 1]` units for `objectBoundingBox`).
    // `transform` is applied at render time (`resvg::clip::apply`).
    let mut root = usvg::Group::empty();
    build_clip_path_children(
        element,
        context,
        gradients,
        defs,
        diagonal,
        fonts,
        usvg::Transform::identity(),
        &mut root,
    );

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
fn build_clip_path_children(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    parent_abs_transform: usvg::Transform,
    root: &mut usvg::Group,
) {
    for child in element.as_node().dom_children() {
        for node in convert_clip_child(
            child,
            context,
            gradients,
            defs,
            diagonal,
            fonts,
            parent_abs_transform,
        ) {
            root.push_child(node);
        }
    }
}

/// Converts a single clip-path child (shape, group, `<use>`, or text) into clip
/// geometry nodes.
fn convert_clip_child(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let ty = element_layout_type(&element);

    if ty == LayoutElementType::SVGUseElement {
        return convert_clip_use(
            &element,
            context,
            gradients,
            defs,
            diagonal,
            fonts,
            parent_abs_transform,
        );
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    // Per the SVG spec a `<clipPath>` may only contain basic shapes, `text` and
    // `use` (referencing those). A `<g>`/`<a>`/`<svg>` container is not a
    // conforming child: usvg's parser skips it (`is_graphic` excludes `G`/`A`/`Svg`),
    // leaving the clip path empty, and so do Chrome/Edge/ServoShell. Skip it here to
    // match — the referencing element will then be dropped by the caller.
    if matches!(
        ty,
        LayoutElementType::SVGGElement
            | LayoutElementType::SVGAElement
            | LayoutElementType::SVGSVGElement
    ) {
        return Vec::new();
    }

    // Text contributes its flattened glyph outlines (fill color is irrelevant for
    // the alpha-only clip mask).
    if ty == LayoutElementType::SVGTextElement {
        return convert_text(
            &element,
            computed.as_deref(),
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            fonts,
        );
    }

    // Shapes: geometry + a black fill (rule from `clip-rule`), never stroked.
    if let Some(path) = build_shape_path(&element, ty, computed.as_deref()) {
        let transform = element
            .attribute_as_str(&ns!(), &LocalName::from("transform"))
            .map(parse_transform)
            .unwrap_or_else(usvg::Transform::identity);
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
        .map(|p| usvg::Node::Path(Box::new(p)))
        else {
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
fn convert_clip_use(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(href) = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))
    else {
        return Vec::new();
    };
    let id = href.trim_start_matches('#');
    let Some(referenced) = defs.get(id) else {
        return Vec::new();
    };
    if referenced.as_node().opaque() == element.as_node().opaque() {
        return Vec::new();
    }

    let mut transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let x = length_attr_opt(element, "x").unwrap_or(0.0);
    let y = length_attr_opt(element, "y").unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        transform = transform.pre_concat(usvg::Transform::from_translate(x, y));
    }
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let mut nodes = convert_clip_child(
        referenced.as_node(),
        context,
        gradients,
        defs,
        diagonal,
        fonts,
        abs_transform,
    );

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
enum ClipPathOutcome {
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
fn resolve_clip_path(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    object_bbox: Option<usvg::NonZeroRect>,
) -> ClipPathOutcome {
    let Some(ref_id) = clip_path_reference(element) else {
        return ClipPathOutcome::None;
    };
    let Some(linked) = defs.get(&ref_id) else {
        // Dangling id: not in the document, so treat as "no clip" like usvg.
        return ClipPathOutcome::None;
    };
    match build_clip_path(
        linked,
        context,
        gradients,
        defs,
        diagonal,
        fonts,
        object_bbox,
        0,
    ) {
        Some(clip) => ClipPathOutcome::Clip(clip),
        None => ClipPathOutcome::Invalid,
    }
}

/// Extracts the referenced id from a `mask` attribute, if it is a local
/// `url(#id)` reference (not `none`).
fn mask_reference(element: &ServoLayoutElement<'_>) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from("mask"))?
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

/// Builds a [`usvg::Mask`] from a `<mask>` element, mirroring usvg's
/// `parser::mask::convert`.
///
/// `object_bbox` is the bounding box of the element *being masked* (in its local
/// coordinate system); `depth` guards against cyclic `mask` references.
fn build_mask(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    object_bbox: Option<usvg::NonZeroRect>,
    depth: usize,
) -> Option<Arc<usvg::Mask>> {
    if depth > 8 || element_layout_type(element) != LayoutElementType::SVGMaskElement {
        return None;
    }
    let id_str = element_id(element)?;

    // `maskUnits` (the mask *region*) defaults to `objectBoundingBox`; the mask
    // *content* (`maskContentUnits`) defaults to `userSpaceOnUse`.
    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("maskUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let content_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("maskContentUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    // Region x/y/width/height default to `-10% -10% 120% 120%`. Percentages are
    // resolved as a fraction of the object bbox (`length_or_percentage_attr` maps
    // `%` → `value/100`), which is the correct interpretation for `objectBoundingBox`.
    let x = length_or_percentage_attr(element, "x", -0.1);
    let y = length_or_percentage_attr(element, "y", -0.1);
    let width = length_or_percentage_attr(element, "width", 1.2);
    let height = length_or_percentage_attr(element, "height", 1.2);
    let rect = usvg::NonZeroRect::from_xywh(x, y, width, height)?;

    let mut rect = match units {
        usvg::Units::ObjectBoundingBox => {
            // `objectBoundingBox` maps the `(0,0)-(1,1)` square onto the object's
            // bbox. When there is no bbox (zero-sized/empty object), the whole
            // element is masked, mirroring usvg's `mask_all` branch.
            match object_bbox {
                Some(bbox) => rect.bbox_transform(bbox),
                None => {
                    let id = usvg::NonEmptyString::new(id_str)?;
                    return Some(Arc::new(usvg::Mask::new(
                        id,
                        rect,
                        usvg::MaskType::Luminance,
                        None,
                        usvg::Group::empty(),
                    )));
                },
            }
        },
        usvg::Units::UserSpaceOnUse => rect,
    };

    // A `<mask>` may itself reference another `<mask>` via its own `mask` attribute,
    // nesting the two.
    let mask = match mask_reference(element) {
        Some(ref_id) => {
            let linked = defs.get(&ref_id)?;
            Some(build_mask(
                linked,
                context,
                gradients,
                defs,
                diagonal,
                fonts,
                object_bbox,
                depth + 1,
            )?)
        },
        None => None,
    };

    let kind = if element.attribute_as_str(&ns!(), &LocalName::from("mask-type")) == Some("alpha") {
        usvg::MaskType::Alpha
    } else {
        usvg::MaskType::Luminance
    };

    let mut root = usvg::Group::empty();

    // Mask content is authored in the referencing element's user space
    // (`maskContentUnits="userSpaceOnUse"`, the default) or in `[0,1]` bbox units
    // (`objectBoundingBox`), in which case it is wrapped in a `from_bbox` group.
    if content_units == usvg::Units::ObjectBoundingBox {
        let object_bbox = object_bbox?;
        let mut subroot = usvg::Group::empty();
        subroot.transform = usvg::Transform::from_bbox(object_bbox);
        subroot.abs_transform = subroot.transform;

        for child in element.as_node().dom_children() {
            for node in convert_node(
                child,
                context,
                gradients,
                defs,
                diagonal,
                subroot.transform,
                None,
                fonts,
            ) {
                subroot.push_child(node);
            }
        }

        if !subroot.has_children() {
            return None;
        }

        root.push_child(usvg::Node::Group(Box::new(subroot)));
    } else {
        for child in element.as_node().dom_children() {
            for node in convert_node(
                child,
                context,
                gradients,
                defs,
                diagonal,
                usvg::Transform::identity(),
                None,
                fonts,
            ) {
                root.push_child(node);
            }
        }

        // A mask without children is invalid (the referencing element is dropped),
        // except in the zero-bbox case handled above.
        if !root.has_children() {
            return None;
        }
    }

    Some(Arc::new(usvg::Mask::new(
        usvg::NonEmptyString::new(id_str)?,
        rect,
        kind,
        mask,
        root,
    )))
}

/// The result of resolving an element's `mask` attribute.
enum MaskOutcome {
    /// No mask: the attribute is absent, `none`, malformed, or a dangling
    /// reference — render the element normally.
    None,
    /// A valid mask to apply.
    Mask(Arc<usvg::Mask>),
    /// The `mask` references a `mask` element that is invalid (empty, or not a
    /// `<mask>`) — the element must be dropped entirely.
    Invalid,
}

/// Resolves an element's `mask` reference into a [`usvg::Mask`], mirroring usvg's
/// `convert_group`/`mask::convert` handshake: a dangling reference is ignored, while
/// a present-but-invalid mask drops the element.
fn resolve_mask(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    fonts: &SvgFonts,
    object_bbox: Option<usvg::NonZeroRect>,
) -> MaskOutcome {
    let Some(ref_id) = mask_reference(element) else {
        return MaskOutcome::None;
    };
    let Some(linked) = defs.get(&ref_id) else {
        // Dangling id: not in the document, so treat as "no mask" like usvg.
        return MaskOutcome::None;
    };
    match build_mask(linked, context, gradients, defs, diagonal, fonts, object_bbox, 0) {
        Some(mask) => MaskOutcome::Mask(mask),
        None => MaskOutcome::Invalid,
    }
}

/// Returns the `id` referenced by an element's `filter="url(#id)"` attribute,
/// or `None` when the element has no filter (or `filter="none"`).
fn filter_reference(element: &ServoLayoutElement<'_>) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from("filter"))?
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

/// The result of resolving an element's `filter` attribute.
enum FilterOutcome {
    /// No filter: the attribute is absent, `none`, or malformed — render normally.
    None,
    /// A valid filter to apply.
    Filter(Arc<filter::Filter>),
    /// The `filter` references a missing or empty `<filter>` element — per usvg the
    /// whole element must be dropped.
    Invalid,
}

/// Resolves an element's `filter` reference into a [`usvg::Filter`], mirroring usvg's
/// `convert_group`/`filter::convert` handshake: `filter="none"` is a no-op, while a
/// dangling reference or an empty filter drops the element.
fn resolve_filter(
    element: &ServoLayoutElement<'_>,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> FilterOutcome {
    let Some(ref_id) = filter_reference(element) else {
        return FilterOutcome::None;
    };
    let Some(linked) = defs.get(&ref_id) else {
        return FilterOutcome::Invalid;
    };
    match build_filter(linked, object_bbox) {
        Some(filter) => FilterOutcome::Filter(filter),
        None => FilterOutcome::Invalid,
    }
}

/// Builds a [`usvg::Filter`] from a `<filter>` element, mirroring usvg's
/// `parser::filter::convert_url`.
///
/// `object_bbox` is the bounding box of the element *being filtered* (used by
/// `filterUnits="objectBoundingBox"`, the default, and by
/// `primitiveUnits="objectBoundingBox"`).
fn build_filter(
    element: &ServoLayoutElement<'_>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> Option<Arc<filter::Filter>> {
    let id_str = element_id(element)?;

    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("filterUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let primitive_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("primitiveUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    // Filter region defaults to `-10% -10% 120% 120%`; percentages are resolved as
    // a fraction of the object bbox (`length_or_percentage_attr` maps `%` → `/100`),
    // the same convention usvg uses for `objectBoundingBox`.
    let rect = match units {
        usvg::Units::ObjectBoundingBox => {
            let x = length_or_percentage_attr(element, "x", -0.1);
            let y = length_or_percentage_attr(element, "y", -0.1);
            let w = length_or_percentage_attr(element, "width", 1.2);
            let h = length_or_percentage_attr(element, "height", 1.2);
            let r = usvg::NonZeroRect::from_xywh(x, y, w, h)?;
            // `objectBoundingBox` maps the `(0,0)-(1,1)` square onto the object bbox.
            r.bbox_transform(object_bbox?)
        },
        usvg::Units::UserSpaceOnUse => {
            let x = length_attr_opt(element, "x");
            let y = length_attr_opt(element, "y");
            let w = length_attr_opt(element, "width");
            let h = length_attr_opt(element, "height");
            match (x, y, w, h) {
                (Some(x), Some(y), Some(w), Some(h)) => usvg::NonZeroRect::from_xywh(x, y, w, h)?,
                // No explicit user-space region: fall back to the object bbox.
                _ => object_bbox?,
            }
        },
    };

    let primitives = build_filter_primitives(element, primitive_units, object_bbox, rect);
    if primitives.is_empty() {
        // An empty filter is invalid, mirroring usvg's `convert_url`.
        return None;
    }

    Some(Arc::new(filter::Filter::new(
        usvg::NonEmptyString::new(id_str)?,
        rect,
        primitives,
    )))
}

/// Builds the filter primitives (`fe*` children) of a `<filter>` element.
fn build_filter_primitives(
    filter_element: &ServoLayoutElement<'_>,
    primitive_units: usvg::Units,
    object_bbox: Option<usvg::NonZeroRect>,
    filter_region: usvg::NonZeroRect,
) -> Vec<filter::Primitive> {
    let mut primitives: Vec<filter::Primitive> = Vec::new();
    let mut result_idx = 0usize;
    let mut used_names: Vec<String> = Vec::new();

    // Scale factor for `primitiveUnits="objectBoundingBox"`: primitive lengths
    // (`stdDeviation`, `dx`/`dy`, `scale`, `radius`) are multiplied by the bbox size.
    let scale = match primitive_units {
        usvg::Units::ObjectBoundingBox => match object_bbox {
            Some(bbox) => bbox.size(),
            None => return Vec::new(),
        },
        usvg::Units::UserSpaceOnUse => usvg::Size::from_wh(1.0, 1.0).unwrap(),
    };

    for child in filter_element.as_node().dom_children() {
        let Some(child_el) = child.as_element() else {
            continue;
        };
        let tag = child_el.local_name().to_string();

        let kind = match tag.as_str() {
            "feGaussianBlur" => filter_gaussian_blur(&child_el, &primitives, scale),
            "feDropShadow" => filter_drop_shadow(&child_el, &primitives, scale),
            "feColorMatrix" => filter_color_matrix(&child_el, &primitives),
            "feOffset" => filter_offset(&child_el, &primitives, scale),
            "feBlend" => filter_blend(&child_el, &primitives),
            "feFlood" => filter_flood(&child_el),
            "feComposite" => filter_composite(&child_el, &primitives),
            "feMerge" => filter_merge(&child_el, &primitives),
            "feComponentTransfer" => filter_component_transfer(&child_el, &primitives),
            "feTurbulence" => filter_turbulence(&child_el),
            "feDisplacementMap" => filter_displacement_map(&child_el, &primitives, scale),
            "feConvolveMatrix" => match filter_convolve_matrix(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feMorphology" => filter_morphology(&child_el, &primitives, scale),
            "feDiffuseLighting" => match filter_diffuse_lighting(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feSpecularLighting" => match filter_specular_lighting(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feTile" => filter::Kind::Tile(filter::Tile::new(resolve_filter_input(
                &child_el,
                "in",
                &primitives,
            ))),
            _ => continue,
        };

        let color_interpolation = match child_el
            .attribute_as_str(&ns!(), &LocalName::from("color-interpolation-filters"))
        {
            Some("sRGB") => filter::ColorInterpolation::SRGB,
            _ => filter::ColorInterpolation::LinearRGB,
        };

        let result = gen_filter_result(&child_el, &mut result_idx, &mut used_names);

        // Primitive subregion defaults to the whole filter region; per-primitive
        // `x`/`y`/`width`/`height` (rarely used) are not yet supported.
        primitives.push(filter::Primitive::new(
            filter_region,
            color_interpolation,
            result,
            kind,
        ));
    }

    primitives
}

/// Generates a `result` name for a filter primitive, mirroring usvg's `gen_result`:
/// the explicit `result` attribute is used verbatim, otherwise a unique `resultN`
/// name is minted.
fn gen_filter_result(
    element: &ServoLayoutElement<'_>,
    idx: &mut usize,
    used: &mut Vec<String>,
) -> String {
    if let Some(s) = element.attribute_as_str(&ns!(), &LocalName::from("result")) {
        let s = s.to_string();
        used.push(s.clone());
        *idx += 1;
        s
    } else {
        loop {
            *idx += 1;
            let name = format!("result{}", *idx);
            if !used.contains(&name) {
                return name;
            }
        }
    }
}

/// Resolves a filter primitive's `in`/`in2` reference, mirroring usvg's
/// `resolve_input`: an unset input falls back to the previous primitive's result
/// (or `SourceGraphic` for the first primitive), and a reference to an unknown
/// result likewise falls back.
fn resolve_filter_input(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    primitives: &[filter::Primitive],
) -> filter::Input {
    match element.attribute_as_str(&ns!(), &LocalName::from(attr)) {
        Some(s) => {
            let input = parse_filter_input(s);
            if let filter::Input::Reference(ref name) = input {
                if !primitives.iter().any(|p| p.result() == name.as_str()) {
                    return match primitives.last() {
                        Some(prev) => filter::Input::Reference(prev.result().to_string()),
                        None => filter::Input::SourceGraphic,
                    };
                }
            }
            input
        },
        None => match primitives.last() {
            Some(prev) => filter::Input::Reference(prev.result().to_string()),
            None => filter::Input::SourceGraphic,
        },
    }
}

fn parse_filter_input(s: &str) -> filter::Input {
    match s {
        "SourceGraphic" => filter::Input::SourceGraphic,
        "SourceAlpha" => filter::Input::SourceAlpha,
        "BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint" => {
            filter::Input::SourceGraphic
        },
        _ => filter::Input::Reference(s.to_string()),
    }
}

/// Parses a `stdDeviation` attribute (`x`, or `x y`) into a pair, scaled by the
/// primitive units. Returns `(0, 0)` for an unset/malformed value.
fn filter_std_dev(
    element: &ServoLayoutElement<'_>,
    scale: usvg::Size,
    default: &str,
) -> (usvg::PositiveF32, usvg::PositiveF32) {
    let text = element
        .attribute_as_str(&ns!(), &LocalName::from("stdDeviation"))
        .unwrap_or(default);
    let nums = parse_number_list(text);
    let (sx, sy) = match (nums.first(), nums.get(1)) {
        (Some(a), Some(b)) => (*a, *b),
        (Some(a), None) => (*a, *a),
        _ => (0.0, 0.0),
    };
    let sx = usvg::PositiveF32::new(sx * scale.width()).unwrap_or(usvg::PositiveF32::ZERO);
    let sy = usvg::PositiveF32::new(sy * scale.height()).unwrap_or(usvg::PositiveF32::ZERO);
    (sx, sy)
}

/// Parses a color attribute (e.g. `flood-color`, `lighting-color`), defaulting to
/// `default` when absent/unparseable.
fn filter_color(element: &ServoLayoutElement<'_>, attr: &str, default: usvg::Color) -> usvg::Color {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<svgtypes::Color>().ok())
        .map(|c| usvg::Color::new_rgb(c.red, c.green, c.blue))
        .unwrap_or(default)
}

/// Parses a 0..1 opacity attribute (e.g. `flood-opacity`), defaulting to 1.
fn filter_opacity(element: &ServoLayoutElement<'_>, attr: &str) -> usvg::Opacity {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<f32>().ok())
        .and_then(|v| usvg::Opacity::new(v.clamp(0.0, 1.0)))
        .unwrap_or(usvg::Opacity::ONE)
}

fn filter_gaussian_blur(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let (sx, sy) = filter_std_dev(element, scale, "0 0");
    filter::Kind::GaussianBlur(filter::GaussianBlur::new(
        resolve_filter_input(element, "in", primitives),
        sx,
        sy,
    ))
}

fn filter_drop_shadow(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let (sx, sy) = filter_std_dev(element, scale, "2 2");
    let color = filter_color(element, "flood-color", usvg::Color::black());
    let opacity = filter_opacity(element, "flood-opacity");
    filter::Kind::DropShadow(filter::DropShadow::new(
        resolve_filter_input(element, "in", primitives),
        length_attr(element, "dx", 2.0) * scale.width(),
        length_attr(element, "dy", 2.0) * scale.height(),
        sx,
        sy,
        color,
        opacity,
    ))
}

fn filter_color_matrix(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let values = element
        .attribute_as_str(&ns!(), &LocalName::from("values"))
        .map(parse_number_list)
        .unwrap_or_default();
    let kind = match element.attribute_as_str(&ns!(), &LocalName::from("type")) {
        Some("saturate") => {
            let n = values.first().copied().unwrap_or(1.0).clamp(0.0, 1.0);
            filter::ColorMatrixKind::Saturate(usvg::PositiveF32::new(n).unwrap())
        },
        Some("hueRotate") => {
            filter::ColorMatrixKind::HueRotate(values.first().copied().unwrap_or(0.0))
        },
        Some("luminanceToAlpha") => filter::ColorMatrixKind::LuminanceToAlpha,
        _ => {
            if values.len() == 20 {
                filter::ColorMatrixKind::Matrix(values)
            } else {
                filter::ColorMatrixKind::default()
            }
        },
    };
    filter::Kind::ColorMatrix(filter::ColorMatrix::new(
        resolve_filter_input(element, "in", primitives),
        kind,
    ))
}

fn filter_offset(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    filter::Kind::Offset(filter::Offset::new(
        resolve_filter_input(element, "in", primitives),
        length_attr(element, "dx", 0.0) * scale.width(),
        length_attr(element, "dy", 0.0) * scale.height(),
    ))
}

fn filter_blend(element: &ServoLayoutElement<'_>, primitives: &[filter::Primitive]) -> filter::Kind {
    let mode = match element.attribute_as_str(&ns!(), &LocalName::from("mode")) {
        Some("multiply") => usvg::BlendMode::Multiply,
        Some("screen") => usvg::BlendMode::Screen,
        Some("darken") => usvg::BlendMode::Darken,
        Some("lighten") => usvg::BlendMode::Lighten,
        _ => usvg::BlendMode::Normal,
    };
    filter::Kind::Blend(filter::Blend::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        mode,
    ))
}

fn filter_flood(element: &ServoLayoutElement<'_>) -> filter::Kind {
    let color = filter_color(element, "flood-color", usvg::Color::black());
    let opacity = filter_opacity(element, "flood-opacity");
    filter::Kind::Flood(filter::Flood::new(color, opacity))
}

fn filter_composite(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let operator = match element.attribute_as_str(&ns!(), &LocalName::from("operator")) {
        Some("in") => filter::CompositeOperator::In,
        Some("out") => filter::CompositeOperator::Out,
        Some("atop") => filter::CompositeOperator::Atop,
        Some("xor") => filter::CompositeOperator::Xor,
        Some("arithmetic") => filter::CompositeOperator::Arithmetic {
            k1: number_attr(element, "k1", 0.0),
            k2: number_attr(element, "k2", 0.0),
            k3: number_attr(element, "k3", 0.0),
            k4: number_attr(element, "k4", 0.0),
        },
        _ => filter::CompositeOperator::Over,
    };
    filter::Kind::Composite(filter::Composite::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        operator,
    ))
}

fn filter_merge(element: &ServoLayoutElement<'_>, primitives: &[filter::Primitive]) -> filter::Kind {
    let mut inputs = Vec::new();
    for child in element.as_node().dom_children() {
        if let Some(child_el) = child.as_element() {
            inputs.push(resolve_filter_input(&child_el, "in", primitives));
        }
    }
    filter::Kind::Merge(filter::Merge::new(inputs))
}

fn filter_component_transfer(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let mut func_r = filter::TransferFunction::Identity;
    let mut func_g = filter::TransferFunction::Identity;
    let mut func_b = filter::TransferFunction::Identity;
    let mut func_a = filter::TransferFunction::Identity;
    for child in element.as_node().dom_children() {
        let Some(child_el) = child.as_element() else {
            continue;
        };
        let tag = child_el.local_name().to_string();
        let Some(func) = filter_transfer_function(&child_el) else {
            continue;
        };
        match tag.as_str() {
            "feFuncR" => func_r = func,
            "feFuncG" => func_g = func,
            "feFuncB" => func_b = func,
            "feFuncA" => func_a = func,
            _ => {},
        }
    }
    filter::Kind::ComponentTransfer(filter::ComponentTransfer::new(
        resolve_filter_input(element, "in", primitives),
        func_r,
        func_g,
        func_b,
        func_a,
    ))
}

fn filter_transfer_function(element: &ServoLayoutElement<'_>) -> Option<filter::TransferFunction> {
    let ty = element.attribute_as_str(&ns!(), &LocalName::from("type"))?;
    let table = element
        .attribute_as_str(&ns!(), &LocalName::from("tableValues"))
        .map(parse_number_list);
    Some(match ty {
        "identity" => filter::TransferFunction::Identity,
        "table" => filter::TransferFunction::Table(table.unwrap_or_default()),
        "discrete" => filter::TransferFunction::Discrete(table.unwrap_or_default()),
        "linear" => filter::TransferFunction::Linear {
            slope: number_attr(element, "slope", 1.0),
            intercept: number_attr(element, "intercept", 0.0),
        },
        "gamma" => filter::TransferFunction::Gamma {
            amplitude: number_attr(element, "amplitude", 1.0),
            exponent: number_attr(element, "exponent", 1.0),
            offset: number_attr(element, "offset", 0.0),
        },
        _ => return None,
    })
}

fn filter_turbulence(element: &ServoLayoutElement<'_>) -> filter::Kind {
    let (base_x, base_y) = {
        let list = element
            .attribute_as_str(&ns!(), &LocalName::from("baseFrequency"))
            .map(parse_number_list)
            .unwrap_or_default();
        let (x, y) = match (list.first(), list.get(1)) {
            (Some(a), Some(b)) => (*a, *b),
            (Some(a), None) => (*a, *a),
            _ => (0.0, 0.0),
        };
        let x = usvg::PositiveF32::new(x).unwrap_or(usvg::PositiveF32::ZERO);
        let y = usvg::PositiveF32::new(y).unwrap_or(usvg::PositiveF32::ZERO);
        (x, y)
    };
    let num_octaves = number_attr(element, "numOctaves", 1.0).max(0.0).round() as u32;
    let kind = match element.attribute_as_str(&ns!(), &LocalName::from("type")) {
        Some("fractalNoise") => filter::TurbulenceKind::FractalNoise,
        _ => filter::TurbulenceKind::Turbulence,
    };
    filter::Kind::Turbulence(filter::Turbulence::new(
        base_x,
        base_y,
        num_octaves,
        number_attr(element, "seed", 0.0).trunc() as i32,
        element.attribute_as_str(&ns!(), &LocalName::from("stitchTiles")) == Some("stitch"),
        kind,
    ))
}

fn filter_displacement_map(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let channel = |attr: &str| match element.attribute_as_str(&ns!(), &LocalName::from(attr)) {
        Some("R") => filter::ColorChannel::R,
        Some("G") => filter::ColorChannel::G,
        Some("B") => filter::ColorChannel::B,
        _ => filter::ColorChannel::A,
    };
    let scale = (scale.width() + scale.height()) / 2.0;
    filter::Kind::DisplacementMap(filter::DisplacementMap::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        number_attr(element, "scale", 0.0) * scale,
        channel("xChannelSelector"),
        channel("yChannelSelector"),
    ))
}

fn filter_convolve_matrix(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let (order_x, order_y) = {
        let list = element
            .attribute_as_str(&ns!(), &LocalName::from("order"))
            .map(parse_number_list)
            .unwrap_or_default();
        let ox = list.first().copied().unwrap_or(3.0).max(1.0) as u32;
        let oy = list.get(1).copied().unwrap_or(ox as f32).max(1.0) as u32;
        (ox, oy)
    };
    let matrix = element
        .attribute_as_str(&ns!(), &LocalName::from("kernelMatrix"))
        .map(parse_number_list)
        .unwrap_or_default();
    if matrix.len() != (order_x * order_y) as usize {
        return None;
    }

    let mut kernel_sum: f32 = matrix.iter().sum();
    kernel_sum = (kernel_sum * 1_000_000.0).round() / 1_000_000.0;
    if kernel_sum.approx_zero_ulps(4) {
        kernel_sum = 1.0;
    }
    let divisor = element
        .attribute_as_str(&ns!(), &LocalName::from("divisor"))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(kernel_sum);
    if divisor.approx_zero_ulps(4) {
        return None;
    }
    let divisor = usvg::NonZeroF32::new(divisor)?;

    let default_target = ((order_x as f32) / 2.0).floor() as u32;
    let target_x = number_attr(element, "targetX", default_target as f32) as i32;
    let target_y = number_attr(element, "targetY", default_target as f32) as i32;
    if target_x < 0 || target_x >= order_x as i32 || target_y < 0 || target_y >= order_y as i32 {
        return None;
    }

    let matrix_data =
        filter::ConvolveMatrixData::new(target_x as u32, target_y as u32, order_x, order_y, matrix)?;

    let edge_mode = match element.attribute_as_str(&ns!(), &LocalName::from("edgeMode")) {
        Some("none") => filter::EdgeMode::None,
        Some("wrap") => filter::EdgeMode::Wrap,
        _ => filter::EdgeMode::Duplicate,
    };
    let preserve_alpha = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAlpha"))
        .unwrap_or("false")
        == "true";

    Some(filter::Kind::ConvolveMatrix(filter::ConvolveMatrix::new(
        resolve_filter_input(element, "in", primitives),
        matrix_data,
        divisor,
        number_attr(element, "bias", 0.0),
        edge_mode,
        preserve_alpha,
    )))
}

fn filter_morphology(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let operator = match element.attribute_as_str(&ns!(), &LocalName::from("operator")) {
        Some("dilate") => filter::MorphologyOperator::Dilate,
        _ => filter::MorphologyOperator::Erode,
    };
    let list = element
        .attribute_as_str(&ns!(), &LocalName::from("radius"))
        .map(parse_number_list)
        .unwrap_or_default();
    let mut rx = list.first().copied().unwrap_or(scale.width());
    let mut ry = list.get(1).copied().unwrap_or(rx);
    // A zero radius on either axis resets it to 1.0 (Chrome/Safari behaviour).
    if rx.approx_zero_ulps(4) && ry.approx_zero_ulps(4) {
        rx = 1.0;
        ry = 1.0;
    } else if rx.approx_zero_ulps(4) {
        rx = 1.0;
    } else if ry.approx_zero_ulps(4) {
        ry = 1.0;
    }
    let rx = usvg::PositiveF32::new(rx * scale.width()).unwrap_or(usvg::PositiveF32::new(1.0).unwrap());
    let ry = usvg::PositiveF32::new(ry * scale.height()).unwrap_or(usvg::PositiveF32::new(1.0).unwrap());
    filter::Kind::Morphology(filter::Morphology::new(
        resolve_filter_input(element, "in", primitives),
        operator,
        rx,
        ry,
    ))
}

fn filter_diffuse_lighting(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let light_source = filter_light_source(element)?;
    Some(filter::Kind::DiffuseLighting(filter::DiffuseLighting::new(
        resolve_filter_input(element, "in", primitives),
        number_attr(element, "surfaceScale", 1.0),
        number_attr(element, "diffuseConstant", 1.0),
        filter_color(element, "lighting-color", usvg::Color::white()),
        light_source,
    )))
}

fn filter_specular_lighting(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let light_source = filter_light_source(element)?;
    let specular_exponent = number_attr(element, "specularExponent", 1.0);
    if !(1.0..=128.0).contains(&specular_exponent) {
        return None;
    }
    Some(filter::Kind::SpecularLighting(filter::SpecularLighting::new(
        resolve_filter_input(element, "in", primitives),
        number_attr(element, "surfaceScale", 1.0),
        number_attr(element, "specularConstant", 1.0),
        specular_exponent,
        filter_color(element, "lighting-color", usvg::Color::white()),
        light_source,
    )))
}

fn filter_light_source(element: &ServoLayoutElement<'_>) -> Option<filter::LightSource> {
    let child = element.as_node().dom_children().find_map(|c| {
        let el = c.as_element()?;
        let tag = el.local_name().to_string();
        matches!(tag.as_str(), "feDistantLight" | "fePointLight" | "feSpotLight")
            .then_some(el)
    })?;
    let tag = child.local_name().to_string();
    Some(match tag.as_str() {
        "feDistantLight" => filter::LightSource::DistantLight(filter::DistantLight {
            azimuth: number_attr(&child, "azimuth", 0.0),
            elevation: number_attr(&child, "elevation", 0.0),
        }),
        "fePointLight" => filter::LightSource::PointLight(filter::PointLight {
            x: number_attr(&child, "x", 0.0),
            y: number_attr(&child, "y", 0.0),
            z: number_attr(&child, "z", 0.0),
        }),
        "feSpotLight" => filter::LightSource::SpotLight(filter::SpotLight {
            x: number_attr(&child, "x", 0.0),
            y: number_attr(&child, "y", 0.0),
            z: number_attr(&child, "z", 0.0),
            points_at_x: number_attr(&child, "pointsAtX", 0.0),
            points_at_y: number_attr(&child, "pointsAtY", 0.0),
            points_at_z: number_attr(&child, "pointsAtZ", 0.0),
            specular_exponent: usvg::PositiveF32::new(number_attr(&child, "specularExponent", 1.0))
                .unwrap_or_else(|| usvg::PositiveF32::new(1.0).unwrap()),
            limiting_cone_angle: child
                .attribute_as_str(&ns!(), &LocalName::from("limitingConeAngle"))
                .and_then(|s| s.parse::<f32>().ok()),
        }),
        _ => return None,
    })
}

/// Reads a plain numeric attribute, falling back to `default`.
fn number_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default)
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
    fonts: &SvgFonts,
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

    // A `<use>` referencing a `<symbol>` establishes a new viewport (the symbol's
    // `viewBox` mapped onto the `<use>`'s `width`×`height`), handled separately.
    if element_layout_type(referenced) == LayoutElementType::SVGSymbolElement {
        return convert_use_symbol(
            node,
            referenced,
            computed.as_deref(),
            context,
            gradients,
            defs,
            diagonal,
            parent_abs_transform,
            fonts,
        );
    }

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
    for child_node in convert_node(
        referenced_node,
        context,
        gradients,
        defs,
        diagonal,
        abs_transform,
        computed.as_deref(),
        fonts,
    ) {
        group.push_child(child_node);
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Converts a `<use href="#symbol">` reference. Unlike `<use>` of a plain group, a
/// `<symbol>` establishes a new viewport: its `viewBox` is mapped onto the `<use>`'s
/// `x`/`y`/`width`/`height` rectangle (mirroring usvg's `use_node::convert`). The
/// structure matches [`convert_svg`]: an outer group carries the `transform`
/// attribute (plus an optional viewport clip), and an inner group carries the
/// viewport transform (`translate(x, y) · viewBox→viewport`).
fn convert_use_symbol(
    node: ServoLayoutNode<'_>,
    symbol: &ServoLayoutElement<'_>,
    host: Option<&ComputedValues>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
) -> Option<usvg::Node> {
    let element = node.as_element()?;

    // The `transform` attribute operates in the parent user space, before the new
    // viewport is established, so it stays on the outer group.
    let orig_ts = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);

    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);

    // Viewport transform: translate to (x, y), then map the symbol's `viewBox` onto
    // the `<use>`'s width×height rectangle (defaulting each to 100%, like usvg).
    let mut viewport_ts = usvg::Transform::from_translate(x, y);
    if let Some(vb) = parse_view_box(symbol) {
        let w = length_attr_opt(&element, "width").unwrap_or(100.0);
        let h = length_attr_opt(&element, "height").unwrap_or(100.0);
        if let Some(size) = usvg::Size::from_wh(w, h) {
            viewport_ts = viewport_ts.pre_concat(vb.to_transform(size));
        }
    }

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();
    group.transform = orig_ts;
    let abs_transform = parent_abs_transform.pre_concat(orig_ts);
    group.abs_transform = abs_transform;

    if let Some(computed) = host {
        group.opacity = usvg::Opacity::new(computed.get_effects().opacity)
            .unwrap_or(usvg::Opacity::ONE);
    }

    // A symbol establishes a viewport; clip to its rectangle unless `overflow` is
    // `visible` (or `auto`), mirroring the nested-`<svg>` handling in `convert_svg`.
    let overflow_visible = matches!(
        symbol.attribute_as_str(&ns!(), &LocalName::from("overflow")),
        Some("visible") | Some("auto")
    );
    if !overflow_visible {
        let w = length_attr_opt(&element, "width").unwrap_or(100.0);
        let h = length_attr_opt(&element, "height").unwrap_or(100.0);
        if let Some(clip_rect) = usvg::NonZeroRect::from_xywh(x, y, w, h) {
            group.clip_path = rect_clip_path(clip_rect);
        }
    }

    // Wrap the symbol's children in an inner group carrying the viewport transform.
    let mut inner = usvg::Group::empty();
    inner.transform = viewport_ts;
    inner.abs_transform = abs_transform.pre_concat(viewport_ts);

    let symbol_node = symbol.as_node();
    for child in symbol_node.dom_children() {
        for child_node in convert_node(
            child,
            context,
            gradients,
            defs,
            diagonal,
            inner.abs_transform,
            host,
            fonts,
        ) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}

/// Whether an element explicitly sets its own paint, in which case it should not
/// inherit fill/stroke from an enclosing `<use>` host.
fn element_has_explicit_fill(element: &ServoLayoutElement<'_>) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("fill"))
        .is_some()
        || element
            .attribute_as_str(&ns!(), &LocalName::from("style"))
            .is_some()
}

fn element_has_explicit_stroke(element: &ServoLayoutElement<'_>) -> bool {
    element
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
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
) -> Vec<usvg::Node> {
    let Some(data) = build_shape_path(element, ty, computed) else {
        return Vec::new();
    };

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
    let fill = fill_computed.and_then(|c| build_fill(c, gradients));
    let stroke = stroke_computed.and_then(|c| build_stroke(c, gradients, diagonal));

    // Marker scaling uses the stroke width in `markerUnits="strokeWidth"` mode
    // (the default). This is the raw stroke width, resolved independently of
    // whether a stroke is actually painted (`stroke="none"` still scales markers
    // at the default 1.0).
    let stroke_width = stroke_computed
        .map(|c| {
            let inherited = c.get_inherited_svg();
            match &inherited.stroke_width {
                SVGLength::LengthPercentage(nn_lp) => {
                    nn_lp.0.resolve(Length::new(diagonal)).px()
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
    .map(|p| usvg::Node::Path(Box::new(p)))
    else {
        return Vec::new();
    };

    let mut nodes = vec![path_node];
    nodes.extend(build_markers(
        element,
        &data,
        stroke_width,
        context,
        gradients,
        defs,
        diagonal,
        abs_transform,
        fonts,
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
    let clip_path = match resolve_clip_path(
        element,
        context,
        gradients,
        defs,
        diagonal,
        fonts,
        object_bbox,
    ) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return Vec::new(),
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, context, gradients, defs, diagonal, fonts, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    let filter = match resolve_filter(element, defs, object_bbox) {
        FilterOutcome::Filter(filter) => Some(filter),
        FilterOutcome::Invalid => return Vec::new(),
        FilterOutcome::None => None,
    };
    if !transform.is_identity()
        || element_opacity < 1.0
        || clip_path.is_some()
        || mask.is_some()
        || filter.is_some()
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

/// Computes the aligned origin for a `preserveAspectRatio` fit, mirroring usvg's
/// `crate::aligned_pos` (which is crate-private).
fn aligned_pos(align: svgtypes::Align, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    match align {
        svgtypes::Align::None | svgtypes::Align::XMinYMin => (x, y),
        svgtypes::Align::XMidYMin => (x + w / 2.0, y),
        svgtypes::Align::XMaxYMin => (x + w, y),
        svgtypes::Align::XMinYMid => (x, y + h / 2.0),
        svgtypes::Align::XMidYMid => (x + w / 2.0, y + h / 2.0),
        svgtypes::Align::XMaxYMid => (x + w, y + h / 2.0),
        svgtypes::Align::XMinYMax => (x, y + h),
        svgtypes::Align::XMidYMax => (x + w / 2.0, y + h),
        svgtypes::Align::XMaxYMax => (x + w, y + h),
    }
}

/// Converts an `<image>` element into a [`usvg::Image`] node, reading the raster
/// data from the element's `href` (or `xlink:href`) data URI.
///
/// External URLs are not yet supported: Servo's image pipeline only exposes
/// *decoded* pixels, not the raw encoded bytes usvg's `ImageKind::{PNG,JPEG,…}`
/// expects. The image is always wrapped in an inner group carrying the position/scale
/// (align) transform, because resvg positions an image by transforming its intrinsic
/// (0,0,w,h) rect with the *accumulated group* transform (never `abs_transform`).
/// When the element has a clip-path/mask/filter/opacity, those go on an *outer*
/// group (transform = the element's own `transform` attribute), matching usvg's
/// parser, so a `userSpaceOnUse` clip isn't distorted by the align scale.
fn convert_image(
    element: &ServoLayoutElement<'_>,
    computed: Option<&ComputedValues>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
) -> Vec<usvg::Node> {
    let Some(href) = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))
    else {
        return Vec::new();
    };

    let Ok(data_url) = DataUrl::process(href) else {
        // External URL (or malformed data URI): unsupported synchronously.
        return Vec::new();
    };
    let mime = data_url.mime_type();
    if mime.type_ != "image" {
        return Vec::new();
    }

    let Ok((bytes, _fragment)) = data_url.decode_to_vec() else {
        return Vec::new();
    };

    // Header-only probe of the intrinsic size; resvg decodes the bytes for real at
    // render time, so we hand it the raw encoded data, not pixels.
    let Some((w, h)) = ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
    else {
        return Vec::new();
    };
    let (intrinsic_w, intrinsic_h) = (w as f32, h as f32);

    let Some(size) = usvg::Size::from_wh(intrinsic_w, intrinsic_h) else {
        return Vec::new();
    };

    let kind = match mime.subtype.as_str() {
        "png" => usvg::ImageKind::PNG(Arc::new(bytes)),
        "jpeg" | "jpg" => usvg::ImageKind::JPEG(Arc::new(bytes)),
        "gif" => usvg::ImageKind::GIF(Arc::new(bytes)),
        "webp" => usvg::ImageKind::WEBP(Arc::new(bytes)),
        _ => return Vec::new(),
    };

    // Geometry: x/y default to 0, width/height default to the intrinsic size, and
    // when only one of width/height is set the other preserves aspect ratio
    // (mirroring usvg's `parser::image::convert`).
    let x = length_attr(element, "x", 0.0);
    let y = length_attr(element, "y", 0.0);
    let width_attr = length_attr_opt(element, "width");
    let height_attr = length_attr_opt(element, "height");
    let (width, height) = match (width_attr, height_attr) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, intrinsic_h * (w / intrinsic_w)),
        (None, Some(h)) => (intrinsic_w * (h / intrinsic_h), h),
        (None, None) => (intrinsic_w, intrinsic_h),
    };

    // `preserveAspectRatio` (default `xMidYMid meet`) fits the intrinsic image into
    // the x/y/width/height box, scaling *uniformly* unless `none` is requested. This
    // mirrors usvg's `parser::image::convert_inner` (fit_view_box + aligned_pos).
    let Some(rect) = usvg::NonZeroRect::from_xywh(x, y, width, height) else {
        return Vec::new();
    };
    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();
    let rect_size = rect.size();
    let aligned_size = if aspect.align == svgtypes::Align::None {
        rect_size
    } else if aspect.slice {
        size.expand_to(rect_size)
    } else {
        size.scale_to(rect_size)
    };
    let (aligned_x, aligned_y) = aligned_pos(
        aspect.align,
        rect.x(),
        rect.y(),
        rect.width() - aligned_size.width(),
        rect.height() - aligned_size.height(),
    );
    let view_box = aligned_size.to_non_zero_rect(aligned_x, aligned_y);

    // translate to the aligned origin, then scale the intrinsic size onto the aligned
    // box, with the element's own `transform` attribute applied on top (mirroring how
    // `build_shape_node` wraps a shape's `transform`).
    let translate_scale = usvg::Transform::from_row(
        view_box.width() / intrinsic_w,
        0.0,
        0.0,
        view_box.height() / intrinsic_h,
        view_box.x(),
        view_box.y(),
    );
    let element_transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let image_ts = element_transform.pre_concat(translate_scale);
    let abs_transform = parent_abs_transform.pre_concat(image_ts);

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

    let Some(image) = usvg::Image::new(
        id,
        visible,
        size,
        usvg::ImageRendering::default(),
        kind,
        abs_transform,
    ) else {
        return Vec::new();
    };

    // Resolve clip-path/mask/filter against the image's bounding box, then wrap the
    // image (see the two-group structure below).
    let object_bbox = usvg::NonZeroRect::from_xywh(x, y, width, height);
    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let clip_path = match resolve_clip_path(
        element,
        context,
        gradients,
        defs,
        diagonal,
        fonts,
        object_bbox,
    ) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return Vec::new(),
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, context, gradients, defs, diagonal, fonts, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    let filter = match resolve_filter(element, defs, object_bbox) {
        FilterOutcome::Filter(filter) => Some(filter),
        FilterOutcome::Invalid => return Vec::new(),
        FilterOutcome::None => None,
    };

    // The image is always wrapped in an inner group carrying the align transform
    // (x/y/width/height/preserveAspectRatio position+scale). clip-path/mask/filter
    // must NOT live on this group: resvg isolates the group that carries them and
    // applies the clip/mask in that group's shifted *local* coordinate system, so a
    // `userSpaceOnUse` clip authored in the element's user space would be scaled by
    // the align transform and clip the image away. Instead those properties go on an
    // *outer* group whose transform is the element's own `transform` attribute — the
    // same two-group structure usvg's parser produces (`image::convert_inner` wraps
    // the aligned group in a `convert_group` carrying the clip/mask/filter/opacity).
    let mut inner = usvg::Group::empty();
    inner.transform = translate_scale;
    inner.abs_transform = abs_transform;
    inner.push_child(usvg::Node::Image(Box::new(image)));

    if !element_transform.is_identity()
        || element_opacity < 1.0
        || clip_path.is_some()
        || mask.is_some()
        || filter.is_some()
    {
        let mut outer = usvg::Group::empty();
        outer.transform = element_transform;
        outer.abs_transform = parent_abs_transform.pre_concat(element_transform);
        outer.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        outer.clip_path = clip_path;
        outer.mask = mask;
        if let Some(filter) = filter {
            outer.filters.push(filter);
        }
        outer.push_child(usvg::Node::Group(Box::new(inner)));
        vec![usvg::Node::Group(Box::new(outer))]
    } else {
        vec![usvg::Node::Group(Box::new(inner))]
    }
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

/// A marker path segment. `QuadTo` is resolved to `CubicTo` up front (mirroring
/// usvg's `parser::marker::Segment`), since the vertex-tangent math only handles
/// lines and cubics.
#[derive(Copy, Clone, Debug)]
enum MarkerSegment {
    MoveTo(tiny_skia_path::Point),
    LineTo(tiny_skia_path::Point),
    CubicTo(tiny_skia_path::Point, tiny_skia_path::Point, tiny_skia_path::Point),
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

/// Monotonic id counter for the synthetic clip paths that `<marker>` overflow
/// clipping produces. The id only matters for SVG re-serialization; resvg keys clip
/// paths by pointer identity, so it just needs to be a valid non-empty string.
static MARKER_CLIP_ID: AtomicU32 = AtomicU32::new(0);
/// Monotonic counter for synthetic clip-path ids used by nested-`<svg>` viewport
/// clipping.
static SVG_CLIP_ID: AtomicU32 = AtomicU32::new(0);

/// Whether `element` is a `<marker>` element. Markers are only referenced by `id`
/// (they are not rendered on their own and have no dedicated DOM type), so they are
/// discriminated by local name.
fn is_marker_element(element: &ServoLayoutElement<'_>) -> bool {
    element.local_name() == &LocalName::from("marker")
}

/// Builds the marker groups for `element`'s shape `path`, returning them as sibling
/// [`usvg::Node`]s to be placed after the path (the default paint order draws
/// markers last).
fn build_markers(
    element: &ServoLayoutElement<'_>,
    path: &tiny_skia_path::Path,
    stroke_width: f32,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    shape_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
) -> Vec<usvg::Node> {
    let kinds = [
        ("marker-start", MarkerKind::Start),
        ("marker-mid", MarkerKind::Middle),
        ("marker-end", MarkerKind::End),
    ];

    // Resolve the three marker references up front so the shared segment list is
    // only built when at least one marker is actually referenced.
    let mut refs: Vec<(MarkerKind, ServoLayoutElement<'_>)> = Vec::new();
    for (attr, kind) in kinds {
        if let Some(marker) = marker_reference(element, attr)
            .and_then(|id| defs.get(&id))
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
            context,
            gradients,
            defs,
            diagonal,
            shape_abs_transform,
            fonts,
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

/// Places one `<marker>` at each vertex of `kind`, appending the resulting groups
/// to `out`. This mirrors usvg's `parser::marker::resolve`.
fn resolve_marker(
    segments: &[MarkerSegment],
    marker: ServoLayoutElement<'_>,
    kind: MarkerKind,
    stroke_width: f32,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    shape_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
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
            let size =
                usvg::Size::from_wh(r.width() * stroke_scale, r.height() * stroke_scale)
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
            for child_node in convert_node(
                child,
                context,
                gradients,
                defs,
                diagonal,
                g.abs_transform,
                None,
                fonts,
            ) {
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
                calc_curves_angle(prev.x, prev.y, prev.x, prev.y, pl.x, pl.y, p1.x, p1.y, p.x, p.y)
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
        if v < 0.0 {
            v + PI * 2.0
        } else {
            v
        }
    }

    fn vector_angle(vx: f32, vy: f32) -> f32 {
        let rad = vy.atan2(vx);
        if rad.is_nan() {
            0.0
        } else {
            normalize(rad)
        }
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
) -> (tiny_skia_path::Point, tiny_skia_path::Point, tiny_skia_path::Point) {
    fn calc(n1: f32, n2: f32) -> f32 {
        (n1 + n2 * 2.0) / 3.0
    }

    (
        tiny_skia_path::Point::from_xy(calc(prev.x, p1.x), calc(prev.y, p1.y)),
        tiny_skia_path::Point::from_xy(calc(p.x, p1.x), calc(p.y, p1.y)),
        p,
    )
}

fn build_fill(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let (paint, color_alpha) = resolve_paint(&inherited.fill, computed, gradients)?;

    // `fill-opacity` multiplies the alpha already carried by the `fill` color
    // (e.g. `fill="rgba(..., 0.4)"`), per SVG2.
    let opacity = (match inherited.fill_opacity {
        SVGOpacity::Opacity(op) => op,
        _ => 1.0,
    } * color_alpha)
        .clamp(0.0, 1.0);
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
    let (paint, color_alpha) = resolve_paint(&inherited.stroke, computed, gradients)?;

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
    // `stroke-opacity` multiplies the alpha already carried by the `stroke`
    // color, mirroring the fill path above.
    stroke.opacity = match inherited.stroke_opacity {
        SVGOpacity::Opacity(op) => usvg::Opacity::new((op * color_alpha).clamp(0.0, 1.0))
            .unwrap_or(usvg::Opacity::ONE),
        _ => usvg::Opacity::new(color_alpha).unwrap_or(usvg::Opacity::ONE),
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

    Some(stroke)
}

fn resolve_paint(
    svg_paint: &SVGPaint,
    computed: &ComputedValues,
    gradients: &Gradients,
) -> Option<(usvg::Paint, f32)> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            // `fill`/`stroke` may carry an alpha channel (`rgba()`, `hsla()`,
            // 4/8-digit hex, `color(..., / alpha)`). usvg's `Paint::Color` is
            // RGB-only, so we fold the color's own alpha into the fill/stroke
            // opacity instead of dropping it (which would render the shape at
            // full opacity).
            let alpha = srgb.alpha.clamp(0.0, 1.0);
            Some((
                usvg::Paint::Color(usvg::Color::new_rgb(
                    (srgb.components.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (srgb.components.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (srgb.components.2.clamp(0.0, 1.0) * 255.0).round() as u8,
                )),
                alpha,
            ))
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
            let paint = if let Some(g) = gradients.linear.get(&fragment) {
                usvg::Paint::LinearGradient(g.clone())
            } else if let Some(g) = gradients.radial.get(&fragment) {
                usvg::Paint::RadialGradient(g.clone())
            } else if let Some(p) = gradients.pattern.get(&fragment) {
                usvg::Paint::Pattern(p.clone())
            } else {
                // Unresolved paint server: fall back to black, matching usvg's
                // behaviour when a referenced paint server is missing.
                usvg::Paint::Color(usvg::Color::black())
            };
            Some((paint, 1.0))
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

// ===== Text support =====

/// A text character position. _Character_ is a Unicode codepoint, per SVG 2.
#[derive(Clone, Copy, Debug)]
struct CharacterPosition {
    /// An absolute X axis position.
    x: Option<f32>,
    /// An absolute Y axis position.
    y: Option<f32>,
    /// A relative X axis offset.
    dx: Option<f32>,
    /// A relative Y axis offset.
    dy: Option<f32>,
}

/// State threaded through the chunk-collection walk.
struct IterState {
    chars_count: usize,
    chunk_bytes_count: usize,
    split_chunk: bool,
    text_flow: usvg::TextFlow,
    chunks: Vec<usvg::TextChunk>,
}

/// Parses a whitespace/comma-separated list of numbers (for `x`/`y`/`dx`/`dy`/
/// `rotate` text-positioning lists).
fn parse_number_list(value: &str) -> Vec<f32> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(parse_length_attr)
        .collect()
}

/// SVG whitespace handling mode, driven by the `xml:space` attribute.
#[derive(Clone, Copy, PartialEq, Eq)]
enum XmlSpace {
    Default,
    Preserve,
}

fn xml_space(element: &ServoLayoutElement<'_>) -> Option<XmlSpace> {
    match element.attribute_as_str(&ns!(xml), &LocalName::from("space")) {
        Some("preserve") => Some(XmlSpace::Preserve),
        Some(_) => Some(XmlSpace::Default),
        None => None,
    }
}

/// Collapses whitespace in a single text node per the SVG whitespace spec:
/// line breaks and tabs become spaces, and (in the default mode) runs of spaces
/// collapse to a single space. Ported from usvg's `trim_text`.
fn trim_text(text: &str, space: XmlSpace) -> String {
    let mut s = String::with_capacity(text.len());
    let mut prev = '0';
    for c in text.chars() {
        let c = match c {
            '\r' | '\n' | '\t' => ' ',
            _ => c,
        };
        if space == XmlSpace::Default && c == ' ' && c == prev {
            continue;
        }
        prev = c;
        s.push(c);
    }
    s
}

/// The fully whitespace-trimmed text content for each text node, keyed by the
/// node's opaque identity. Positions (`x`/`y`/`dx`/`dy`/`rotate`) and span
/// building are all resolved against this trimmed text.
type TrimmedTexts = HashMap<OpaqueNode, String>;

/// A text node captured during the whitespace-trimming walk.
struct RawTextNode<'a> {
    node: ServoLayoutNode<'a>,
    depth: usize,
    xml_space: XmlSpace,
    text: String,
}

fn collect_raw_text_nodes<'a>(
    node: ServoLayoutNode<'a>,
    depth: usize,
    inherited: XmlSpace,
    out: &mut Vec<RawTextNode<'a>>,
) {
    for child in node.dom_children() {
        if child.is_text_node() {
            out.push(RawTextNode {
                node: child,
                depth,
                xml_space: inherited,
                text: child.text_content().to_string(),
            });
        } else if let Some(child_element) = child.as_element() {
            let space = xml_space(&child_element).unwrap_or(inherited);
            collect_raw_text_nodes(child, depth + 1, space, out);
        }
    }
}

fn remove_first_space(s: &mut String) {
    debug_assert!(s.starts_with(' '));
    s.remove(0);
}

fn remove_last_space(s: &mut String) {
    debug_assert!(s.ends_with(' '));
    s.pop();
}

/// Removes leading/trailing spaces and collapses spaces at the boundaries
/// between adjacent text nodes, ported from usvg's `trim_text_nodes`.
fn boundary_trim(nodes: &mut [RawTextNode]) {
    let len = nodes.len();
    if len == 0 {
        return;
    }
    if len == 1 {
        if nodes[0].xml_space == XmlSpace::Default {
            nodes[0].text = nodes[0].text.trim_matches(' ').to_string();
        }
        return;
    }

    let mut i = 0;
    while i < len - 1 {
        let idx2 = i + 1;
        let (left, right) = nodes.split_at_mut(idx2);
        let node1 = &mut left[i];
        let node2 = &mut right[0];

        let xmlspace1 = node1.xml_space;
        let xmlspace2 = node2.xml_space;
        let depth1 = node1.depth;
        let depth2 = node2.depth;

        let c1 = node1.text.as_bytes().first().copied();
        let c2 = node1.text.as_bytes().last().copied();
        let c3 = node2.text.as_bytes().first().copied();
        let c4 = node2.text.as_bytes().last().copied();

        if depth1 < depth2 {
            if c3 == Some(b' ') && xmlspace2 == XmlSpace::Default {
                remove_first_space(&mut node2.text);
            }
        } else if c2 == Some(b' ') && c2 == c3 {
            if xmlspace1 == XmlSpace::Default && xmlspace2 == XmlSpace::Default {
                remove_last_space(&mut node1.text);
            } else if xmlspace1 == XmlSpace::Preserve && xmlspace2 == XmlSpace::Default {
                remove_first_space(&mut node2.text);
            }
        }

        let is_first = i == 0;
        let is_last = i == len - 1;

        if is_first && c1 == Some(b' ') && xmlspace1 == XmlSpace::Default && !node1.text.is_empty() {
            remove_first_space(&mut node1.text);
        } else if is_last && c4 == Some(b' ') && !node2.text.is_empty() && xmlspace2 == XmlSpace::Default {
            remove_last_space(&mut node2.text);
        }

        if is_last && c2 == Some(b' ') && !node1.text.is_empty() && node2.text.is_empty() && node1.text.ends_with(' ') {
            remove_last_space(&mut node1.text);
        }

        i += 1;
    }
}

/// Collects the whitespace-trimmed text for every text node under a `<text>`
/// element, keyed by opaque node identity.
fn trim_text_tree(root: ServoLayoutNode<'_>) -> TrimmedTexts {
    let inherited = root
        .as_element()
        .and_then(|e| xml_space(&e))
        .unwrap_or(XmlSpace::Default);
    let mut nodes = Vec::new();
    collect_raw_text_nodes(root, 0, inherited, &mut nodes);
    for n in &mut nodes {
        n.text = trim_text(&n.text, n.xml_space);
    }
    boundary_trim(&mut nodes);
    nodes
        .into_iter()
        .map(|n| (n.node.opaque(), n.text))
        .collect()
}

/// Counts the Unicode codepoints across all text descendants of `node`, using
/// the whitespace-trimmed text.
fn count_chars(node: ServoLayoutNode<'_>, texts: &TrimmedTexts) -> usize {
    node.dom_children()
        .map(|child| {
            if child.as_element().is_some() {
                count_chars(child, texts)
            } else {
                texts
                    .get(&child.opaque())
                    .map(|s| s.chars().count())
                    .unwrap_or(0)
            }
        })
        .sum()
}

/// Resolves per-character `x`/`y`/`dx`/`dy` positions, ported from usvg's
/// `resolve_positions_list`.
fn resolve_positions_list(node: ServoLayoutNode<'_>, texts: &TrimmedTexts) -> Vec<CharacterPosition> {
    let mut list = vec![
        CharacterPosition {
            x: None,
            y: None,
            dx: None,
            dy: None,
        };
        count_chars(node, texts)
    ];
    let mut offset = 0usize;
    resolve_positions_impl(node, &mut list, &mut offset, texts);
    list
}

fn resolve_positions_impl(
    node: ServoLayoutNode<'_>,
    list: &mut [CharacterPosition],
    offset: &mut usize,
    texts: &TrimmedTexts,
) {
    if let Some(element) = node.as_element() {
        let ty = element_layout_type(&element);
        if matches!(
            ty,
            LayoutElementType::SVGTextElement | LayoutElementType::SVGTSpanElement
        ) {
            let child_chars = count_chars(node, texts);
            macro_rules! push_list {
                ($attr:literal, $field:ident) => {
                    if let Some(value) =
                        element.attribute_as_str(&ns!(), &LocalName::from($attr))
                    {
                        let nums = parse_number_list(value);
                        let len = nums.len().min(child_chars);
                        for i in 0..len {
                            list[*offset + i].$field = Some(nums[i]);
                        }
                    }
                };
            }
            push_list!("x", x);
            push_list!("y", y);
            push_list!("dx", dx);
            push_list!("dy", dy);
        }
        for child in node.dom_children() {
            resolve_positions_impl(child, list, offset, texts);
        }
    } else if node.is_text_node() {
        *offset += texts
            .get(&node.opaque())
            .map(|s| s.chars().count())
            .unwrap_or(0);
    }
}

/// Resolves per-character rotation, ported from usvg's `resolve_rotate_list`.
fn resolve_rotate_list(node: ServoLayoutNode<'_>, texts: &TrimmedTexts) -> Vec<f32> {
    let mut list = vec![0.0; count_chars(node, texts)];
    let mut last = 0.0;
    let mut offset = 0usize;
    resolve_rotate_impl(node, &mut list, &mut offset, &mut last, texts);
    list
}

fn resolve_rotate_impl(
    node: ServoLayoutNode<'_>,
    list: &mut [f32],
    offset: &mut usize,
    last: &mut f32,
    texts: &TrimmedTexts,
) {
    if let Some(element) = node.as_element() {
        if let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from("rotate")) {
            let rotate = parse_number_list(value);
            let child_chars = count_chars(node, texts);
            for i in 0..child_chars {
                if let Some(a) = rotate.get(i).copied() {
                    list[*offset + i] = a;
                    *last = a;
                } else {
                    list[*offset + i] = *last;
                }
            }
        }
        for child in node.dom_children() {
            resolve_rotate_impl(child, list, offset, last, texts);
        }
    } else if node.is_text_node() {
        *offset += texts
            .get(&node.opaque())
            .map(|s| s.chars().count())
            .unwrap_or(0);
    }
}

fn text_anchor(computed: &ComputedValues) -> usvg::TextAnchor {
    match computed.get_inherited_svg().text_anchor {
        style::computed_values::text_anchor::T::Middle => usvg::TextAnchor::Middle,
        style::computed_values::text_anchor::T::End => usvg::TextAnchor::End,
        _ => usvg::TextAnchor::Start,
    }
}

fn dominant_baseline(computed: &ComputedValues) -> usvg::DominantBaseline {
    use style::values::computed::DominantBaseline;
    match computed.get_inherited_box().dominant_baseline {
        DominantBaseline::Alphabetic => usvg::DominantBaseline::Alphabetic,
        DominantBaseline::Ideographic => usvg::DominantBaseline::Ideographic,
        DominantBaseline::Hanging => usvg::DominantBaseline::Hanging,
        DominantBaseline::Mathematical => usvg::DominantBaseline::Mathematical,
        DominantBaseline::Central => usvg::DominantBaseline::Central,
        DominantBaseline::Middle => usvg::DominantBaseline::Middle,
        DominantBaseline::TextTop => usvg::DominantBaseline::TextBeforeEdge,
        DominantBaseline::TextBottom => usvg::DominantBaseline::TextAfterEdge,
        _ => usvg::DominantBaseline::Auto,
    }
}

fn alignment_baseline(computed: &ComputedValues) -> usvg::AlignmentBaseline {
    use style::values::computed::AlignmentBaseline;
    match computed.get_box().alignment_baseline {
        AlignmentBaseline::Baseline => usvg::AlignmentBaseline::Baseline,
        AlignmentBaseline::TextBottom => usvg::AlignmentBaseline::TextAfterEdge,
        AlignmentBaseline::Middle => usvg::AlignmentBaseline::Middle,
        AlignmentBaseline::TextTop => usvg::AlignmentBaseline::TextBeforeEdge,
        _ => usvg::AlignmentBaseline::Auto,
    }
}

fn baseline_shift(computed: &ComputedValues) -> Vec<usvg::BaselineShift> {
    use style::values::computed::BaselineShift as ServoBaselineShift;
    match &computed.get_box().baseline_shift {
        ServoBaselineShift::Keyword(kw) => match kw {
            style::values::generics::box_::BaselineShiftKeyword::Sub => {
                vec![usvg::BaselineShift::Subscript]
            },
            style::values::generics::box_::BaselineShiftKeyword::Super => {
                vec![usvg::BaselineShift::Superscript]
            },
            _ => Vec::new(),
        },
        ServoBaselineShift::Length(lp) => match lp.to_length() {
            Some(length) if length.px() != 0.0 => vec![usvg::BaselineShift::Number(length.px())],
            _ => Vec::new(),
        },
    }
}

fn letter_spacing(computed: &ComputedValues) -> f32 {
    computed
        .get_inherited_text()
        .letter_spacing
        .0
        .to_length()
        .map(|l| l.px())
        .unwrap_or(0.0)
}

fn word_spacing(computed: &ComputedValues) -> f32 {
    computed
        .get_inherited_text()
        .word_spacing
        .to_length()
        .map(|l| l.px())
        .unwrap_or(0.0)
}

fn length_adjust(element: &ServoLayoutElement<'_>) -> usvg::LengthAdjust {
    match element.attribute_as_str(&ns!(), &LocalName::from("lengthAdjust")) {
        Some("spacingAndGlyphs") => usvg::LengthAdjust::SpacingAndGlyphs,
        _ => usvg::LengthAdjust::Spacing,
    }
}

fn convert_writing_mode(element: &ServoLayoutElement<'_>) -> usvg::WritingMode {
    match element.attribute_as_str(&ns!(), &LocalName::from("writing-mode")) {
        Some("tb") | Some("tb-rl") | Some("vertical-rl") | Some("vertical-lr") => {
            usvg::WritingMode::TopToBottom
        },
        _ => usvg::WritingMode::LeftToRight,
    }
}

fn convert_direction(computed: &ComputedValues) -> usvg::TextDirection {
    match computed.get_inherited_box().direction {
        style::computed_values::direction::T::Rtl => usvg::TextDirection::RightToLeft,
        _ => usvg::TextDirection::LeftToRight,
    }
}

/// Maps a Servo `SingleFontFamily` onto usvg's `svgtypes::FontFamily`.
fn font_family_to_usvg(family: &SingleFontFamily) -> usvg::FontFamily {
    match family {
        SingleFontFamily::FamilyName(name) => usvg::FontFamily::Named(name.name.to_string()),
        SingleFontFamily::Generic(generic) => match generic {
            style::values::computed::font::GenericFontFamily::Serif => usvg::FontFamily::Serif,
            style::values::computed::font::GenericFontFamily::SansSerif => {
                usvg::FontFamily::SansSerif
            },
            style::values::computed::font::GenericFontFamily::Cursive => usvg::FontFamily::Cursive,
            style::values::computed::font::GenericFontFamily::Fantasy => usvg::FontFamily::Fantasy,
            style::values::computed::font::GenericFontFamily::Monospace => {
                usvg::FontFamily::Monospace
            },
            _ => usvg::FontFamily::SansSerif,
        },
    }
}

/// Builds a [`usvg::Font`] from a Servo computed `font-*` style.
fn convert_font(computed: &ComputedValues) -> usvg::Font {
    let font = computed.get_font();

    let style = if font.font_style == ServoFontStyle::NORMAL {
        usvg::FontStyle::Normal
    } else if font.font_style == ServoFontStyle::ITALIC {
        usvg::FontStyle::Italic
    } else {
        usvg::FontStyle::Oblique
    };

    let stretch = match font.font_stretch.as_keyword() {
        Some(FontStretchKeyword::UltraCondensed) => usvg::FontStretch::UltraCondensed,
        Some(FontStretchKeyword::ExtraCondensed) => usvg::FontStretch::ExtraCondensed,
        Some(FontStretchKeyword::Condensed) => usvg::FontStretch::Condensed,
        Some(FontStretchKeyword::SemiCondensed) => usvg::FontStretch::SemiCondensed,
        Some(FontStretchKeyword::Normal) => usvg::FontStretch::Normal,
        Some(FontStretchKeyword::SemiExpanded) => usvg::FontStretch::SemiExpanded,
        Some(FontStretchKeyword::Expanded) => usvg::FontStretch::Expanded,
        Some(FontStretchKeyword::ExtraExpanded) => usvg::FontStretch::ExtraExpanded,
        Some(FontStretchKeyword::UltraExpanded) => usvg::FontStretch::UltraExpanded,
        None => usvg::FontStretch::Normal,
    };

    let weight = font.font_weight.value().round() as u16;

    let variations = font
        .clone_font_variation_settings()
        .0
        .iter()
        .map(|setting| usvg::FontVariation::new(setting.tag.0.to_be_bytes(), setting.value))
        .collect();

    let mut families: Vec<usvg::FontFamily> =
        font.font_family.families.iter().map(font_family_to_usvg).collect();
    if families.is_empty() {
        families.push(usvg::FontFamily::SansSerif);
    }

    usvg::Font {
        families,
        style,
        stretch,
        weight,
        variations,
    }
}

/// Builds the `text-decoration` for a span from the computed
/// `text-decoration-line` (wired into presentational hints in `svgelement.rs`).
fn text_decoration(
    computed: &ComputedValues,
    gradients: &Gradients,
    diagonal: f32,
) -> usvg::TextDecoration {
    let line = computed.get_text().text_decoration_line;
    let make_deco = |has: bool| -> Option<usvg::TextDecorationStyle> {
        if !has {
            return None;
        }
        Some(usvg::TextDecorationStyle {
            fill: build_fill(computed, gradients),
            stroke: build_stroke(computed, gradients, diagonal),
        })
    };

    usvg::TextDecoration {
        underline: make_deco(line.contains(style::values::specified::TextDecorationLine::UNDERLINE)),
        overline: make_deco(line.contains(style::values::specified::TextDecorationLine::OVERLINE)),
        line_through: make_deco(
            line.contains(style::values::specified::TextDecorationLine::LINE_THROUGH),
        ),
    }
}

/// Builds a [`usvg::TextSpan`] from an element's computed style + text attributes.
fn build_text_span(
    element: &ServoLayoutElement<'_>,
    computed: &ComputedValues,
    font_size: usvg::NonZeroPositiveF32,
    gradients: &Gradients,
    diagonal: f32,
) -> usvg::TextSpan {
    let visible = !matches!(
        computed.get_inherited_box().visibility,
        style::computed_values::visibility::T::Hidden |
            style::computed_values::visibility::T::Collapse
    );

    let small_caps =
        computed.get_font().font_variant_caps == style::computed_values::font_variant_caps::T::SmallCaps;

    usvg::TextSpan {
        start: 0,
        end: 0,
        fill: build_fill(computed, gradients),
        stroke: build_stroke(computed, gradients, diagonal),
        paint_order: usvg::PaintOrder::default(),
        font: convert_font(computed),
        font_size,
        small_caps,
        apply_kerning: true,
        font_optical_sizing: usvg::FontOpticalSizing::Auto,
        decoration: text_decoration(computed, gradients, diagonal),
        dominant_baseline: dominant_baseline(computed),
        alignment_baseline: alignment_baseline(computed),
        baseline_shift: baseline_shift(computed),
        visible,
        letter_spacing: letter_spacing(computed),
        word_spacing: word_spacing(computed),
        text_length: length_attr_opt(element, "textLength").filter(|v| *v >= 0.0),
        length_adjust: length_adjust(element),
    }
}

/// Collects the [`usvg::TextChunk`]s for a `<text>` element, ported from usvg's
/// `collect_text_chunks`.
fn collect_text_chunks(
    element: &ServoLayoutElement<'_>,
    pos_list: &[CharacterPosition],
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    texts: &TrimmedTexts,
) -> Vec<usvg::TextChunk> {
    let mut state = IterState {
        chars_count: 0,
        chunk_bytes_count: 0,
        split_chunk: false,
        text_flow: usvg::TextFlow::Linear,
        chunks: Vec::new(),
    };
    collect_chunks_impl(element, pos_list, context, gradients, defs, diagonal, &mut state, texts);
    state.chunks
}

fn collect_chunks_impl(
    element: &ServoLayoutElement<'_>,
    pos_list: &[CharacterPosition],
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    state: &mut IterState,
    texts: &TrimmedTexts,
) {
    for child in element.as_node().dom_children() {
        if let Some(child_element) = child.as_element() {
            // `<textPath>` (text-on-path) must be a direct child of `<text>`; any
            // nested `<textPath>` is ignored. Resolve its referenced path and switch
            // the current text flow, then split the chunk on either side of it.
            let is_text_path = child_element.local_name() == &LocalName::from("textPath");
            if is_text_path {
                if element_layout_type(element) != LayoutElementType::SVGTextElement {
                    state.chars_count += count_chars(child, texts);
                    continue;
                }

                match resolve_text_flow(&child_element, context, defs) {
                    Some(flow) => state.text_flow = flow,
                    None => {
                        // Skip an invalid text path and all its children. We still
                        // advance the chars count because `pos_list` was built
                        // including this subtree.
                        state.chars_count += count_chars(child, texts);
                        continue;
                    },
                }

                state.split_chunk = true;
            }

            collect_chunks_impl(&child_element, pos_list, context, gradients, defs, diagonal, state, texts);

            state.text_flow = usvg::TextFlow::Linear;

            // The next character after a `textPath` must start a new chunk too.
            if is_text_path {
                state.split_chunk = true;
            }

            continue;
        }

        if !child.is_text_node() {
            continue;
        }
        let Some(text) = texts.get(&child.opaque()).cloned() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }

        let computed = element.style(&context.style_context);

        let Some(font_size) = usvg::NonZeroPositiveF32::new(
            computed.get_font().font_size.computed_size().px(),
        ) else {
            // A zero font size makes the span invalid; skip it.
            state.chars_count += text.chars().count();
            continue;
        };

        let span = build_text_span(element, &computed, font_size, gradients, diagonal);
        let anchor = text_anchor(&computed);

        let mut is_new_span = true;
        for c in text.chars() {
            let char_len = c.len_utf8();

            // A new chunk starts on the first span, whenever a character has an
            // absolute x/y coordinate, and after a `<textPath>` boundary.
            let is_new_chunk = pos_list[state.chars_count].x.is_some()
                || pos_list[state.chars_count].y.is_some()
                || state.split_chunk
                || state.chunks.is_empty();

            state.split_chunk = false;

            if is_new_chunk {
                state.chunk_bytes_count = 0;
                let mut span2 = span.clone();
                span2.start = 0;
                span2.end = char_len;
                state.chunks.push(usvg::TextChunk {
                    x: pos_list[state.chars_count].x,
                    y: pos_list[state.chars_count].y,
                    anchor,
                    spans: vec![span2],
                    text_flow: state.text_flow.clone(),
                    text: c.to_string(),
                });
            } else if is_new_span {
                let mut span2 = span.clone();
                span2.start = state.chunk_bytes_count;
                span2.end = state.chunk_bytes_count + char_len;
                if let Some(chunk) = state.chunks.last_mut() {
                    chunk.text.push(c);
                    chunk.spans.push(span2);
                }
            } else if let Some(chunk) = state.chunks.last_mut() {
                chunk.text.push(c);
                if let Some(span) = chunk.spans.last_mut() {
                    span.end += char_len;
                }
            }

            is_new_span = false;
            state.chars_count += 1;
            state.chunk_bytes_count += char_len;
        }
    }
}

/// Resolves a `<textPath>` element into a [`usvg::TextFlow::Path`], ported from
/// usvg's `resolve_text_flow`: the `href` target is converted to a path outline,
/// its own `transform` applied, and `startOffset` (a percentage relative to the
/// whole path length, or an absolute length) resolved.
fn resolve_text_flow(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
) -> Option<usvg::TextFlow> {
    let href = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))?;
    let linked = defs.get(href.trim_start_matches('#'))?;

    let linked_ty = element_layout_type(linked);
    let linked_computed = linked
        .style_data()
        .is_some()
        .then(|| linked.as_node().style(&context.style_context));
    let mut path = build_shape_path(linked, linked_ty, linked_computed.as_deref())?;

    // The referenced path's own `transform` applies to its outline.
    let transform = linked
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    if !transform.is_identity() {
        path = path.transform(transform)?;
    }
    let path = Arc::new(path);

    let start_offset = match element.attribute_as_str(&ns!(), &LocalName::from("startOffset")) {
        Some(value) => match value.trim().parse::<svgtypes::Length>() {
            // 'If a percentage is given, then the `startOffset` represents a
            // percentage distance along the entire path.'
            Ok(length) if length.unit == LengthUnit::Percent => {
                usvg::path_length(&path) * (length.number as f32 / 100.0)
            },
            Ok(length) => length.number as f32,
            Err(_) => 0.0,
        },
        None => 0.0,
    };

    let id = usvg::NonEmptyString::new(element_id(linked)?)?;
    Some(usvg::TextFlow::Path(Arc::new(usvg::TextPath {
        id,
        start_offset,
        path,
    })))
}

/// Builds a [`usvg::Text`] node from a `<text>` element, laying it out into glyph
/// outlines via usvg's own text engine.
fn convert_text(
    element: &ServoLayoutElement<'_>,
    computed: Option<&ComputedValues>,
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    parent_abs_transform: usvg::Transform,
    fonts: &SvgFonts,
) -> Vec<usvg::Node> {
    let Some(computed) = computed else {
        return Vec::new();
    };

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let node = element.as_node();
    let texts = trim_text_tree(node);
    let pos_list = resolve_positions_list(node, &texts);
    let rotate_list = resolve_rotate_list(node, &texts);
    let writing_mode = convert_writing_mode(element);
    let direction = convert_direction(computed);
    let chunks = collect_text_chunks(element, &pos_list, context, gradients, defs, diagonal, &texts);
    if chunks.is_empty() {
        return Vec::new();
    }

    let rendering_mode = element
        .attribute_as_str(&ns!(), &LocalName::from("text-rendering"))
        .and_then(|s| s.parse::<usvg::TextRendering>().ok())
        .unwrap_or_default();

    let mut text = usvg::Text::new(element_id(element).unwrap_or_default(), abs_transform);
    text.rendering_mode = rendering_mode;
    text.dx = pos_list.iter().map(|p| p.dx.unwrap_or(0.0)).collect();
    text.dy = pos_list.iter().map(|p| p.dy.unwrap_or(0.0)).collect();
    text.rotate = rotate_list;
    text.writing_mode = writing_mode;
    text.direction = direction;
    text.chunks = chunks;

    if usvg::layout(&mut text, &fonts.resolver, &mut fonts.cache.borrow_mut()).is_none() {
        return Vec::new();
    }

    let object_bbox = text.bounding_box().to_non_zero_rect();

    let node = usvg::Node::Text(Box::new(text));

    // Like shapes, a `<text>` element's local `transform`/`opacity`/`clip-path`/
    // `mask` are carried by a wrapper group (usvg::Text has no such fields).
    let element_opacity = computed.get_effects().opacity;
    let clip_path =
        match resolve_clip_path(element, context, gradients, defs, diagonal, fonts, object_bbox) {
            ClipPathOutcome::Clip(clip) => Some(clip),
            ClipPathOutcome::Invalid => return Vec::new(),
            ClipPathOutcome::None => None,
        };
    let mask = match resolve_mask(element, context, gradients, defs, diagonal, fonts, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    if !transform.is_identity() || element_opacity < 1.0 || clip_path.is_some() || mask.is_some() {
        let mut group = usvg::Group::empty();
        group.transform = transform;
        group.abs_transform = abs_transform;
        group.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        group.clip_path = clip_path;
        group.mask = mask;
        group.push_child(node);
        vec![usvg::Node::Group(Box::new(group))]
    } else {
        vec![node]
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
