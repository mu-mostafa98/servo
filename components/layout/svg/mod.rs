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
/// would miss.
///
/// A `<use>` subtree is additionally keyed on its referenced target's version
/// and computed style ([`CrossRefFingerprint`]), so it is reused when neither
/// the `<use>` nor its target changed. A `<use>` whose target itself contains a
/// cross-reference (and any subtree containing a `<textPath>` or such a
/// `<use>`) is still never cached, because its target's target can change
/// without bumping either of their versions.
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
    /// For a `<use>` subtree, its referenced target's version and computed style
    /// (see [`CrossRefFingerprint`]). `None` for any other subtree.
    cross_ref: Option<CrossRefFingerprint>,
    /// The cached subtree, shared across reflows.
    subtree: Arc<SvgNode>,
}

/// A `<use>` subtree's dependency on its referenced target: the target's DOM
/// version and computed style. Only stored when the target is itself free of
/// cross-references, so this single pair fully captures when the `<use>` needs
/// rebuilding (the target's version covers its subtree's mutations, and its
/// computed style covers restyles that a version bump would miss).
#[derive(Clone)]
pub(crate) struct CrossRefFingerprint {
    pub(crate) version: u64,
    pub(crate) style: Option<ServoArc<ComputedValues>>,
}

/// Why a cache lookup missed — used for diagnostics so a rebuild's cause is
/// visible in the log, and for asserting the exact reason in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheMiss {
    /// No cached subtree for this node yet (first build).
    NoEntry,
    /// The node or one of its descendants was mutated since it was built.
    SubtreeDirty,
    /// The viewport reference dimensions (`vw`/`vh`) changed.
    Viewport,
    /// The inherited `currentColor` changed.
    CurrentColor,
    /// The node's computed style changed (own/inherited property, stylesheet,
    /// or pseudo-class).
    ComputedStyle,
    /// A `<use>`'s referenced target (its version or computed style) changed.
    ReferencedTarget,
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

    /// Return a cached subtree if one is fresh for the given build inputs, or
    /// the reason it is stale.
    pub(crate) fn lookup(
        &self,
        root: OpaqueNode,
        node: OpaqueNode,
        dom_version: u64,
        vw: f32,
        vh: f32,
        inherited_color: Option<SvgColor>,
        computed_style: Option<&ServoArc<ComputedValues>>,
        cross_ref: Option<&CrossRefFingerprint>,
    ) -> Result<Arc<SvgNode>, CacheMiss> {
        let Some(entry) = self.roots.get(&root).and_then(|root| root.entries.get(&node)) else {
            return Err(CacheMiss::NoEntry);
        };
        if entry.dom_version != dom_version {
            return Err(CacheMiss::SubtreeDirty);
        }
        if entry.vw != vw || entry.vh != vh {
            return Err(CacheMiss::Viewport);
        }
        if entry.inherited_color != inherited_color {
            return Err(CacheMiss::CurrentColor);
        }
        if !same_computed_style(&entry.computed_style, computed_style) {
            return Err(CacheMiss::ComputedStyle);
        }
        match (&entry.cross_ref, cross_ref) {
            (None, None) => {},
            (Some(entry), Some(current))
                if entry.version == current.version &&
                    same_computed_style(&entry.style, current.style.as_ref()) => {},
            _ => return Err(CacheMiss::ReferencedTarget),
        }
        Ok(Arc::clone(&entry.subtree))
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
        cross_ref: Option<CrossRefFingerprint>,
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
                cross_ref,
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
fn same_computed_style<T>(
    cached: &Option<ServoArc<T>>,
    current: Option<&ServoArc<T>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use servo_svg::element::{Container, SvgTag};
    use servo_svg::style::NodeStyle;

    /// A minimal cacheable subtree: a `<g>` with no id, style, transform,
    /// viewport, or children.
    fn dummy_subtree() -> Arc<SvgNode> {
        Arc::new(SvgNode {
            id: None,
            tag: SvgTag::Container(Container::Group),
            style: NodeStyle::default(),
            transforms: Vec::new(),
            viewport: None,
            children: Vec::new(),
        })
    }

    #[test]
    fn computed_style_identity_matches_only_same_allocation() {
        // The cache keys on allocation identity, not value equality: a real
        // style change always produces a new `Arc`, so `ptr_eq` never misses a
        // change.
        let a = ServoArc::new(1u32);
        let b = ServoArc::new(1u32);

        assert!(same_computed_style(&Some(ServoArc::clone(&a)), Some(&a)));
        assert!(!same_computed_style(&Some(a), Some(&b)));
        // A cached `None` style matches only a current `None`.
        assert!(same_computed_style::<u32>(&None, None));
        assert!(!same_computed_style(&Some(b), None));
    }

    #[test]
    fn cache_reuses_clean_subtree_and_reports_miss_reason() {
        let root = OpaqueNode(0);
        let node = OpaqueNode(1);
        let mut cache = SvgSubtreeCache::default();
        let subtree = dummy_subtree();

        // First lookup has no entry.
        assert!(matches!(
            cache.lookup(root, node, 0, 100.0, 100.0, None, None, None),
            Err(CacheMiss::NoEntry)
        ));

        cache.store(root, node, 0, 100.0, 100.0, None, None, None, Arc::clone(&subtree));

        // A clean lookup (same inputs) reuses the exact subtree.
        match cache.lookup(root, node, 0, 100.0, 100.0, None, None, None) {
            Ok(hit) => assert!(Arc::ptr_eq(&hit, &subtree)),
            Err(reason) => panic!("expected a cache hit, got {reason:?}"),
        }

        // Each changed input misses with the matching reason.
        assert!(matches!(
            cache.lookup(root, node, 1, 100.0, 100.0, None, None, None),
            Err(CacheMiss::SubtreeDirty)
        ));
        assert!(matches!(
            cache.lookup(root, node, 0, 200.0, 100.0, None, None, None),
            Err(CacheMiss::Viewport)
        ));
        assert!(matches!(
            cache.lookup(root, node, 0, 100.0, 100.0, Some(SvgColor::black()), None, None),
            Err(CacheMiss::CurrentColor)
        ));
        assert!(matches!(
            cache.lookup(
                root,
                node,
                0,
                100.0,
                100.0,
                None,
                Some(&ServoArc::new(1u32)),
                None,
            ),
            Err(CacheMiss::ComputedStyle)
        ));
    }

    #[test]
    fn cache_keys_use_subtrees_on_their_referenced_target() {
        let root = OpaqueNode(0);
        let node = OpaqueNode(1);
        let mut cache = SvgSubtreeCache::default();
        let subtree = dummy_subtree();

        let dep = CrossRefFingerprint {
            version: 5,
            style: None,
        };
        cache.store(root, node, 0, 100.0, 100.0, None, None, Some(dep), Arc::clone(&subtree));

        // Same target version + style → hit.
        let current = CrossRefFingerprint {
            version: 5,
            style: None,
        };
        assert!(matches!(
            cache.lookup(root, node, 0, 100.0, 100.0, None, None, Some(&current)),
            Ok(_)
        ));

        // Target version bumped → miss with the cross-reference reason.
        let changed = CrossRefFingerprint {
            version: 6,
            style: None,
        };
        assert!(matches!(
            cache.lookup(root, node, 0, 100.0, 100.0, None, None, Some(&changed)),
            Err(CacheMiss::ReferencedTarget)
        ));
    }
}
