/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.
//!
//! Walks the [`SvgRenderTree`] recursively, applying transforms, clip-paths,
//! masks, and filters at each node, then dispatching to shape/text/image
//! renderers that emit WebRender display list commands.

use euclid::Transform2D;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    ClipChainId, DisplayListBuilder, MixBlendMode, PrimitiveFlags, PropertyBinding, RasterSpace,
    ReferenceFrameKind, SpatialId, StackingContextFlags, TransformStyle,
};

use crate::effects::clip::{build_mask_clips, resolve_node_clip_path};
use crate::effects::filter::get_filter_ops;
use crate::render_tree::*;
use crate::renderer::{
    ClipMaskProvider, FilterProvider, PaintResourceProvider, Render, RenderContext,
    clip_chain_option, transform,
};

// ======================= Public Entry Point =======================

/// Render an entire SVG tree into a WebRender display list.
///
/// 1. Set up the SVG viewport clip (unless `overflow: visible`).
/// 2. Push a viewBox reference frame (if a viewBox is defined).
/// 3. Walk the tree starting from the root node.
pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let svg_clip_chain = build_viewport_clip(tree, svg_origin, svg_size, spatial_id, clip_chain_id, wr);

    let (root_origin, root_spatial_id, pop_frame) =
        push_viewbox_frame(tree, svg_origin, svg_size, spatial_id, wr);

    let providers = ResourceProviders {
        paints: tree,
        clips: tree,
        filters: tree,
    };
    render_node(
        &tree.root,
        &root_origin,
        root_spatial_id,
        svg_clip_chain,
        wr,
        &providers,
        1.0,
    );

    if pop_frame {
        wr.pop_reference_frame();
    }
}

// ======================= Viewport Setup =======================

/// Build a clip chain that confines rendering to the SVG viewport bounds.
/// Skipped when the SVG element has `overflow: visible`.
fn build_viewport_clip(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> ClipChainId {
    if tree.viewport.overflow_visible {
        return clip_chain_id;
    }
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    wr.define_clip_chain(clip_chain_option(clip_chain_id), [svg_clip_id])
}

/// Push a reference frame that implements the viewBox → viewport transform.
/// Returns `(new_origin, new_spatial_id, should_pop)`.
fn push_viewbox_frame(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    wr: &mut DisplayListBuilder,
) -> (LayoutPoint, SpatialId, bool) {
    let Some(ref vb) = tree.viewport.view_box else {
        return (*svg_origin, spatial_id, false);
    };
    let (sx, sy, ox, oy) = compute_viewbox_transform(
        vb.width, vb.height, svg_size.width, svg_size.height,
        tree.viewport.aspect_ratio.as_ref(),
    );
    let t1 = Transform2D::<f32, (), ()>::translation(-vb.min_x, -vb.min_y);
    let s = Transform2D::<f32, (), ()>::scale(sx, sy);
    let t2 = Transform2D::<f32, (), ()>::translation(ox, oy);
    let combined = t1.then(&s).then(&t2);
    let lt = transform::to_layout_transform(&combined);
    let fid = wr.push_reference_frame(
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
    (LayoutPoint::new(0.0, 0.0), fid, true)
}

// ======================= Bundled Parameter Structs =======================

/// Bundled resource providers — reduces argument count for recursive functions.
struct ResourceProviders<'a> {
    paints: &'a dyn PaintResourceProvider,
    clips: &'a dyn ClipMaskProvider,
    filters: &'a dyn FilterProvider,
}

/// Bundled effect parameters — reduces argument count for `emit_geometry`.
struct EffectParams<'a> {
    mask_clips: &'a Option<Vec<ClipChainId>>,
    filter_ops: &'a Option<Vec<webrender_api::FilterOp>>,
    paints: &'a dyn PaintResourceProvider,
}

// ======================= Tree Traversal =======================

