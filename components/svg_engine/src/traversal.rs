/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal — walks the [`SvgRenderTree`] and emits
//! WebRender display list commands via each shape's [`Render`] impl.
//!
//! **Single responsibility:** coordinate the tree walk.  Clip/mask/filter
//! resolution is delegated to [`crate::effects`]; transform application is
//! delegated to [`crate::renderer::transform`]; per-shape rendering is
//! delegated to the [`Render`] trait.
//!
//! The entry point is [`render_svg_tree`], which sets up the viewport
//! clip and viewBox transform, then recursively walks the node tree.

use euclid::Transform2D;
use webrender_api::{
    ClipChainId, DisplayListBuilder,
    MixBlendMode, PrimitiveFlags, PropertyBinding,
    RasterSpace, ReferenceFrameKind, SpatialId, StackingContextFlags,
    TransformStyle, units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::effects::clip::{resolve_node_clip_path, build_mask_clips};
use crate::effects::filter::get_filter_ops;
use crate::render_tree::*;
use crate::renderer::transform;
use crate::renderer::{Render, RenderContext, clip_chain_option};
use crate::renderer::{PaintResourceProvider, ClipMaskProvider, FilterProvider};

/// Render an SVG render tree into the WebRender display list.
pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Apply SVG viewport clip.
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let svg_clip_chain = wr.define_clip_chain(
        clip_chain_option(clip_chain_id),
        [svg_clip_id],
    );

    // Apply viewBox transform if present.
    let (root_origin, root_spatial_id, pop_frame) =
        if let Some(ref vb) = tree.viewport.view_box {
            let scale_x = svg_size.width / vb.width;
            let scale_y = svg_size.height / vb.height;
            let t1: Transform2D<f32, (), ()> = Transform2D::translation(-vb.min_x, -vb.min_y);
            let s: Transform2D<f32, (), ()> = Transform2D::scale(scale_x, scale_y);
            let combined = t1.then(&s);
            let lt = transform::to_layout_transform(&combined);
            let frame_id = wr.push_reference_frame(
                *svg_origin,
                spatial_id,
                TransformStyle::Flat,
                PropertyBinding::Value(lt),
                ReferenceFrameKind::Transform {
                    is_2d_scale_translation: false,
                    should_snap: false,
                    paired_with_perspective: false,
                },
            );
            (LayoutPoint::new(0.0, 0.0), frame_id, true)
        } else {
            (*svg_origin, spatial_id, false)
        };

    render_node(
        &tree.root, &root_origin, root_spatial_id, svg_clip_chain, wr,
        tree, tree, tree, 1.0,
    );

    if pop_frame {
        wr.pop_reference_frame();
    }
}

// ======================= Recursive Node Rendering =======================

/// Recursively render a single node and its children.
///
/// `parent_scale` is the accumulated transform scale from all ancestor
/// transforms (excluding this node's own transforms). Used for
/// `vector-effect: non-scaling-stroke` compensation.
fn render_node(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
    paints: &impl PaintResourceProvider,
    clips: &impl ClipMaskProvider,
    filters: &impl FilterProvider,
    parent_scale: f32,
) {
    if !node.style.is_displayed() {
        return;
    }

    let (cur_origin, cur_spatial_id, pushed_count, accumulated_scale) =
        apply_node_transforms(node, svg_origin, spatial_id, parent_scale, wr);

    // Resolve effects (clip-path, mask, filter).
    let node_clip_chain = resolve_node_clip_path(
        node, clips, &cur_origin, cur_spatial_id, clip_chain_id, wr,
    );
    let mask_clips = build_mask_clips(
        node, clips, &cur_origin, cur_spatial_id, node_clip_chain, wr,
    );
    let filter_ops = get_filter_ops(node, filters);

    // Render shape if this node is a visible shape.
    render_shape(
        node, &cur_origin, cur_spatial_id, node_clip_chain,
        &mask_clips, &filter_ops, accumulated_scale, paints, wr,
    );

    // Recurse into children (skip <defs> — non-visual).
    recurse_children(
        node, &cur_origin, cur_spatial_id, node_clip_chain,
        clip_chain_id, paints, clips, filters, accumulated_scale, wr,
    );

    // Pop reference frames pushed by transforms.
    for _ in 0..pushed_count {
        wr.pop_reference_frame();
    }
}

