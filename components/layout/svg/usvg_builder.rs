/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Thin per-element builders that assemble a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! Each `build_*` function reads the small bit of per-element state it needs and
//! delegates the parsing/math/placement details to [`crate::svg::primitives`] and
//! [`crate::svg::effects`], then constructs the resulting [`usvg::Node`](s). The
//! heavy lifting — geometry, paint servers, clip/mask/filter, marker placement,
//! image decode, text layout — lives in those lower layers.

use std::collections::HashMap;
use std::sync::Arc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_arc::Arc as ServoArc;
use style::dom::TNode;
use style::properties::ComputedValues;
use style::values::computed::Length;
use style::values::generics::svg::SVGLength;

use crate::context::LayoutContext;
use crate::svg::effects::clip::rect_clip_path;
use crate::svg::effects::marker::build_markers;
use crate::svg::effects::paint::{
    Gradients, build_fill, build_pattern, build_stroke, collect_paint_servers,
};
use crate::svg::effects::{Effects, resolve_effects};
use crate::svg::primitives::attrs::{
    element_has_explicit_fill, element_has_explicit_stroke, element_id, element_layout_type,
    is_group_element, length_attr_opt, parse_transform, parse_view_box, resolve_size_and_view_box,
};
use crate::svg::primitives::geometry::normalized_diagonal;
use crate::svg::primitives::image::{decode_image, image_geometry};
use crate::svg::primitives::shape::resolve_shape_path;
use crate::svg::primitives::text::{
    SvgFonts, collect_text_chunks, convert_direction, convert_writing_mode, resolve_positions_list,
    resolve_rotate_list, trim_text_tree,
};

/// The shared state every builder needs to build its [`usvg::Node`].
///
/// `'a` borrows the surrounding build scope (`LayoutContext`, the collected
/// paint servers, the id→element def map and the font resolver); `'dom` is the
/// DOM lifetime of the elements in `defs`.
pub(crate) struct SvgContext<'a, 'dom> {
    pub(crate) context: &'a LayoutContext<'a>,
    pub(crate) gradients: &'a Gradients,
    pub(crate) defs: &'a HashMap<String, ServoLayoutElement<'dom>>,
    pub(crate) diagonal: f32,
    pub(crate) fonts: &'a SvgFonts,
}

impl SvgContext<'_, '_> {
    /// The computed style of `element`, or `None` when the element is unstyled.
    pub(crate) fn computed_style(
        &self,
        element: &ServoLayoutElement<'_>,
    ) -> Option<ServoArc<ComputedValues>> {
        element
            .style_data()
            .is_some()
            .then(|| element.as_node().style(&self.context.style_context))
    }

    /// The element's `transform` attribute as a [`usvg::Transform`], or identity
    /// when absent/unparseable.
    pub(crate) fn transform_attr(&self, element: &ServoLayoutElement<'_>) -> usvg::Transform {
        element
            .attribute_as_str(&ns!(), &LocalName::from("transform"))
            .map(parse_transform)
            .unwrap_or_else(usvg::Transform::identity)
    }

    /// The element's group opacity, defaulting to fully opaque when `computed` is
    /// `None` (unstyled) or the computed opacity is not a valid normalised value.
    pub(crate) fn opacity(&self, computed: Option<&ComputedValues>) -> usvg::Opacity {
        computed
            .and_then(|c| usvg::Opacity::new(c.get_effects().opacity))
            .unwrap_or(usvg::Opacity::ONE)
    }
}