/// Render a single node and recurse into its children.
///
/// For each node:
/// 1. Skip if `display: none`.
/// 2. Apply SVG transforms (translate, scale, rotate, …).
/// 3. Resolve and apply clip-path, mask, and filter effects.
/// 4. Render the shape/text/image.
/// 5. Recurse into children with the updated clip chain.
fn render_node(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
    providers: &ResourceProviders,
    parent_scale: f32,
) {
    if !node.style.is_displayed() {
        return;
    }

    // Step 1 — Apply transforms.
    let (cur_origin, cur_spatial_id, pushed_count, accumulated_scale) =
        apply_node_transforms(node, svg_origin, spatial_id, parent_scale, wr);

    // Step 2 — Resolve effects (clip-path, mask, filter).
    let resolved = resolve_node_effects(node, providers, &cur_origin, cur_spatial_id, clip_chain_id, wr);

    // Step 3 — Render the element.
    let shape_params = EffectParams {
        mask_clips: &resolved.mask_clips,
        filter_ops: &resolved.filter_ops,
        paints: providers.paints,
    };
    emit_element(
        node,
        &cur_origin,
        cur_spatial_id,
        resolved.clip_chain,
        accumulated_scale,
        wr,
        &shape_params,
    );

    // Step 4 — Recurse into children.
    recurse_children(node, &cur_origin, cur_spatial_id, resolved.clip_chain, providers, accumulated_scale, wr);

    // Step 5 — Pop transform reference frames.
    for _ in 0..pushed_count {
        wr.pop_reference_frame();
    }
}

/// The resolved effects for a node — clip chain, mask clips, filter ops.
struct ResolvedEffects {
    clip_chain: ClipChainId,
    mask_clips: Option<Vec<ClipChainId>>,
    filter_ops: Option<Vec<webrender_api::FilterOp>>,
}

/// Resolve clip-path, mask, and filter effects for a node.
fn resolve_node_effects(
    node: &SvgRenderNode,
    providers: &ResourceProviders,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> ResolvedEffects {
    let node_clip_chain = resolve_node_clip_path(
        node, providers.clips, cur_origin, cur_spatial_id, parent_clip_chain, wr,
    );
    let mask_clips = build_mask_clips(
        node, providers.clips, cur_origin, cur_spatial_id, node_clip_chain, wr,
    );
    let filter_ops = get_filter_ops(node, providers.filters);

    ResolvedEffects {
        clip_chain: if node_clip_chain != parent_clip_chain { node_clip_chain } else { parent_clip_chain },
        mask_clips,
        filter_ops,
    }
}

// ======================= Transforms =======================

/// Apply all [`TransformOp`]s on a node, pushing WebRender reference frames.
///
/// Returns the new origin, spatial ID, number of pushed frames (for later popping),
/// and the accumulated scale (for `vector-effect: non-scaling-stroke`).
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
    let node_scale = transform::compute_transform_scale(&node.transforms);
    let accumulated_scale = parent_scale * node_scale;
    for op in &node.transforms {
        let result = transform::apply_transform_op(op, cur_origin, cur_spatial_id, wr);
        cur_origin = result.child_origin;
        cur_spatial_id = result.child_spatial_id;
        if result.pushed_frame {
            pushed_count += 1;
        }
    }
    (cur_origin, cur_spatial_id, pushed_count, accumulated_scale)
}

// ======================= Shape Rendering =======================

/// Dispatch rendering to the correct path based on the node's [`SvgTag`].
fn emit_element(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    node_clip_chain: ClipChainId,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
    params: &EffectParams,
) {
    match &node.tag {
        SvgTag::Shape(shape) => emit_geometry(
            shape, node.style.clone(), cur_origin, cur_spatial_id,
            node_clip_chain, accumulated_scale, wr, params,
        ),
        SvgTag::Text(text) => emit_leaf(
            text, node, cur_origin, cur_spatial_id, node_clip_chain, wr,
        ),
        SvgTag::Image(img) => emit_leaf(
            img, node, cur_origin, cur_spatial_id, node_clip_chain, wr,
        ),
        SvgTag::Container(_) => {},
    }
}

/// Render a geometric shape with full clip-path, mask, and filter support.
fn emit_geometry(
    shape: &crate::shapes::Shape,
    style: crate::style::NodeStyle,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    node_clip_chain: ClipChainId,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
    params: &EffectParams,
) {
    if !style.is_visible() { return; }

    let pushed_filter = push_filter_context(params.filter_ops, cur_spatial_id, node_clip_chain, wr);

    let effective_clip = params.mask_clips.as_ref()
        .and_then(|c| c.first().copied())
        .unwrap_or(node_clip_chain);

    if let Some(clips) = params.mask_clips {
        for &mask_chain in clips {
            emit_shape(shape, &style, cur_origin, cur_spatial_id, mask_chain, accumulated_scale, params.paints, wr);
        }
    } else {
        emit_shape(shape, &style, cur_origin, cur_spatial_id, effective_clip, accumulated_scale, params.paints, wr);
    }

    if pushed_filter {
        wr.pop_stacking_context();
    }
}

