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
use svg_engine::style::gradient::{GradientDef, PaintServer};
use svg_engine::style::transform::TransformOp;
use svg_engine::units::Length;
use web_atoms::ns;

use super::SvgTreeBuilder;
use crate::svg::primitives::attrs::get_attr;
use crate::svg::primitives::viewport::{extract_viewbox, parse_aspect_ratio};

// ======================= Children Resolution =======================

/// Resolve children for a render node.
/// For `<use>`, clones the referenced element with x/y translation.
/// For all others, recursively builds children from DOM.
pub(crate) fn resolve_children<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &SvgTag,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
    node_style: &NodeStyle,
    in_shadow: bool,
    vw: f32,
    vh: f32,
) -> Vec<SvgNode> {
    if let SvgTag::Container(Container::Use) = tag {
        resolve_use_children(node, root_node, builder, resolving, node_style, vw, vh)
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
            .filter_map(|child| builder.build_render_node(child, root_node, resolving, child_inherited, vw, vh))
            .collect()
    }
}

/// Resolve children for a `<use>` element.
///
/// Looks up the referenced element by its `#id`, builds its render node,
/// clones it as a child, and applies x/y translation if specified.
fn resolve_use_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
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
    if resolving.contains(&ref_id) {
        return vec![];
    }
    resolving.insert(ref_id.clone());

    // Parse x/y offset.
    let parse_coord = |attr: &str| -> Option<f32> {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
    };
    let offset = (parse_coord("x"), parse_coord("y"));

    // Build target and clone with optional translation.
    let target = builder.element_ids.get(&ref_id).copied();
    let target_element = target.as_ref().and_then(|n| n.as_element());

    // The referenced element's viewport attributes, used when the target is a
    // <symbol> whose viewBox maps its internal coordinates onto the viewport
    // declared by the <use> (falling back to the symbol's own width/height).
    let parse_len = |e: &ServoLayoutElement, name: &str| -> Option<f32> {
        get_attr(e, name)
            .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
    };
    let sym_view_box = target_element
        .and_then(|e| get_attr(&e, "viewBox"))
        .as_deref()
        .and_then(extract_viewbox);
    let sym_aspect_ratio = target_element
        .and_then(|e| get_attr(&e, "preserveAspectRatio"))
        .as_deref()
        .map(parse_aspect_ratio);
    let sym_width = target_element.and_then(|e| parse_len(&e, "width"));
    let sym_height = target_element.and_then(|e| parse_len(&e, "height"));

    let result = target
        .and_then(|t| builder.build_render_node(t, root_node, resolving, Some(use_style), vw, vh))
        .map(|target_node| {
            // Shared helper: apply <use> x/y offset as a translate transform.
            let apply_offset = |node: &mut SvgNode| {
                if let (Some(dx), Some(dy)) = offset {
                    if dx != 0.0 || dy != 0.0 {
                        node.transforms
                            .insert(0, TransformOp::Translate(dx, dy));
                    }
                }
            };

            // <symbol> is never rendered directly. When it carries a viewBox,
            // wrap its children in a viewport-carrying group so the traversal
            // maps the symbol's coordinates onto the <use> viewport (the same
            // viewBox → viewport machinery used for nested <svg> elements).
            if let SvgTag::Container(Container::Symbol) = &target_node.tag {
                if let Some(vb) = sym_view_box {
                    let width = parse_coord("width").or(sym_width).unwrap_or(vb.width.get());
                    let height = parse_coord("height").or(sym_height).unwrap_or(vb.height.get());
                    let wrapper = SvgNode {
                        id: target_node.id,
                        tag: SvgTag::Container(Container::Group),
                        style: target_node.style,
                        transforms: Vec::new(),
                        viewport: Some(SvgViewport {
                            x: Length::new(offset.0.unwrap_or(0.0)),
                            y: Length::new(offset.1.unwrap_or(0.0)),
                            width: Length::new(width),
                            height: Length::new(height),
                            view_box: Some(vb),
                            aspect_ratio: sym_aspect_ratio,
                            overflow_visible: false,
                        }),
                        children: target_node.children,
                    };
                    return vec![wrapper];
                }

                // No viewBox — unwrap the children with the x/y offset applied.
                let mut children = target_node.children;
                for child in &mut children {
                    apply_offset(child);
                }
                return children;
            }

            let mut cloned = target_node;
            apply_offset(&mut cloned);
            vec![cloned]
        })
        .unwrap_or_default();

    resolving.remove(&ref_id);
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