/// Builds a [`usvg::Tree`] from the `<svg>` element at `node`.
///
/// Returns the tree together with the parsed [`usvg::ViewBox`] (if any). The
/// viewBox transform is deliberately *not* baked into the tree: content stays in
/// viewBox coordinates, and the viewBox→device mapping is applied at raster time
/// (see [`crate::svg::rasterize_svg_tree`]) so `preserveAspectRatio` is honoured
/// uniformly rather than being distorted by a non-uniform CSS-box stretch.
///
/// Returns `None` when `node` is not an `<svg>` element or the tree would be
/// empty/invalid.
#[expect(unsafe_code)]
pub(crate) fn build_usvg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
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

    // Build each pattern and insert it immediately, so that a pattern whose
    // content references a *previously* built pattern (nested `<pattern>`) can
    // resolve it. `ctx` is scoped per element so its `&gradients` borrow is
    // dropped before the insertion mutates `gradients`.
    for element in &pattern_elements {
        if let Some(id) = element_id(element) {
            let pattern = {
                let ctx = SvgContext {
                    context,
                    gradients: &gradients,
                    defs: &defs,
                    diagonal,
                    fonts: &fonts,
                };
                build_pattern(element, &ctx)
            };
            if let Some(pattern) = pattern {
                gradients.pattern.insert(id, Arc::new(pattern));
            }
        }
    }

    let ctx = SvgContext {
        context,
        gradients: &gradients,
        defs: &defs,
        diagonal,
        fonts: &fonts,
    };

    // Children are built in viewBox (user) coordinates; the viewBox transform is
    // applied later, at raster time.
    let mut root = usvg::Group::empty();
    for child in node.dom_children() {
        for child_node in build_usvg_node(child, &ctx, usvg::Transform::identity(), None) {
            root.push_child(child_node);
        }
    }

    let mut tree = usvg::Tree::new(size, root);
    tree.finalize();
    Some((tree, view_box))
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

/// Converts a DOM node (and its subtree) into a list of [`usvg::Node`]s.
///
/// A shape normally produces a single node, but a shape carrying markers emits the
/// path plus one group per placed marker (markers are siblings of the path in usvg,
/// not a child node). Callers must therefore push every returned node.
///
/// `ctx.defs` is a document-wide `id`→element map used to resolve `<use>` references.
/// `host` is the computed style of the enclosing `<use>` element (if any): it is
/// used as the inheritance parent for paint, since `<use>` shadow content inherits
/// from the `<use>` host rather than from its location in the `<defs>`.
pub(crate) fn build_usvg_node<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Vec<usvg::Node> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let ty = element_layout_type(&element);

    // A nested `<svg>` establishes a new viewport (x/y + viewBox transform), so it
    // needs dedicated handling rather than the plain `<g>` group path.
    if ty == LayoutElementType::SVGSVGElement {
        return build_svg(node, ctx, parent_abs_transform, host)
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
        return build_group(node, ctx, parent_abs_transform, host)
            .into_iter()
            .collect();
    }

    if ty == LayoutElementType::SVGUseElement {
        return build_use(node, ctx, parent_abs_transform)
            .into_iter()
            .collect();
    }

    let computed = ctx.computed_style(&element);

    if ty == LayoutElementType::SVGTextElement {
        return build_text(&element, computed.as_deref(), ctx, parent_abs_transform)
            .into_iter()
            .collect();
    }

    if ty == LayoutElementType::SVGImageElement {
        return build_image(&element, computed.as_deref(), ctx, parent_abs_transform)
            .into_iter()
            .collect();
    }

    build_shape(
        &element,
        ty,
        computed.as_deref(),
        host,
        ctx,
        parent_abs_transform,
    )
}

/// Builds the path geometry + paint + markers for a basic shape element.
///
/// This is the one builder that returns a `Vec` rather than a single node: a shape
/// carrying markers emits the path plus one group per placed marker, and markers
/// are *siblings* of the path in usvg.
fn build_shape<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
    host: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(data) = resolve_shape_path(element, ty, computed) else {
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

    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let object_bbox = data.bounds().to_non_zero_rect();

    let Some(effects) = resolve_effects(element, ctx, object_bbox) else {
        return Vec::new();
    };

    if needs_carry(&transform, element_opacity, &effects) {
        let mut group = usvg::Group::empty();
        carry_group(&mut group, transform, abs_transform, element_opacity, effects);
        for node in nodes {
            group.push_child(node);
        }
        vec![usvg::Node::Group(Box::new(group))]
    } else {
        nodes
    }
}

