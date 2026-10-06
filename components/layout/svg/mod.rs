/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — public API surface.
//!
//! This module bridges Servo's DOM and style system with the SVG engine's
//! render tree types.
//!
//! # Module Map
//!
//! | Module | Role |
//! |--------|------|
//! | [`builder`] | Orchestrator — assembles the render tree (Builder pattern) |
//! | [`defines`] | Definition collection — gradients, clip-paths, etc. (Strategy pattern) |
//! | [`primitives`] | Leaf parsers — attributes, geometry, text, paint, CSS, viewport, transforms |
//! | [`style`] | Style construction — [`ComputedValues`] → [`NodeStyle`] |
//!
//! The main entry point is [`build_svg_tree`], called from
//! [`crate::replaced`].

pub(crate) mod builder;
pub(crate) mod defines;
pub(crate) mod primitives;
pub(crate) mod style;

use std::collections::HashMap;
use std::sync::Arc;

use script::layout_dom::ServoLayoutNode;
use servo_arc::Arc as ServoArc;
use servo_svg::document::SvgTree;
use servo_svg::element::SvgNode;
// `::style` disambiguates the external `style` crate from the sibling
// `svg::style` module declared below.
use ::style::dom::OpaqueNode;
use ::style::properties::ComputedValues;
use svgtypes::Color as SvgColor;

use crate::context::LayoutContext;
use crate::svg::primitives::css::CssClassRules;

/// Persistent cache of clean SVG render subtrees, shared across reflows via
/// [`LayoutContext::image_resolver`].
///
/// This is the Level 2 incremental-build cache: subtrees are keyed by the DOM
/// node that produced them and reused when that node's `inclusive_descendants_version`
/// is unchanged, its computed style is the same allocation, and the build inputs
/// — the viewport reference dimensions, the inherited `currentColor`, and the
/// root's class-based `<style>` rules — all match. The computed-style check
/// (`Arc::ptr_eq`) is what invalidates a subtree when an *ancestor's* inherited
/// property (or an external stylesheet) changes, which a version bump alone
/// would miss. Subtrees that contain a cross-reference (`<use>` or `<textPath>`)
/// are never cached, because a referenced element can change without bumping
/// the referencing node's version.
pub(crate) struct SvgSubtreeCache {
    /// Per-root caches, so each `<svg>` root's class-based CSS rules stay
    /// isolated and can be invalidated independently.
    roots: HashMap<OpaqueNode, SvgRootCache>,
}

impl Default for SvgSubtreeCache {
    fn default() -> Self {
        SvgSubtreeCache {
            roots: HashMap::new(),
        }
    }
}

/// The cache for a single root `<svg>` element.
struct SvgRootCache {
    /// The class-based CSS rules the cached subtrees were built with. A change
    /// to a `<style>` element clears this root's entries.
    css_rules: CssClassRules,
    /// Cached subtrees keyed by the DOM node that produced them.
    entries: HashMap<OpaqueNode, SvgSubtreeCacheEntry>,
}

impl Default for SvgRootCache {
    fn default() -> Self {
        SvgRootCache {
            css_rules: CssClassRules::new(),
            entries: HashMap::new(),
        }
    }
}

/// One cached, self-contained subtree.
struct SvgSubtreeCacheEntry {
    /// The producing DOM node's `inclusive_descendants_version` when built.
    dom_version: u64,
    /// The viewport reference dimensions the subtree was built against.
    vw: f32,
    vh: f32,
    /// The inherited `currentColor` the subtree was built against.
    inherited_color: Option<SvgColor>,
    /// The producing node's computed style (`ComputedValues`) when built. Held
    /// by the entry so its address stays pinned, which makes the `ptr_eq`
    /// comparison on lookup sound (a real style change always allocates a new
    /// `Arc`).
    computed_style: Option<ServoArc<ComputedValues>>,
    /// The cached subtree, shared across reflows.
    subtree: Arc<SvgNode>,
}

impl SvgSubtreeCache {
    /// Ensure `root`'s cached subtrees were built against the same class-based
    /// CSS rules. If the rules changed (a `<style>` element was edited), clear
    /// the root's entries so they are rebuilt.
    pub(crate) fn ensure_css_rules(&mut self, root: OpaqueNode, css_rules: &CssClassRules) {
        let root_cache = self.roots.entry(root).or_default();
        if &root_cache.css_rules != css_rules {
            root_cache.css_rules = css_rules.clone();
            root_cache.entries.clear();
        }
    }

    /// Return a cached subtree if one is fresh for the given build inputs.
    pub(crate) fn lookup(
        &self,
        root: OpaqueNode,
        node: OpaqueNode,
        dom_version: u64,
        vw: f32,
        vh: f32,
        inherited_color: Option<SvgColor>,
        computed_style: Option<&ServoArc<ComputedValues>>,
    ) -> Option<Arc<SvgNode>> {
        let entry = self.roots.get(&root)?.entries.get(&node)?;
        if entry.dom_version != dom_version ||
            entry.vw != vw ||
            entry.vh != vh ||
            entry.inherited_color != inherited_color ||
            !same_computed_style(&entry.computed_style, computed_style)
        {
            return None;
        }
        Some(Arc::clone(&entry.subtree))
    }

    /// Store a freshly built subtree.
    pub(crate) fn store(
        &mut self,
        root: OpaqueNode,
        node: OpaqueNode,
        dom_version: u64,
        vw: f32,
        vh: f32,
        inherited_color: Option<SvgColor>,
        computed_style: Option<ServoArc<ComputedValues>>,
        subtree: Arc<SvgNode>,
    ) {
        let root_cache = self.roots.entry(root).or_default();
        root_cache.entries.insert(
            node,
            SvgSubtreeCacheEntry {
                dom_version,
                vw,
                vh,
                inherited_color,
                computed_style,
                subtree,
            },
        );
    }
}

/// Whether two computed styles are the very same allocation.
///
/// This is the style-change half of the cache key. The entry keeps the old
/// `Arc` alive, so its address cannot be reused; a real style change — an
/// ancestor's inherited property (e.g. `<g fill="blue">`), a stylesheet rule,
/// or a pseudo-class flip — always produces a new `ComputedValues` allocation.
/// `ptr_eq` therefore never misses a change (it can only over-rebuild when
/// Stylo re-allocates an equal style, which is safe). A `None` style (an
/// unstyled node) is only ever equal to another `None`.
fn same_computed_style(
    cached: &Option<ServoArc<ComputedValues>>,
    current: Option<&ServoArc<ComputedValues>>,
) -> bool {
    match (cached, current) {
        (None, None) => true,
        (Some(cached), Some(current)) => ServoArc::ptr_eq(cached, current),
        _ => false,
    }
}

/// Main entry point — builds a complete `SvgTree` from an SVG DOM element.
///
/// `viewport_width`/`viewport_height` are the resolved root viewport dimensions
/// (user units) computed by CSS layout; they feed the root [`SvgTree::viewport`]
/// and the percentage-resolution reference when no viewBox is present.
pub(crate) fn build_svg_tree<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<Arc<SvgTree>> {
    builder::SvgTreeBuilder::new(node, context, viewport_width, viewport_height).build()
}
