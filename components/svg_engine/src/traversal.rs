/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.
//!
//! Walks a [`usvg::Tree`] recursively, dispatching shapes to emitters
//! and feeding paint commands to the renderer.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};

use crate::emitter::{Emit, EmitContext, PaintCommand};
use crate::renderer::{Backend, Renderer, webrender::WebRenderBackend};

// ======================= Public Entry Points =======================

/// Render an SVG tree into a WebRender display list (screen output).
/// Convenience wrapper around [`render_svg_tree_to`] that sets up the viewport clip.
pub fn render_svg_tree(
    tree: &usvg::Tree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Clip to viewport bounds (WebRender-specific).
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let parent = (clip_chain_id != ClipChainId::INVALID).then_some(clip_chain_id);
    let svg_clip_chain = wr.define_clip_chain(parent, [svg_clip_id]);

    let mut backend = WebRenderBackend { wr };
    render_svg_tree_to(tree, svg_origin, &mut backend, spatial_id, svg_clip_chain);
}

/// Render an SVG tree to any backend (WebRender, Krilla, etc.).
///
/// Collects paint commands from emitters and dispatches them to the given backend.
/// The backend determines the output target — GPU screen, PDF file, etc.
pub fn render_svg_tree_to<B: Backend>(
    tree: &usvg::Tree,
    svg_origin: &LayoutPoint,
    backend: &mut B,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) {
    let mut commands: Vec<PaintCommand> = Vec::new();
    let emit_ctx = EmitContext { svg_origin: *svg_origin };
    emit_group(tree.root(), &emit_ctx, &mut commands);

    let renderer = Renderer { commands };
    renderer.render(backend, spatial_id, clip_chain_id);
}

// ======================= Group Traversal =======================

fn emit_group(group: &usvg::Group, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
    // Apply group's transform.
    let sub_ctx = if group.transform().is_identity() {
        EmitContext { svg_origin: ctx.svg_origin }
    } else {
        EmitContext {
            svg_origin: LayoutPoint::new(
                ctx.svg_origin.x + group.transform().tx,
                ctx.svg_origin.y + group.transform().ty,
            ),
        }
    };

    for child in group.children() {
        emit_node(child, &sub_ctx, commands);
    }
}

fn emit_node(node: &usvg::Node, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
    match node {
        usvg::Node::Group(g) => emit_group(g, ctx, commands),
        usvg::Node::SimpleShape(shape) => shape.emit(ctx, commands),
        // Path, Image, Text — future (Vello CPU terminal)
        _ => {}
    }
}
