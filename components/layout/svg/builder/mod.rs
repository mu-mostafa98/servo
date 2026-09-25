/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — assembles an [`SvgTree`] from DOM nodes.
//!
//! Uses the **Builder pattern**: [`SvgTreeBuilder`] accumulates state
//! (CSS rules, definition maps) through chained methods, then produces the
//! final tree via [`build`](SvgTreeBuilder::build).
//!
//! This module is split into three layers:
//! - [`mod`] — orchestration: the builder struct, tag dispatch, and image
//!   assembly.
//! - [`resolve`] — child/`<use>` resolution and reference resolution.
//! - [`text`] — `<text>`/`<tspan>` node assembly and font shaping.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::document::*;
use svg_engine::element::*;
use svg_engine::style::NodeStyle;
use svg_engine::style::gradient::GradientDef;
use svg_engine::units::Id;
use svg_engine::resource::ResourceKey;
use web_atoms::ns;

use crate::context::LayoutContext;
use crate::svg::defines::{
    ClipPathParser, DefinitionCollector, FilterParser, GradientParser, MarkerParser, MaskParser,
    PatternParser, resolve_gradient_hrefs,
};
use crate::svg::primitives::attrs::get_attr;
use crate::svg::primitives::css::collect_svg_css_rules;
use crate::svg::primitives::geometry::build_shape;
use crate::svg::primitives::viewport::{
    extract_nested_viewport, extract_viewport_info, parse_aspect_ratio,
};
use crate::svg::style::build_style;

mod resolve;
mod text;

// ======================= Builder =======================

/// Builds an [`SvgTree`] from a DOM SVG element.
pub(crate) struct SvgTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    css_rules: HashMap<String, HashMap<String, String>>,
    /// Document-wide `id → DOM node` map, built once so `<use href="#id">`
    /// references resolve in O(1) instead of re-walking the document.
    element_ids: HashMap<String, ServoLayoutNode<'dom>>,
}

impl<'dom, 'a> SvgTreeBuilder<'dom, 'a> {
    /// Start building from an SVG DOM element node.
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        let css_rules = collect_svg_css_rules(node);
        let element_ids = build_element_id_map(node);
        SvgTreeBuilder {
            root_node: node,
            context,
            css_rules,
            element_ids,
        }
    }

    /// Build the complete [`SvgTree`].
    pub(crate) fn build(self) -> Option<Arc<SvgTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new(), None)?;
        let viewport = extract_viewport_info(self.root_node);
        let definitions = collect_definitions(self.root_node, &self);

        let mut tree = SvgTree {
            root,
            viewport,
            gradients: definitions.gradients,
            clip_paths: definitions.clip_paths,
            patterns: definitions.patterns,
            masks: definitions.masks,
            filters: definitions.filters,
            markers: definitions.markers,
        };

        // Resolve transient `PaintServer::Ref { id, .. }` / `DefRef::Ref(id)`
        // values into typed `Arc` handles now that the definition maps are collected.
        resolve::resolve_references(&mut tree);

        Some(Arc::new(tree))
    }

    /// Recursively build a render node from a DOM node.
    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        resolving: &mut HashSet<String>,
        inherited: Option<&NodeStyle>,
    ) -> Option<SvgNode> {
        let element = node.as_element()?;
        let tag_name = element.local_name().as_ref().to_owned();

        // Text / tspan — extract text content from DOM children.
        if tag_name == "text" || tag_name == "tspan" {
            return text::build_text_node(node, self.context, &self.css_rules);
        }

        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));
        let tag = build_tag(&element, computed.as_ref().map(|v| &**v), node, self.context)?;
        let (style, transforms) = build_style(node, self.context, &self.css_rules, inherited);
        let id = extract_id(&element);
        let children = resolve::resolve_children(
            node,
            &tag,
            root_node,
            self,
            resolving,
            &style,
            inherited.is_some(),
        );

        // A nested `<svg>` (any `<svg>` except the root) establishes its own
        // viewport. The root's viewport is handled via `SvgTree::viewport`.
        let viewport = if tag_name == "svg" && node != root_node {
            extract_nested_viewport(node)
        } else {
            None
        };

        Some(SvgNode {
            id,
            tag,
            style,
            transforms,
            viewport,
            children,
        })
    }

    /// Build a single definition-content node, used by the definition parsers
    /// to build clip-path / pattern / mask / marker children into full render
    /// nodes (recursively handling `<g>`, `<use>`, `<text>`, nested `<defs>`)
    /// instead of flattening them to a flat list of shapes.
    pub(crate) fn build_def_content(
        &self,
        node: ServoLayoutNode<'dom>,
    ) -> Option<SvgNode> {
        self.build_render_node(node, self.root_node, &mut HashSet::new(), None)
    }
}

