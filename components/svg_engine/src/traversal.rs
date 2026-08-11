/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.
//!
//! Walks a [`usvg::Tree`] recursively, dispatching shapes to emitters
//! and feeding paint commands to the renderer.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};

use std::sync::Mutex;

use crate::emitter::{Emit, EmitContext, PaintCommand};
use crate::renderer::{Backend, Renderer, krilla::KrillaBackend, webrender::WebRenderBackend};
use crate::{FontKeyRegistry, GlyphStore, RasterizedImage, SvgRenderData};

static PDF_BACKEND: Mutex<Option<KrillaBackend>> = Mutex::new(None);
static PDF_Y: Mutex<f32> = Mutex::new(0.0);

// ======================= Public Entry Points =======================

/// Render an SVG tree into a WebRender display list (screen output).
/// Convenience wrapper around [`render_svg_tree_to`] that sets up the viewport clip.
/// Returns rasterized images from the path emitter for compositor upload.
pub fn render_svg_tree(
    data: &SvgRenderData,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> Vec<RasterizedImage> {
    // Clip to viewport bounds (WebRender-specific).
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let parent = (clip_chain_id != ClipChainId::INVALID).then_some(clip_chain_id);
    let svg_clip_chain = wr.define_clip_chain(parent, [svg_clip_id]);

    let mut backend = WebRenderBackend { wr, font_keys: data.font_keys.clone() };
    let images = render_svg_tree_to(
        &data.tree, svg_origin, &data.glyphs, &data.font_keys,
        &mut backend, spatial_id, svg_clip_chain,
    );

    // Also render to PDF — stack SVGs vertically. PDF has no FontInstanceKey
    // concept, so its text rendering stays path-based.
    if let Ok(mut opt) = PDF_BACKEND.lock() {
        if opt.is_none() {
            *opt = Some(KrillaBackend::new(800.0, 2000.0));
        }
        if let Some(ref mut pdf) = *opt {
            let y = {
                let guard = PDF_Y.lock().unwrap();
                let y = *guard;
                drop(guard);
                y
            };
            render_svg_tree_to(
                &data.tree, &LayoutPoint::new(svg_origin.x, y),
                &data.glyphs, &data.font_keys,
                pdf, spatial_id, clip_chain_id,
            );
            if let Ok(mut gy) = PDF_Y.lock() {
                *gy = y + svg_size.height.max(data.tree.size().height()) + 20.0;
            }
            let _ = std::fs::write("svg.pdf", pdf.finish());
        }
    }

    images
}

/// Render an SVG tree to any backend (WebRender, Krilla, etc.).
///
/// Collects paint commands from emitters and dispatches them to the given backend.
/// The backend determines the output target — GPU screen, PDF file, etc.
/// Returns extracted rasterized images (from path emitter) for compositor upload.
pub fn render_svg_tree_to<B: Backend>(
    tree: &usvg::Tree,
    svg_origin: &LayoutPoint,
    glyphs: &GlyphStore,
    font_keys: &FontKeyRegistry,
    backend: &mut B,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) -> Vec<RasterizedImage> {
    let mut commands: Vec<PaintCommand> = Vec::new();
    let emit_ctx = EmitContext {
        svg_origin: *svg_origin,
        glyphs,
        font_keys,
    };
    emit_group(tree.root(), &emit_ctx, &mut commands);

    // Extract rasterized images before consuming commands
    let images: Vec<RasterizedImage> = commands.iter().enumerate().filter_map(|(_i, cmd)| {
        if let PaintCommand::DrawImage { x, y, w, h, data, .. } = cmd {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            data.hash(&mut hasher);
            let hash = hasher.finish();
            Some(RasterizedImage { x: *x, y: *y, width: *w, height: *h, data: data.clone(), content_hash: hash })
        } else {
            None
        }
    }).collect();

    let renderer = Renderer { commands };
    renderer.render(backend, spatial_id, clip_chain_id);
    images
}

// ======================= Group Traversal =======================

fn emit_group(group: &usvg::Group, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
    // Apply group's transform.
    let sub_ctx = if group.transform().is_identity() {
        EmitContext {
            svg_origin: ctx.svg_origin,
            glyphs: ctx.glyphs,
            font_keys: ctx.font_keys,
        }
    } else {
        EmitContext {
            svg_origin: LayoutPoint::new(
                ctx.svg_origin.x + group.transform().tx,
                ctx.svg_origin.y + group.transform().ty,
            ),
            glyphs: ctx.glyphs,
            font_keys: ctx.font_keys,
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
        usvg::Node::Path(path) => path.emit(ctx, commands),
        usvg::Node::Image(img) => img.emit(ctx, commands),
        usvg::Node::Text(text) => text.emit(ctx, commands),
    }
}
