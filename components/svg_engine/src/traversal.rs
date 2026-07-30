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
use crate::renderer::{Renderer, webrender::WebRenderBackend};

// ======================= Public Entry Point =======================

/// Render an SVG tree into a WebRender display list.
pub fn render_svg_tree(
    tree: &usvg::Tree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Clip to viewport bounds.
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let parent = (clip_chain_id != ClipChainId::INVALID).then_some(clip_chain_id);
    let svg_clip_chain = wr.define_clip_chain(parent, [svg_clip_id]);

    // Collect paint commands from emitters.
    let mut commands: Vec<PaintCommand> = Vec::new();
    let emit_ctx = EmitContext { svg_origin: *svg_origin };
    emit_group(tree.root(), &emit_ctx, &mut commands);

    // Render via WebRender backend.
    let mut backend = WebRenderBackend { wr };
    let renderer = Renderer { commands };
    renderer.render(&mut backend, spatial_id, svg_clip_chain);
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
