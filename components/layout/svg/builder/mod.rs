/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — assembles an [`SvgTree`] from DOM nodes.
//!
//! Uses the **Builder pattern**: [`SvgTreeBuilder`] accumulates state
//! (CSS rules, definition maps) through chained methods, then produces the
//! final tree via [`build`](SvgTreeBuilder::build).
//!
//! This module is split into four layers:
//! - [`mod`] — orchestration: the builder struct and tag dispatch.
//! - [`resolve`] — child/`<use>`/`<switch>` resolution.
//! - [`text`] — `<text>`/`<tspan>` node assembly and font shaping.
//! - [`image`] — `<image>` element assembly and image-key resolution.
//!
//! References (`url(#id)`) are left as transient `PaintServer::Ref` /
//! `DefRef::Ref` values in the built tree and resolved at render time (see
//! `servo_svg::document::Defs`), so the tree is immutable after build.

use std::collections::HashMap;
use std::sync::Arc;

use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use log::debug;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_arc::Arc as ServoArc;
use servo_svg::document::*;
use servo_svg::element::*;
use servo_svg::resource::ResourceKey;
use servo_svg::style::NodeStyle;
use servo_svg::units::Id;
use svgtypes::Color as SvgColor;
use web_atoms::ns;

use crate::context::LayoutContext;
use crate::svg::CacheMiss;
use crate::svg::CrossRefFingerprint;
use crate::svg::defines::{
    ClipPathParser, DefinitionCollector, FilterParser, GradientParser, MarkerParser, MaskParser,
    PatternParser, resolve_gradient_hrefs,
};
use crate::svg::primitives::attrs::{conditional_processing_passes, is_unknown_svg_element};
use crate::svg::primitives::css::collect_svg_css_rules;
use crate::svg::primitives::geometry::build_shape;
use crate::svg::primitives::viewport::{
    extract_nested_viewport, extract_viewport_info, viewport_reference,
};
use crate::svg::style::build_style;

mod image;
mod resolve;
mod text;

// ======================= Cross-references =======================

/// A built subtree's cross-reference state, threaded up from the leaves so the
/// cache can decide whether (and how) a subtree is reusable.
///
/// - `None` — no cross-reference; the subtree is cached normally.
/// - `Simple` — a single `<use>` whose referenced target is itself free of
///   cross-references; the target's version + style (`CrossRefFingerprint`)
///   fingerprint it, so the `<use>` can be cached.
/// - `Complex` — a `<textPath>`, or a `<use>` whose target itself contains a
///   cross-reference (and any ancestor of one): the referenced content can
///   change without bumping this subtree's version, so it is never cached.
pub(crate) enum CrossRef {
    None,
    Simple(CrossRefFingerprint),
    Complex,
}

// ======================= Builder =======================

/// Builds an [`SvgTree`] from a DOM SVG element.
pub(crate) struct SvgTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    css_rules: HashMap<String, HashMap<String, String>>,
    /// Document-wide `id → DOM node` map, built once so `<use href="#id">`
    /// references resolve in O(1) instead of re-walking the document.
    element_ids: HashMap<String, ServoLayoutNode<'dom>>,
    /// The root viewport (resolved `width`/`height` + viewBox + aspect ratio),
    /// computed once in [`new`](SvgTreeBuilder::new) and reused by [`build`](SvgTreeBuilder::build).
    root_viewport: ViewportInfo,
    /// Root viewport percentage-resolution reference dimensions (viewBox extent
    /// when present, else the viewport `width`/`height` attributes).
    root_vw: f32,
    root_vh: f32,
}

impl<'dom, 'a> SvgTreeBuilder<'dom, 'a> {
    /// Start building from an SVG DOM element node.
    ///
    /// `root_width`/`root_height` are the resolved viewport dimensions (user
    /// units) supplied by CSS layout; they feed the root [`ViewportInfo`] and,
    /// when no viewBox is present, the percentage-resolution reference.
    pub(crate) fn new(
        node: ServoLayoutNode<'dom>,
        context: &'a LayoutContext<'a>,
        root_width: f32,
        root_height: f32,
    ) -> Self {
        let css_rules = collect_svg_css_rules(node);
        let element_ids = build_element_id_map(node);
        let root_viewport = extract_viewport_info(node, root_width, root_height);
        let (root_vw, root_vh) = viewport_reference(&root_viewport);
        // If the class-based CSS rules changed since the last reflow, every
        // cached subtree under this root is out of date; clear it before any
        // lookup so this build starts from a consistent view.
        context
            .image_resolver
            .svg_subtree_cache
            .write()
            .ensure_css_rules(node.opaque(), &css_rules);
        SvgTreeBuilder {
            root_node: node,
            context,
            css_rules,
            element_ids,
            root_viewport,
            root_vw,
            root_vh,
        }
    }

