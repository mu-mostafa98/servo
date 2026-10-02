/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Reference resolution — the second build pass.
//!
//! After the definition maps are collected (see [`crate::svg::defines`]) and the
//! render tree is assembled, every *transient* reference left in a `NodeStyle`
//! — [`PaintServer::Ref`] and [`DefRef::Ref`] — is rewritten here into a typed
//! `Arc` handle. This is the servo-svg equivalent of usvg's `resolve_effects`
//! pass: it is the one place where `url(#id)` is finally bound to a definition.
//!
//! Resolution rules follow SVG 2's broken-reference behavior:
//! - a paint-server reference resolving to neither a gradient nor a pattern
//!   uses its fallback color, or — with no fallback — drops the paint layer;
//! - a clip-path / mask / filter / marker reference that does not resolve is
//!   dropped (the effect/marker is omitted).

use std::collections::HashMap;
use std::sync::Arc;

use servo_svg::document::{
    ClipPathDef, DefRef, FilterDef, GradientDef, MarkerDef, MaskDef, PatternDef, SvgTree,
};
use servo_svg::element::SvgNode;
use servo_svg::style::paint_servers::PaintServer;

/// Rewrite every transient reference in the tree — [`PaintServer::Ref`] paint
/// servers and [`DefRef::Ref`] clip-path/mask/filter/marker handles — into typed
/// `Arc` handles using the collected definition maps.
///
/// A paint-server reference that resolves to neither a gradient nor a pattern
/// uses its fallback color, or — with no fallback — drops the paint layer
/// entirely (SVG 2 behavior). A clip-path/mask/filter/marker reference that
/// does not resolve is dropped (the effect/marker is omitted), matching SVG's
/// ignore-broken-references behavior.
pub(crate) fn resolve_references(tree: &mut SvgTree) {
    let SvgTree {
        root,
        gradients,
        patterns,
        clip_paths,
        masks,
        filters,
        markers: marker_defs,
        ..
    } = tree;
    resolve_references_in(
        root,
        gradients,
        patterns,
        clip_paths,
        masks,
        filters,
        marker_defs,
    );
}

fn resolve_references_in(
    node: &mut SvgNode,
    gradients: &HashMap<String, Arc<GradientDef>>,
    patterns: &HashMap<String, Arc<PatternDef>>,
    clip_paths: &HashMap<String, Arc<ClipPathDef>>,
    masks: &HashMap<String, Arc<MaskDef>>,
    filters: &HashMap<String, Arc<FilterDef>>,
    marker_defs: &HashMap<String, Arc<MarkerDef>>,
) {
    let fill_keep = match node.style.fill.as_mut().and_then(|f| f.paint_server.as_mut()) {
        Some(paint) => resolve_paint_server(paint, gradients, patterns),
        None => true,
    };
    if !fill_keep {
        node.style.fill = None;
    }
    let stroke_keep = match node.style.stroke.as_mut().and_then(|s| s.paint_server.as_mut()) {
        Some(paint) => resolve_paint_server(paint, gradients, patterns),
        None => true,
    };
    if !stroke_keep {
        node.style.stroke = None;
    }

    node.style.clip_path = resolve_ref(node.style.clip_path.take(), clip_paths);
    node.style.mask = resolve_ref(node.style.mask.take(), masks);
    node.style.filter = resolve_ref(node.style.filter.take(), filters);

    if let Some(refs) = node.style.markers.as_mut() {
        refs.start = resolve_ref(refs.start.take(), marker_defs);
        refs.mid = resolve_ref(refs.mid.take(), marker_defs);
        refs.end = resolve_ref(refs.end.take(), marker_defs);
    }
    if let Some(refs) = node.style.markers.as_ref() {
        if refs.start.is_none() && refs.mid.is_none() && refs.end.is_none() {
            node.style.markers = None;
        }
    }

    for child in &mut node.children {
        resolve_references_in(
            child,
            gradients,
            patterns,
            clip_paths,
            masks,
            filters,
            marker_defs,
        );
    }
}

/// Resolve a transient [`PaintServer::Ref`] in place.
///
/// Returns `false` when the reference is broken *and* has no fallback, in which
/// case the caller must drop the whole paint layer (SVG 2: "no paint is
/// rendered"). A non-`Ref` paint server is already resolved and returns `true`.
fn resolve_paint_server(
    paint: &mut PaintServer,
    gradients: &HashMap<String, Arc<GradientDef>>,
    patterns: &HashMap<String, Arc<PatternDef>>,
) -> bool {
    let (id, fallback) = match paint {
        PaintServer::Ref { id, fallback } => (id.clone(), fallback.take()),
        _ => return true,
    };
    if let Some(def) = gradients.get(id.as_str()) {
        *paint = PaintServer::Gradient(def.clone());
    } else if let Some(def) = patterns.get(id.as_str()) {
        *paint = PaintServer::Pattern(def.clone());
    } else if let Some(color) = fallback {
        *paint = PaintServer::Solid(color);
    } else {
        return false;
    }
    true
}

/// Resolve a transient [`DefRef::Ref`] into a typed handle using `map`.
/// An unresolved reference is dropped (returned as `None`); an already-resolved
/// handle is passed through unchanged.
fn resolve_ref<T>(
    reference: Option<DefRef<T>>,
    map: &HashMap<String, Arc<T>>,
) -> Option<DefRef<T>> {
    match reference {
        Some(DefRef::Ref(id)) => map.get(id.as_str()).cloned().map(DefRef::Resolved),
        other => other,
    }
}
