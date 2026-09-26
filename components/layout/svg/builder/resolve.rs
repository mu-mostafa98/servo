/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Child/`<use>` resolution and reference resolution for the SVG builder.
//!
//! Two passes live here:
//! 1. **Child resolution** — walking the DOM to build child nodes, including
//!    cloning `<use>`-referenced content.
//! 2. **Reference resolution** — rewriting transient `PaintServer::Ref` /
//!    `DefRef::Ref` handles into typed `Arc` handles after definition maps are
//!    collected.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::document::{
    ClipPathDef, DefRef, FilterDef, MarkerDef, MaskDef, PatternDef, SvgTree, SvgViewport,
};
use svg_engine::element::{Container, SvgNode, SvgTag};
use svg_engine::style::NodeStyle;
use svg_engine::style::paint_servers::{GradientDef, PaintServer};
use svg_engine::style::transform::TransformOp;
use svg_engine::units::Length;
use web_atoms::ns;

use super::SvgTreeBuilder;
use crate::svg::primitives::attrs::{get_attr, parse_length_value};
use crate::svg::primitives::viewport::{extract_viewbox, parse_aspect_ratio};

/// Mutable expansion state threaded through the recursive tree build.
///
/// Guards against pathological `<use>` graphs (§5.6) — deep acyclic chains
/// (stack overflow), exponential "billion laughs" fan-out, quadratic blow-up,
/// and command amplification — by bounding reference-nesting depth and total
/// node output (see README §9.2, issues #3–#6).
#[derive(Default)]
pub(crate) struct ResolveState {
    /// DFS path-set: ids currently being resolved, to detect reference cycles.
    pub(crate) resolving: HashSet<String>,
    /// Current `<use>` reference-nesting depth — a guard against a deep acyclic
    /// chain of distinct references (which the cycle guard never trips on).
    pub(crate) use_depth: usize,
    /// Build-work budget consumed so far — a generous global cap on the total
    /// number of nodes materialized, which bounds exponential fan-out and other
    /// output amplification.
    pub(crate) nodes: usize,
}

/// Maximum nested `<use>` reference depth before expansion is cut off. Deeper
/// chains are pathological; legitimate content rarely nests `<use>` beyond a
/// handful of levels.
pub(crate) const MAX_USE_DEPTH: usize = 64;

/// Maximum total render nodes materialized per build. Chosen high enough that
/// legitimate documents are unaffected (real-world SVGs number in the
/// thousands of elements), but low enough to terminate exponential `<use>`
/// blow-up well before memory is exhausted.
pub(crate) const MAX_TOTAL_NODES: usize = 1_000_000;

// ======================= Children Resolution =======================

/// Resolve children for a render node.
/// For `<use>`, clones the referenced element with x/y translation.
/// For all others, recursively builds children from DOM.
pub(crate) fn resolve_children<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &SvgTag,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, '_>,
    state: &mut ResolveState,
    node_style: &NodeStyle,
    in_shadow: bool,
    vw: f32,
    vh: f32,
) -> Vec<SvgNode> {
    if let SvgTag::Container(Container::Use) = tag {
        resolve_use_children(node, root_node, builder, state, node_style, vw, vh)
    } else if let SvgTag::Container(Container::Switch) = tag {
        resolve_switch_children(
            node, root_node, builder, state, node_style, in_shadow, vw, vh,
        )
    } else {
        // Manual inheritance only applies inside a `<use>` shadow tree; for
        // normal content Stylo already resolves inherited properties along the
        // real DOM ancestry.
        let child_inherited = if in_shadow {
            Some(node_style)
        } else {
            None
        };
        node.dom_children()
            .filter_map(|child| builder.build_render_node(child, root_node, state, child_inherited, vw, vh))
            .collect()
    }
}