    /// Build the complete [`SvgTree`].
    pub(crate) fn build(self) -> Option<Arc<SvgTree>> {
        let (root, _) = self.build_render_node(
            self.root_node,
            self.root_node,
            &mut resolve::ResolveState::default(),
            None,
            None,
            self.root_vw,
            self.root_vh,
        )?;
        // The root `<svg>` is never cached, so its freshly built subtree is
        // still uniquely owned here.
        let root = Arc::try_unwrap(root).expect("the root `<svg>` is never cached");
        let viewport = self.root_viewport.clone();
        let definitions = collect_definitions(self.root_node, &self);

        let tree = SvgTree {
            root,
            viewport,
            gradients: definitions.gradients,
            clip_paths: definitions.clip_paths,
            patterns: definitions.patterns,
            masks: definitions.masks,
            filters: definitions.filters,
            markers: definitions.markers,
        };

        // References (`PaintServer::Ref` / `DefRef::Ref`) are left unresolved
        // here and bound to definitions at render time (see
        // `servo_svg::document::Defs`), so the built tree is immutable.
        Some(Arc::new(tree))
    }

    /// Recursively build a render node from a DOM node.
    ///
    /// Returns the built subtree and its cross-reference state (see
    /// [`CrossRef`]). A `Simple` or `Complex` subtree still makes its ancestors
    /// uncacheable — the referenced target can change without bumping the
    /// ancestor's version — but only `Simple` (a single `<use>` whose target is
    /// itself cross-reference-free) is cacheable itself.
    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        state: &mut resolve::ResolveState,
        inherited: Option<&NodeStyle>,
        inherited_color: Option<SvgColor>,
        vw: f32,
        vh: f32,
    ) -> Option<(Arc<SvgNode>, CrossRef)> {
        // Global expansion budget: cap the total work of a single build so that
        // pathological `<use>` graphs — exponential "billion laughs" fan-out,
        // quadratic blow-up, command amplification — terminate (§5.6; README
        // §9.2, issues #4–#6). Each invocation consumes one unit of budget; the
        // cap is generous enough that legitimate documents are unaffected.
        if state.nodes >= resolve::MAX_TOTAL_NODES {
            return None;
        }
        state.nodes += 1;

        // Text/whitespace/comment children produce no render node (and are never
        // cached), so return before the cache lookup — otherwise every whitespace
        // node between elements emits a noisy `build <node> [NoEntry]` line.
        if node.as_element().is_none() {
            return None;
        }

        // Only clean, non-root, non-shadow subtrees are cacheable. The root is
        // excluded because it is returned by value as `SvgTree::root`; shadow
        // subtrees (`inherited = Some`) are excluded because they carry a
        // per-`<use>` inherited style and are never shared between uses.
        let is_root = node == root_node;
        let cacheable = inherited.is_none() && !is_root;

        // The computed style is part of the cache key: it catches inherited-style
        // changes from an ancestor (or an external stylesheet) that a version bump
        // alone would miss. Computed once here and reused for both lookup and store.
        let computed_style = self.computed_style(node);

        // A `<use>`'s referenced target is not covered by this node's own
        // version, so extend the cache key with the target's version + computed
        // style. This lets a `<use>` be reused when neither it nor its target
        // changed, instead of being rebuilt unconditionally. A target that
        // itself contains a cross-reference is detected during the build and
        // left uncached.
        let lookup_cross_ref = self.lookup_cross_ref_dep(node);

        if cacheable {
            match self.lookup_cached(
                node,
                inherited_color,
                vw,
                vh,
                computed_style.as_ref(),
                lookup_cross_ref.as_ref(),
            ) {
                Ok(hit) => {
                    debug!(target: "svg", "reuse {}", describe(node));
                    // A cached `<use>` is still a cross-reference for its
                    // ancestors — its target can change without bumping their
                    // versions — so report `Complex` to the caller even though
                    // this entry itself was reused.
                    let cross_ref = if lookup_cross_ref.is_some() {
                        CrossRef::Complex
                    } else {
                        CrossRef::None
                    };
                    return Some((hit, cross_ref));
                },
                Err(reason) => {
                    debug!(target: "svg", "build {} [{reason:?}]", describe(node));
                },
            }
        }

        let (built, cross_ref) =
            self.build_node(node, root_node, state, inherited, inherited_color, vw, vh)?;

        let subtree = Arc::new(built);
        if cacheable {
            match &cross_ref {
                CrossRef::None => {
                    self.store_cached(
                        node,
                        inherited_color,
                        vw,
                        vh,
                        computed_style,
                        None,
                        Arc::clone(&subtree),
                    );
                },
                CrossRef::Simple(dep) => {
                    self.store_cached(
                        node,
                        inherited_color,
                        vw,
                        vh,
                        computed_style,
                        Some((*dep).clone()),
                        Arc::clone(&subtree),
                    );
                },
                // A cross-reference we can't fingerprint — never cached.
                CrossRef::Complex => {},
            }
        }
        Some((subtree, cross_ref))
    }

    /// Build a render node from a DOM node (no caching). This is the shared
    /// body of the build: tag dispatch, style construction, and child
    /// resolution. See [`SvgTreeBuilder::build_render_node`] for the cache
    /// wrapper around it.
    fn build_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        state: &mut resolve::ResolveState,
        inherited: Option<&NodeStyle>,
        inherited_color: Option<SvgColor>,
        vw: f32,
        vh: f32,
    ) -> Option<(SvgNode, CrossRef)> {
        let element = node.as_element()?;

        // §5.7 conditional processing: an element whose `requiredExtensions`,
        // `systemLanguage`, or `requiredFeatures` test fails is not rendered
        // anywhere — including as a child of a `<switch>` or inside a definition
        // container.
        if !conditional_processing_passes(&element) {
            return None;
        }

        let tag_name = element.local_name().to_string();

        // Text / tspan — extract text content from DOM children.
        if tag_name == "text" || tag_name == "tspan" {
            return text::build_text_node(node, self.context, &self.css_rules, &self.element_ids);
        }

        // A nested `<svg>` (any `<svg>` except the root) establishes its own
        // viewport; its own geometry and its children resolve percentages
        // against that viewport. The root's viewport is handled via
        // `SvgTree::viewport`.
        let viewport = if tag_name == "svg" && node != root_node {
            extract_nested_viewport(node, vw, vh)
        } else {
            None
        };
        let (vw, vh) = match viewport.as_ref() {
            Some(vp) => viewport_reference(&vp.viewport),
            None => (vw, vh),
        };

        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));
        let tag = build_tag(
            &element,
            computed.as_ref().map(|v| &**v),
            node,
            self.context,
            vw,
            vh,
        )?;
        let (style, transforms, current_color) = build_style(
            node,
            self.context,
            &self.css_rules,
            inherited,
            inherited_color,
        );
        let id = extract_id(&element);
        let (children, cross_ref) = resolve::resolve_children(
            node,
            &tag,
            root_node,
            self,
            state,
            &style,
            &current_color,
            inherited.is_some(),
            vw,
            vh,
        );

        Some((
            SvgNode {
                id,
                tag,
                style,
                transforms,
                viewport,
                children,
            },
            cross_ref,
        ))
    }

    /// The computed style of `node`, used as the style-change half of the cache
    /// key. `None` for nodes that have no style data (unstyled nodes), which the
    /// cache treats as equal only to another `None`.
    fn computed_style(
        &self,
        node: ServoLayoutNode<'dom>,
    ) -> Option<ServoArc<style::properties::ComputedValues>> {
        let element = node.as_element()?;
        if element.style_data().is_none() {
            return None;
        }
        Some(node.style(&self.context.style_context))
    }

    /// Look up a cached subtree for `node`, if one is still fresh.
    fn lookup_cached(
        &self,
        node: ServoLayoutNode<'dom>,
        inherited_color: Option<SvgColor>,
        vw: f32,
        vh: f32,
        computed_style: Option<&ServoArc<style::properties::ComputedValues>>,
        cross_ref: Option<&CrossRefFingerprint>,
    ) -> Result<Arc<SvgNode>, CacheMiss> {
        self.context
            .image_resolver
            .svg_subtree_cache
            .read()
            .lookup(
                self.root_node.opaque(),
                node.opaque(),
                node.inclusive_descendants_version(),
                vw,
                vh,
                inherited_color,
                computed_style,
                cross_ref,
            )
    }

    /// Store a freshly built subtree for `node`.
    fn store_cached(
        &self,
        node: ServoLayoutNode<'dom>,
        inherited_color: Option<SvgColor>,
        vw: f32,
        vh: f32,
        computed_style: Option<ServoArc<style::properties::ComputedValues>>,
        cross_ref: Option<CrossRefFingerprint>,
        subtree: Arc<SvgNode>,
    ) {
        self.context
            .image_resolver
            .svg_subtree_cache
            .write()
            .store(
                self.root_node.opaque(),
                node.opaque(),
                node.inclusive_descendants_version(),
                vw,
                vh,
                inherited_color,
                computed_style,
                cross_ref,
                subtree,
            );
    }

    /// Resolve a `<use>` element's `href`/`xlink:href` reference to the
    /// referenced DOM node. Returns `None` for a non-`<use>` element or an
    /// unresolvable reference.
    fn resolve_use_target(&self, node: ServoLayoutNode<'dom>) -> Option<ServoLayoutNode<'dom>> {
        let element = node.as_element()?;
        let ref_id = element
            .attribute_as_str(&ns!(), &local_name!("href"))
            .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
            .and_then(|h| {
                let t = h.trim_start_matches('#');
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_owned())
                }
            })?;
        self.element_ids.get(&ref_id).copied()
    }

    /// The cross-reference fingerprint for a `<use>` node: its referenced
    /// target's `inclusive_descendants_version` and computed style, read without
    /// building the target. `None` for a non-`<use>` node or an unresolvable
    /// reference.
    ///
    /// This is the lookup-time half of the `<use>` cache key. It *assumes* the
    /// target is free of cross-references; a non-simple target is never stored
    /// (see [`CrossRef`]), and a target that became non-simple in the interim
    /// bumped its version (adding/removing a child is a mutation), so the
    /// version check still forces a rebuild.
    fn lookup_cross_ref_dep(&self, node: ServoLayoutNode<'dom>) -> Option<CrossRefFingerprint> {
        let element = node.as_element()?;
        if element.local_name() != &local_name!("use") {
            return None;
        }
        let target = self.resolve_use_target(node)?;
        Some(CrossRefFingerprint {
            version: target.inclusive_descendants_version(),
            style: self.computed_style(target),
        })
    }

    /// Build a single definition-content node, used by the definition parsers
    /// to build clip-path / pattern / mask / marker children into full render
    /// nodes (recursively handling `<g>`, `<use>`, `<text>`, nested `<defs>`)
    /// instead of flattening them to a flat list of shapes.
    pub(crate) fn build_def_content(&self, node: ServoLayoutNode<'dom>) -> Option<Arc<SvgNode>> {
        self.build_render_node(
            node,
            self.root_node,
            &mut resolve::ResolveState::default(),
            None,
            None,
            self.root_vw,
            self.root_vh,
        )
        .map(|(subtree, _)| subtree)
    }

    /// The computed CSS `color` of an element, resolved to an [`SvgColor`].
    ///
    /// Used to resolve the `currentColor` keyword where the element's own
    /// computed color is needed but `build_style` is not run — notably the
    /// gradient `<stop>` parser, whose `stop-color="currentColor"` resolves
    /// against the `<stop>` element's inherited `color`. Falls back to opaque
    /// black when the element has no computed style.
    pub(crate) fn computed_color(&self, node: ServoLayoutNode<'dom>) -> SvgColor {
        let Some(element) = node.as_element() else {
            return SvgColor::black();
        };
        if element.style_data().is_none() {
            return SvgColor::black();
        }
        let computed = node.style(&self.context.style_context);
        crate::svg::style::absolute_to_svg_color(&computed.clone_color())
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
    vw: f32,
    vh: f32,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "switch" => Some(SvgTag::Container(Container::Switch)),
        "image" => image::build_image_tag(element, node, context, vw, vh).map(SvgTag::Image),
        _ => match build_shape(element, tag, computed, vw, vh) {
            Some(shape) => Some(SvgTag::Shape(shape)),
            // §5.3 unknown elements: an SVG-namespace element that is not a
            // known renderable element is treated as a `<g>` (children render,
            // styles inherit). Non-SVG-namespace elements and known
            // non-rendering SVG elements are skipped.
            None if is_unknown_svg_element(element, tag) => {
                Some(SvgTag::Container(Container::Group))
            },
            None => None,
        },
    }
}