/// Builds an `<image>` element into an `Image` node (always wrapped in an inner
/// align group, plus an outer effect group when the element carries
/// transform/opacity/clip/mask/filter).
fn build_image<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let (kind, size) = decode_image(element)?;
    let (translate_scale, object_bbox) = image_geometry(element, size)?;

    let element_transform = ctx.transform_attr(element);
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

    let image = usvg::Image::new(
        id,
        visible,
        size,
        usvg::ImageRendering::default(),
        kind,
        abs_transform,
    )?;

    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let effects = resolve_effects(element, ctx, Some(object_bbox))?;

    // The image is always wrapped in an inner group carrying the align transform
    // (x/y/width/height/preserveAspectRatio position+scale). clip-path/mask/filter
    // must NOT live on this group: resvg isolates the group that carries them and
    // applies the clip/mask in that group's shifted *local* coordinate system, so a
    // `userSpaceOnUse` clip authored in the element's user space would be scaled by
    // the align transform and clip the image away. Instead those properties go on an
    // *outer* group whose transform is the element's own `transform` attribute.
    let mut inner = usvg::Group::empty();
    inner.transform = translate_scale;
    inner.abs_transform = abs_transform;
    inner.push_child(usvg::Node::Image(Box::new(image)));

    let inner_node = usvg::Node::Group(Box::new(inner));

    if needs_carry(&element_transform, element_opacity, &effects) {
        let mut outer = usvg::Group::empty();
        carry_group(
            &mut outer,
            element_transform,
            parent_abs_transform.pre_concat(element_transform),
            element_opacity,
            effects,
        );
        outer.push_child(inner_node);
        Some(usvg::Node::Group(Box::new(outer)))
    } else {
        Some(inner_node)
    }
}

/// Builds a `<text>` element into a `Text` node, laying it out into glyph outlines
/// via usvg's own text engine. Resolves clip-path/mask *and* filter (a `<text>`
/// element may carry `filter`).
pub(crate) fn build_text<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let computed = computed?;

    let transform = ctx.transform_attr(element);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let node = element.as_node();
    let texts = trim_text_tree(node);
    let pos_list = resolve_positions_list(node, &texts);
    let rotate_list = resolve_rotate_list(node, &texts);
    let writing_mode = convert_writing_mode(element);
    let direction = convert_direction(computed);
    let chunks = collect_text_chunks(
        element,
        &pos_list,
        ctx.context,
        ctx.gradients,
        ctx.defs,
        ctx.diagonal,
        &texts,
    );
    if chunks.is_empty() {
        return None;
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

    if usvg::layout(
        &mut text,
        &ctx.fonts.resolver,
        &mut ctx.fonts.cache.borrow_mut(),
    )
    .is_none()
    {
        return None;
    }

    let object_bbox = text.bounding_box().to_non_zero_rect();
    let text_node = usvg::Node::Text(Box::new(text));

    let element_opacity = computed.get_effects().opacity;
    let effects = resolve_effects(element, ctx, object_bbox)?;

    if needs_carry(&transform, element_opacity, &effects) {
        let mut group = usvg::Group::empty();
        carry_group(&mut group, transform, abs_transform, element_opacity, effects);
        group.push_child(text_node);
        Some(usvg::Node::Group(Box::new(group)))
    } else {
        Some(text_node)
    }
}

/// Builds a group-like element (`g`/`a`/`clipPath`/`mask`) into a `Group`. Unlike a
/// basic shape, a group's `transform`/`opacity`/`clip`/`mask`/`filter` live *on* the
/// group node itself, so no wrapper group is needed.
fn build_group<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = ctx.computed_style(&element);

    let transform = ctx.transform_attr(&element);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    for child in node.dom_children() {
        for child_node in build_usvg_node(child, ctx, abs_transform, host) {
            group.push_child(child_node);
        }
    }

    // Compute the object bounding box before resolving effects, since clip/mask
    // may use `objectBoundingBox` units that depend on it.
    let object_bbox = group.compute_object_bbox();
    let effects = resolve_effects(&element, ctx, object_bbox)?;

    let opacity = computed.as_deref().map(|c| c.get_effects().opacity).unwrap_or(1.0);
    carry_group(&mut group, transform, abs_transform, opacity, effects);

    Some(usvg::Node::Group(Box::new(group)))
}