/// Resolve children for a `<switch>` element (§5.7.2).
///
/// A `<switch>` renders the first of its children for which all conditional
/// processing attributes test true. Since a child whose tests fail is already
/// rejected by [`SvgTreeBuilder::build_render_node`] (returning `None`), this
/// simply builds children in document order and keeps the first one that
/// produces a render node. The `display`/`visibility` properties of the
/// children are ignored for selection (§5.7.3).
fn resolve_switch_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, '_>,
    state: &mut ResolveState,
    node_style: &NodeStyle,
    in_shadow: bool,
    vw: f32,
    vh: f32,
) -> Vec<SvgNode> {
    // Manual inheritance only applies inside a `<use>` shadow tree; otherwise
    // Stylo already resolved inherited properties along the real DOM ancestry.
    let child_inherited = if in_shadow {
        Some(node_style)
    } else {
        None
    };
    for child in node.dom_children() {
        if let Some(built) =
            builder.build_render_node(child, root_node, state, child_inherited, vw, vh)
        {
            return vec![built];
        }
    }
    vec![]
}

/// Resolve children for a `<use>` element (§5.6).
///
/// Looks up the referenced element by its `#id`, builds its render node, and
/// clones it as a child. The `<use>` element's `x`/`y` translate the
/// instantiated content; for a referenced `<symbol>` or nested `<svg>`, the
/// `<use>` element's `width`/`height` (and, for `<symbol>`, also `x`/`y`)
/// override the referenced element's viewport geometry (§5.6.2). A negative
/// `width` or `height` is an error and a value of zero disables rendering
/// (§5.6.3).
fn resolve_use_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, '_>,
    state: &mut ResolveState,
    use_style: &NodeStyle,
    vw: f32,
    vh: f32,
) -> Vec<SvgNode> {
    let element = node.as_element().unwrap();

    // Extract href reference.
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
        });

    let Some(ref_id) = ref_id else { return vec![] };
    if state.resolving.contains(&ref_id) {
        return vec![];
    }

    // Parse `x`/`y`/`width`/`height` — percentages resolve against the current
    // viewport (`x`/`width` against width, `y`/`height` against height).
    let parse_coord = |attr: &str, reference: f32| -> Option<f32> {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .and_then(|v| parse_length_value(v, 16.0, reference))
    };
    let use_x = parse_coord("x", vw);
    let use_y = parse_coord("y", vh);
    let use_width = parse_coord("width", vw);
    let use_height = parse_coord("height", vh);

    // §5.6.3: a negative `width`/`height` on `<use>` is an error (render
    // nothing); a value of zero disables rendering of the instantiated content.
    if use_width.is_some_and(|w| w < 0.0) || use_height.is_some_and(|h| h < 0.0) {
        return vec![];
    }
    if use_width.is_some_and(|w| w == 0.0) || use_height.is_some_and(|h| h == 0.0) {
        return vec![];
    }

    // §5.6: bound `<use>` expansion. A deep acyclic chain — where each reference
    // is a distinct id, so the cycle guard above never trips — would otherwise
    // overflow the stack (README §9.2, issue #3).
    if state.use_depth >= MAX_USE_DEPTH {
        return vec![];
    }
    state.use_depth += 1;

    state.resolving.insert(ref_id.clone());

    let offset = (use_x, use_y);

    // Build target and clone with optional translation.
    let target = builder.element_ids.get(&ref_id).copied();
    let target_element = target.as_ref().and_then(|n| n.as_element());

    // The referenced element's viewport attributes, used when the target is a
    // `<symbol>` or nested `<svg>` whose viewport geometry the `<use>` may
    // override (§5.6.2).
    let parse_len = |e: &ServoLayoutElement, name: &str, reference: f32| -> Option<f32> {
        get_attr(e, name).and_then(|s| parse_length_value(&s, 16.0, reference))
    };
    let sym_view_box = target_element
        .and_then(|e| get_attr(&e, "viewBox"))
        .as_deref()
        .and_then(extract_viewbox);
    let sym_aspect_ratio = target_element
        .and_then(|e| get_attr(&e, "preserveAspectRatio"))
        .as_deref()
        .map(parse_aspect_ratio);
    let sym_x = target_element.and_then(|e| parse_len(&e, "x", vw));
    let sym_y = target_element.and_then(|e| parse_len(&e, "y", vh));
    let sym_width = target_element.and_then(|e| parse_len(&e, "width", vw));
    let sym_height = target_element.and_then(|e| parse_len(&e, "height", vh));

    let result = target
        .and_then(|t| builder.build_render_node(t, root_node, state, Some(use_style), vw, vh))
        .map(|target_node| {
            // Shared helper: apply `<use>` x/y offset as a translate transform.
            let apply_offset = |node: &mut SvgNode| {
                if let (Some(dx), Some(dy)) = offset {
                    if dx != 0.0 || dy != 0.0 {
                        node.transforms
                            .insert(0, TransformOp::Translate(dx, dy));
                    }
                }
            };

            let is_symbol = matches!(&target_node.tag, SvgTag::Container(Container::Symbol));
            let is_svg = matches!(&target_node.tag, SvgTag::Container(Container::Svg));

            // `<symbol>` is never rendered directly. When referenced by `<use>`
            // it establishes a viewport (like a nested `<svg>`) from its own
            // `viewBox` / `x` / `y` / `width` / `height`, with the `<use>`
            // element's attributes taking precedence (§5.5, §5.6.2).
            if is_symbol {
                let has_geometry = sym_view_box.is_some()
                    || sym_x.is_some()
                    || sym_y.is_some()
                    || sym_width.is_some()
                    || sym_height.is_some()
                    || use_width.is_some()
                    || use_height.is_some();
                if has_geometry {
                    let width = use_width
                        .or(sym_width)
                        .or_else(|| sym_view_box.map(|vb| vb.width.get()))
                        .unwrap_or(vw);
                    let height = use_height
                        .or(sym_height)
                        .or_else(|| sym_view_box.map(|vb| vb.height.get()))
                        .unwrap_or(vh);
                    let wrapper = SvgNode {
                        id: target_node.id,
                        tag: SvgTag::Container(Container::Group),
                        style: target_node.style,
                        transforms: Vec::new(),
                        viewport: Some(SvgViewport {
                            x: Length::new(use_x.or(sym_x).unwrap_or(0.0)),
                            y: Length::new(use_y.or(sym_y).unwrap_or(0.0)),
                            width: Length::new(width),
                            height: Length::new(height),
                            view_box: sym_view_box,
                            aspect_ratio: sym_aspect_ratio,
                            overflow_visible: false,
                        }),
                        children: target_node.children,
                    };
                    return vec![wrapper];
                }

                // No viewBox and no geometry — unwrap the children with the
                // x/y offset applied.
                let mut children = target_node.children;
                for child in &mut children {
                    apply_offset(child);
                }
                return children;
            }

            let mut cloned = target_node;
            // §5.6.2: for a referenced nested `<svg>`, the `<use>` element's
            // `width`/`height` override the `<svg>` element's.
            if is_svg {
                if let Some(vp) = cloned.viewport.as_mut() {
                    if let Some(w) = use_width {
                        vp.width = Length::new(w);
                    }
                    if let Some(h) = use_height {
                        vp.height = Length::new(h);
                    }
                }
            }
            apply_offset(&mut cloned);
            vec![cloned]
        })
        .unwrap_or_default();

    state.use_depth -= 1;
    state.resolving.remove(&ref_id);
    result
}

// ======================= Reference Resolution =======================

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

    if let Some(effects) = node.style.effects.as_mut() {
        effects.clip_path = resolve_ref(effects.clip_path.take(), clip_paths);
        effects.mask = resolve_ref(effects.mask.take(), masks);
        effects.filter = resolve_ref(effects.filter.take(), filters);
    }
    if let Some(effects) = node.style.effects.as_ref() {
        if effects.clip_path.is_none() && effects.mask.is_none() && effects.filter.is_none() {
            node.style.effects = None;
        }
    }

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