// ======================= Helpers =======================

/// A short human-readable label for a DOM node — `<tag>` or `<tag#id>` — used in
/// cache hit/miss log lines so a rebuild's cause is attributable to a specific
/// element.
fn describe<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let Some(element) = node.as_element() else {
        return "<node>".to_owned();
    };
    let tag = element.local_name().to_string();
    match element.attribute_as_str(&ns!(), &local_name!("id")) {
        Some(id) => format!("<{tag}#{id}>"),
        None => format!("<{tag}>"),
    }
}

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

/// Build a document-wide `id → DOM node` map once, so `<use href="#id">`
/// references resolve in O(1) instead of re-walking the whole document per
/// `<use>`. References resolve against the whole document (not just the current
/// `<svg>` subtree), so we index from the document root.
fn build_element_id_map<'dom>(
    node: ServoLayoutNode<'dom>,
) -> HashMap<String, ServoLayoutNode<'dom>> {
    let mut map = HashMap::new();
    collect_ids(document_root_node(node), &mut map);
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
fn document_root_node<'dom>(node: ServoLayoutNode<'dom>) -> ServoLayoutNode<'dom> {
    let mut root = node;
    while let Some(parent) = unsafe { root.dangerous_dom_parent() } {
        root = parent;
    }
    root
}