/// Builds a nested `<svg>` element, which establishes a new viewport: its `x`/`y`
/// position plus a `viewBox`→viewport transform (`preserveAspectRatio`-aware). When
/// `overflow` is not `visible` (and explicit `width`/`height` form a rectangle), a
/// synthetic clip path limits rendering to the new viewport.
///
/// The structure matches usvg: an outer group carries the `transform` attribute and
/// (optionally) the clip path, and an inner group carries the viewport transform.
fn build_svg<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
    host: Option<&ComputedValues>,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = ctx.computed_style(&element);

    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);

    // The `transform` attribute operates in the parent user space, before the new
    // viewport is established, so it stays on the outer group.
    let orig_ts = ctx.transform_attr(&element);

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

    group.opacity = ctx.opacity(computed.as_deref());

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
        for child_node in build_usvg_node(child, ctx, inner.abs_transform, host) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}

/// Builds a `<use href="#id">` element into a group whose children are the
/// referenced element's nodes, translated by the `<use>`'s `x`/`y`.
fn build_use<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    ctx: &SvgContext<'a, 'dom>,
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

    let referenced = ctx.defs.get(id)?;
    if referenced.as_node().opaque() == node.opaque() {
        return None;
    }

    let computed = ctx.computed_style(&element);

    // A `<use>` referencing a `<symbol>` establishes a new viewport (the symbol's
    // `viewBox` mapped onto the `<use>`'s `width`×`height`), handled separately.
    if element_layout_type(referenced) == LayoutElementType::SVGSymbolElement {
        return build_use_symbol(node, referenced, computed.as_deref(), ctx, parent_abs_transform);
    }

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let mut transform = ctx.transform_attr(&element);
    let x = length_attr_opt(&element, "x").unwrap_or(0.0);
    let y = length_attr_opt(&element, "y").unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        transform = transform.pre_concat(usvg::Transform::from_translate(x, y));
    }
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    group.opacity = ctx.opacity(computed.as_deref());

    let referenced_node = referenced.as_node();
    for child_node in build_usvg_node(referenced_node, ctx, abs_transform, computed.as_deref()) {
        group.push_child(child_node);
    }

    Some(usvg::Node::Group(Box::new(group)))
}

/// Builds a `<use href="#symbol">` reference. Unlike `<use>` of a plain group, a
/// `<symbol>` establishes a new viewport: its `viewBox` is mapped onto the `<use>`'s
/// `x`/`y`/`width`/`height` rectangle. The structure matches [`build_svg`]: an outer
/// group carries the `transform` attribute (plus an optional viewport clip), and an
/// inner group carries the viewport transform.
fn build_use_symbol<'a, 'dom>(
    node: ServoLayoutNode<'dom>,
    symbol: &ServoLayoutElement<'dom>,
    host: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;

    let orig_ts = ctx.transform_attr(&element);

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

    group.opacity = ctx.opacity(host);

    // A symbol establishes a viewport; clip to its rectangle unless `overflow` is
    // `visible` (or `auto`), mirroring the nested-`<svg>` handling in `build_svg`.
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
        for child_node in build_usvg_node(child, ctx, inner.abs_transform, host) {
            inner.push_child(child_node);
        }
    }

    group.push_child(usvg::Node::Group(Box::new(inner)));
    Some(usvg::Node::Group(Box::new(group)))
}

/// Builds one `<marker>` group at the given transform, converting its children in
/// the marker's local coordinate system. Called from
/// [`crate::svg::effects::marker`] for each placed vertex.
pub(crate) fn build_marker<'a, 'dom>(
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

/// Whether an element's transform/opacity/effects require a carrying wrapper group.
fn needs_carry(transform: &usvg::Transform, opacity: f32, effects: &Effects) -> bool {
    !transform.is_identity() ||
        opacity < 1.0 ||
        effects.clip_path.is_some() ||
        effects.mask.is_some() ||
        effects.filter.is_some()
}

/// Writes an element's transform/opacity/clip/mask/filter onto a carrying group.
fn carry_group(
    group: &mut usvg::Group,
    transform: usvg::Transform,
    abs_transform: usvg::Transform,
    opacity: f32,
    effects: Effects,
) {
    group.transform = transform;
    group.abs_transform = abs_transform;
    group.opacity = usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE);
    group.clip_path = effects.clip_path;
    group.mask = effects.mask;
    if let Some(filter) = effects.filter {
        group.filters.push(filter);
    }
}