/// Emit a single render call for the shape (or one of its mask-clipped copies).
fn emit_shape(
    shape: &crate::shapes::Shape,
    style: &crate::style::NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    accumulated_scale: f32,
    paints: &dyn PaintResourceProvider,
    wr: &mut DisplayListBuilder,
) {
    let mut ctx = RenderContext {
        style,
        svg_origin: *svg_origin,
        spatial_id,
        clip_chain_id,
        wr: &mut *wr,
        paints,
        accumulated_scale,
    };
    shape.render(&mut ctx);
}

/// Push a filter stacking context. Returns `true` if one was pushed (caller must pop).
fn push_filter_context(
    filter_ops: &Option<Vec<webrender_api::FilterOp>>,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> bool {
    let Some(ops) = filter_ops else { return false };
    wr.push_stacking_context(
        spatial_id, PrimitiveFlags::default(),
        clip_chain_option(clip_chain_id), TransformStyle::Flat,
        MixBlendMode::Normal, ops, &[], RasterSpace::Screen,
        StackingContextFlags::empty(), None,
    );
    true
}

// ======================= Non-Geometric Rendering =======================

/// A no-op paint resource provider for non-geometric elements (text, image)
/// that don't reference gradients or patterns.
struct NoPaintResources;

impl PaintResourceProvider for NoPaintResources {
    fn gradient(&self, _id: &str) -> Option<&crate::style::gradient::GradientDef> { None }
    fn pattern(&self, _id: &str) -> Option<&crate::render_tree::PatternDef> { None }
}

/// Emit a leaf element (text, image) — no effects pipeline, no children.
fn emit_leaf<T: crate::renderer::Render>(
    item: &T,
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    if !node.style.is_visible() { return; }
    let mut ctx = RenderContext {
        style: &node.style, svg_origin: *cur_origin,
        spatial_id: cur_spatial_id, clip_chain_id,
        wr: &mut *wr, paints: &NoPaintResources, accumulated_scale: 1.0,
    };
    item.render(&mut ctx);
}

// ======================= Child Traversal =======================

/// Recurse into a node's children, skipping `<defs>` containers.
fn recurse_children(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    clip_chain: ClipChainId,
    providers: &ResourceProviders,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
) {
    if let SvgTag::Container(Container::Defs) = &node.tag {
        return;
    }
    for child in &node.children {
        render_node(child, cur_origin, cur_spatial_id, clip_chain, wr, providers, accumulated_scale);
    }
}

// ======================= ViewBox Math =======================

/// Compute the viewBox → viewport scale and translation.
/// Returns `(scale_x, scale_y, offset_x, offset_y)`.
fn compute_viewbox_transform(
    vb_w: f32, vb_h: f32, vp_w: f32, vp_h: f32,
    ar: Option<&AspectRatio>,
) -> (f32, f32, f32, f32) {
    if vb_w <= 0.0 || vb_h <= 0.0 {
        return (1.0, 1.0, 0.0, 0.0);
    }
    let Some(ar) = ar else {
        return (vp_w / vb_w, vp_h / vb_h, 0.0, 0.0);
    };
    if matches!(ar.align, AspectAlign::None) {
        return (vp_w / vb_w, vp_h / vb_h, 0.0, 0.0);
    }
    let s = match ar.meet_or_slice {
        MeetOrSlice::Meet => (vp_w / vb_w).min(vp_h / vb_h),
        MeetOrSlice::Slice => (vp_w / vb_w).max(vp_h / vb_h),
    };
    let ex = vp_w - vb_w * s;
    let ey = vp_h - vb_h * s;
    let ax = match ar.align {
        AspectAlign::XMinYMin | AspectAlign::XMinYMid | AspectAlign::XMinYMax => 0.0,
        AspectAlign::XMidYMin | AspectAlign::XMidYMid | AspectAlign::XMidYMax => 0.5,
        _ => 1.0,
    };
    let ay = match ar.align {
        AspectAlign::XMinYMin | AspectAlign::XMidYMin | AspectAlign::XMaxYMin => 0.0,
        AspectAlign::XMinYMid | AspectAlign::XMidYMid | AspectAlign::XMaxYMid => 0.5,
        _ => 1.0,
    };
    (s, s, ex * ax, ey * ay)
}