// ======================= Definitions =======================

/// Collected definition maps from `<defs>`.
struct DefinitionMaps {
    gradients: HashMap<String, Arc<GradientDef>>,
    clip_paths: HashMap<String, Arc<ClipPathDef>>,
    patterns: HashMap<String, Arc<PatternDef>>,
    masks: HashMap<String, Arc<MaskDef>>,
    filters: HashMap<String, Arc<FilterDef>>,
    markers: HashMap<String, Arc<MarkerDef>>,
}

/// Collect all definition types (gradients, clip-paths, patterns, masks,
/// filters, markers) from `<defs>` containers in the SVG subtree.
fn collect_definitions<'dom, 'a>(
    node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, 'a>,
) -> DefinitionMaps {
    let mut gradients = DefinitionCollector::collect::<GradientParser>(node, builder);
    resolve_gradient_hrefs(&mut gradients);
    DefinitionMaps {
        gradients,
        clip_paths: DefinitionCollector::collect::<ClipPathParser>(node, builder),
        patterns: DefinitionCollector::collect::<PatternParser>(node, builder),
        masks: DefinitionCollector::collect::<MaskParser>(node, builder),
        filters: DefinitionCollector::collect::<FilterParser>(node, builder),
        markers: DefinitionCollector::collect::<MarkerParser>(node, builder),
    }
}

// ======================= Tag Dispatch =======================

/// Map a DOM element's tag name to an [`SvgTag`].
fn build_tag<'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&style::properties::ComputedValues>,
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "image" => build_image_tag(element, node, context).map(SvgTag::Image),
        _ => build_shape(element, tag, computed).map(SvgTag::Shape),
    }
}