/// Apply this node's transform operations onto the WebRender display list.
///
/// Returns the new origin, spatial id, number of reference frames pushed,
/// and the accumulated scale factor (for `vector-effect: non-scaling-stroke`).
fn apply_node_transforms(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_scale: f32,
    wr: &mut DisplayListBuilder,
) -> (LayoutPoint, SpatialId, u32, f32) {
    let mut cur_spatial_id = spatial_id;
    let mut cur_origin = *svg_origin;
    let mut pushed_count: u32 = 0;

    let node_scale = transform::compute_transform_scale(&node.style.transform);
    let accumulated_scale = parent_scale * node_scale;

    for op in &node.style.transform {
        let result = transform::apply_transform_op(op, cur_origin, cur_spatial_id, wr);
        cur_origin = result.child_origin;
        cur_spatial_id = result.child_spatial_id;
        if result.pushed_frame {
            pushed_count += 1;
        }
    }

    (cur_origin, cur_spatial_id, pushed_count, accumulated_scale)
}

/// Render a shape node into the WebRender display list, handling filter
/// stacking contexts and mask clip chains.
fn render_shape(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    node_clip_chain: ClipChainId,
    mask_clips: &Option<Vec<ClipChainId>>,
    filter_ops: &Option<Vec<webrender_api::FilterOp>>,
    accumulated_scale: f32,
    paints: &dyn PaintResourceProvider,
    wr: &mut DisplayListBuilder,
) {
    let SvgTag::Shape(shape) = &node.tag else { return };
    if !node.style.is_visible() {
        return;
    }

    // Push a stacking context when filters are present.
    let pushed_filter = if let Some(ops) = filter_ops {
        wr.push_stacking_context(
            cur_spatial_id,
            PrimitiveFlags::default(),
            clip_chain_option(node_clip_chain),
            TransformStyle::Flat,
            MixBlendMode::Normal,
            ops,
            &[], // filter_datas
            RasterSpace::Screen,
            StackingContextFlags::empty(),
            None, // snapshot
        );
        true
    } else {
        false
    };

    // Determine the clip chain to use (mask clips or node clip).
    let effective_clip = if let Some(clips) = mask_clips {
        clips.first().copied().unwrap_or(node_clip_chain)
    } else {
        node_clip_chain
    };

    // Render shape once (with masks: once per mask shape for union).
    if let Some(clips) = mask_clips {
        for &mask_chain in clips {
            let mut ctx = RenderContext {
                style: &node.style,
                svg_origin: *cur_origin,
                spatial_id: cur_spatial_id,
                clip_chain_id: mask_chain,
                wr: &mut *wr,
                paints,
                accumulated_scale,
            };
            shape.render(&mut ctx);
        }
    } else {
        let mut ctx = RenderContext {
            style: &node.style,
            svg_origin: *cur_origin,
            spatial_id: cur_spatial_id,
            clip_chain_id: effective_clip,
            wr: &mut *wr,
            paints,
            accumulated_scale,
        };
        shape.render(&mut ctx);
    }

    // Pop the stacking context if we pushed one for filters.
    if pushed_filter {
        wr.pop_stacking_context();
    }
}

/// Recursively render child nodes.  Skips `<defs>` containers whose
/// children are non-visual definitions.
fn recurse_children(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    node_clip_chain: ClipChainId,
    parent_clip_chain: ClipChainId,
    paints: &impl PaintResourceProvider,
    clips: &impl ClipMaskProvider,
    filters: &impl FilterProvider,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
) {
    if let SvgTag::Container(Container::Defs) = &node.tag {
        return; // <defs> children are non-visual definitions.
    }

    let recurse_clip_chain = if node_clip_chain != parent_clip_chain {
        node_clip_chain
    } else {
        parent_clip_chain
    };

    for child in &node.children {
        render_node(
            child, cur_origin, cur_spatial_id, recurse_clip_chain, wr,
            paints, clips, filters, accumulated_scale,
        );
    }
}