/// Build an [`SvgImage`] from element attributes.
///
/// Resolves the `href`/`xlink:href` attribute to a WebRender [`ImageKey`] via
/// the layout image cache: the URL is resolved against the owner document's
/// base URL, then looked up (or requested) through `image_resolver`. When the
/// image is not yet loaded the key is `None` and the renderer draws a
/// placeholder; once it loads, a reflow re-runs this and yields `Some(key)`.
fn build_image_tag(
    element: &ServoLayoutElement,
    node: ServoLayoutNode,
    context: &LayoutContext,
) -> Option<SvgImage> {
    use layout_api::LayoutNode;
    use net_traits::request::InternalRequest;
    use net_traits::image_cache::Image;
    use layout_api::LayoutImageDestination;
    use crate::svg::primitives::attrs::parse_length;
    let fs = 16.0;
    let get = |name: &str| get_attr(element, name);
    let x = parse_length("x", &get, fs).unwrap_or(0.0);
    let y = parse_length("y", &get, fs).unwrap_or(0.0);
    let w = parse_length("width", &get, fs).unwrap_or(0.0).max(0.0);
    let h = parse_length("height", &get, fs).unwrap_or(0.0).max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let get_xlink = |name: &str| {
        element.attribute_as_str(&ns!(xlink), &LocalName::from(name)).map(|s| s.to_string())
    };
    let href = get("href").or_else(|| get_xlink("href"));
    // Resolve href → ImageKey + natural dimensions. Relative URLs are resolved
    // against the owner document's base URL; data: URIs parse directly.  A
    // None/empty href, a pending load, or a decode failure all yield
    // `image_key = None`, in which case the renderer falls back to a
    // placeholder.
    let raster_data: Option<(Option<webrender_api::ImageKey>, u32, u32)> =
        href.as_deref().and_then(|href_str| {
            let base = node.base_url();
            let resolved = base.join(href_str.trim()).ok()?;
            context
                .image_resolver
                .get_cached_image_for_url(
                    node.opaque(),
                    resolved,
                    LayoutImageDestination::BoxTreeConstruction,
                    InternalRequest::No,
                )
                .ok()
                .and_then(|image| match image {
                    Image::Raster(raster) => {
                        Some((raster.id, raster.metadata.width, raster.metadata.height))
                    },
                    Image::Vector(..) => None, // vector images need rasterization; not handled here
                })
        });
    let (image_key, natural_width, natural_height) = match raster_data {
        Some((id, w, h)) => (id.map(image_key_to_resource), Some(w), Some(h)),
        None => (None, None, None),
    };

    // Parse preserveAspectRatio — defaults to xMidYMid meet per SVG spec.
    let preserve_aspect_ratio = get("preserveAspectRatio")
        .map(|v| parse_aspect_ratio(&v))
        .unwrap_or_default();

    Some(SvgImage {
        x,
        y,
        width: w,
        height: h,
        href,
        image_key,
        natural_width,
        natural_height,
        preserve_aspect_ratio,
    })
}

// ======================= Helpers =======================

/// Extract the `id` attribute from an SVG DOM element.
fn extract_id(element: &ServoLayoutElement) -> Option<Id> {
    element
        .attribute_as_str(&ns!(), &local_name!("id"))
        .map(|s| Id::new(s))
}

/// Convert a WebRender font-instance key into the opaque model resource key.
fn font_key_to_resource(key: webrender_api::FontInstanceKey) -> ResourceKey {
    ResourceKey {
        namespace: key.0.0,
        id: key.1,
    }
}

/// Convert a WebRender image key into the opaque model resource key.
fn image_key_to_resource(key: webrender_api::ImageKey) -> ResourceKey {
    ResourceKey {
        namespace: key.0.0,
        id: key.1,
    }
}

/// Build a document-wide `id → DOM node` map once, so `<use href="#id">`
/// references resolve in O(1) instead of re-walking the whole document per
/// `<use>`. References resolve against the whole document (not just the current
/// `<svg>` subtree), so we index from the document root.
fn build_element_id_map<'dom>(
    node: ServoLayoutNode<'dom>,
) -> HashMap<String, ServoLayoutNode<'dom>> {
    let mut map = HashMap::new();
    collect_ids(document_root(node), &mut map);
    map
}

/// Recursively collect `id → node` entries into `map`; first occurrence wins.
fn collect_ids<'dom>(
    node: ServoLayoutNode<'dom>,
    map: &mut HashMap<String, ServoLayoutNode<'dom>>,
) {
    if let Some(element) = node.as_element() {
        if let Some(id) = element.attribute_as_str(&ns!(), &local_name!("id")) {
            map.entry(id.to_owned()).or_insert(node);
        }
    }
    for child in node.dom_children() {
        collect_ids(child, map);
    }
}

/// Walk up to the topmost DOM ancestor (the document node).
///
/// # Safety
///
/// Called during box tree construction, which runs on the main thread. The
/// parent walk is only `unsafe` because accessing ancestors while layout worker
/// threads are running is forbidden.
#[expect(unsafe_code)]
fn document_root<'dom>(node: ServoLayoutNode<'dom>) -> ServoLayoutNode<'dom> {
    let mut root = node;
    while let Some(parent) = unsafe { root.dangerous_dom_parent() } {
        root = parent;
    }
    root
}
