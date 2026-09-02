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
use crate::renderer::path::rasterize_bez;
use crate::RasterizedImage;

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
) -> Vec<RasterizedImage> {
    let svg_clip_chain =
        build_viewport_clip(tree, svg_origin, svg_size, spatial_id, clip_chain_id, wr);

    let (root_origin, root_spatial_id, pop_frame, viewbox) =
        push_viewbox_frame(tree, svg_origin, svg_size, spatial_id, wr);

    let providers = ResourceProviders {
        paints: tree,
        clips: tree,
        filters: tree,
    };
    let viewbox_scale = viewbox
        .as_ref()
        .map(|v| (v.sx, v.sy))
        .unwrap_or((1.0, 1.0));
    // Fold the root viewBox translation into the raster offset up front. The
    // scale is applied during rasterization (via `viewbox_scale`), and the
    // document-space `svg_origin` is added back when the rasters are finalized
    // below. Keeping the full translation in `raster_offset` also lets the
    // traversal detect a non-identity viewBox (to rasterize native shapes).
    let root_raster_offset = viewbox
        .as_ref()
        .map(|v| LayoutPoint::new(v.ox - v.min_x * v.sx, v.oy - v.min_y * v.sy))
        .unwrap_or(LayoutPoint::zero());
    let mut rasters: Vec<RasterizedImage> = Vec::new();
    render_node(
        &tree.root,
        &root_origin,
        root_spatial_id,
        svg_clip_chain,
        wr,
        &providers,
        1.0,
        viewbox_scale,
        root_raster_offset,
        None,
        Transform2D::<f32, (), ()>::identity(),
        &mut rasters,
    );

    if pop_frame {
        wr.pop_reference_frame();
    }

    // Rasterized images bypass the viewBox reference frame, so add back the
    // document-space origin. The viewBox translation was already folded into
    // `raster_offset` and the scale into `viewbox_scale` during rasterization.
    for raster in &mut rasters {
        raster.x = svg_origin.x + raster.x;
        raster.y = svg_origin.y + raster.y;
    }

    rasters
}

/// The resolved viewBox → viewport transform.
struct ViewboxTransform {
    sx: f32,
    sy: f32,
    ox: f32,
    oy: f32,
    min_x: f32,
    min_y: f32,
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
/// Returns `(new_origin, new_spatial_id, should_pop, viewbox_transform)`.
fn push_viewbox_frame(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    wr: &mut DisplayListBuilder,
) -> (LayoutPoint, SpatialId, bool, Option<ViewboxTransform>) {
    let Some(ref vb) = tree.viewport.view_box else {
        return (*svg_origin, spatial_id, false, None);
    };
    let (sx, sy, ox, oy) = compute_viewbox_transform(
        vb.width,
        vb.height,
        svg_size.width,
        svg_size.height,
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
    (
        LayoutPoint::new(0.0, 0.0),
        fid,
        true,
        Some(ViewboxTransform {
            sx,
            sy,
            ox,
            oy,
            min_x: vb.min_x,
            min_y: vb.min_y,
        }),
    )
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
    viewbox_scale: (f32, f32),
    mut raster_offset: LayoutPoint,
    mut clip_rect: Option<LayoutRect>,
    mut node_xform: Transform2D<f32, (), ()>,
    rasters: &mut Vec<RasterizedImage>,
) {
    if !node.style.is_displayed() {
        return;
    }

    // Step 1 — Apply transforms.
    let (mut cur_origin, mut cur_spatial_id, mut pushed_count, accumulated_scale) =
        apply_node_transforms(node, svg_origin, spatial_id, parent_scale, wr);

    // Fold the node's own transform into the raster-space affine so the
    // CPU-rasterized path can apply it (it bypasses the reference frame).
    // `a.then(b) == b * a`, so compose as `node_xform * node_matrix`.
    node_xform = transform::compute_transform_matrix(&node.transforms).then(&node_xform);

    // Step 2 — Establish a nested viewport for nested `<svg>` elements. When a
    // viewBox is present, push the viewBox → viewport reference frame (native
    // shapes) and fold its translation into `raster_offset` (CPU-rasterized
    // shapes, which bypass reference frames).
    let cur_clip_chain = clip_chain_id;
    let mut cur_viewbox_scale = viewbox_scale;
    if let Some(vp) = &node.viewport {
        // Sub-viewport clip: a nested `<svg>` clips its content to its own
        // viewport (unless `overflow: visible`). The clip rect is expressed in
        // the SVG-local space tracked by `raster_offset`/`cur_viewbox_scale`,
        // so it can be intersected with any inherited clip and later applied
        // to the CPU-rasterized shapes.
        if !vp.overflow_visible {
            let clip_origin = LayoutPoint::new(
                raster_offset.x + cur_viewbox_scale.0 * vp.x,
                raster_offset.y + cur_viewbox_scale.1 * vp.y,
            );
            let clip_size = LayoutSize::new(
                cur_viewbox_scale.0 * vp.width,
                cur_viewbox_scale.1 * vp.height,
            );
            let sub_clip = LayoutRect::from_origin_and_size(clip_origin, clip_size);
            clip_rect = match clip_rect {
                Some(existing) => Some(existing.intersection(&sub_clip).unwrap_or_else(|| {
                    // Non-overlapping viewports: clip everything (zero-size).
                    LayoutRect::from_origin_and_size(existing.min, LayoutSize::new(0.0, 0.0))
                })),
                None => Some(sub_clip),
            };
        }

        let vp_origin = LayoutPoint::new(cur_origin.x + vp.x, cur_origin.y + vp.y);

        if let Some(vb) = &vp.view_box {
            let (sx, sy, ox, oy) = compute_viewbox_transform(
                vb.width,
                vb.height,
                vp.width,
                vp.height,
                vp.aspect_ratio.as_ref(),
            );

            // Translation that maps viewBox space → parent space:
            //   p -> scale(p - min) + offset, then + (x, y)
            //   = scale*p + (x + offset - scale*min)
            let tx = vp.x + ox - vb.min_x * sx;
            let ty = vp.y + oy - vb.min_y * sy;
            raster_offset = LayoutPoint::new(
                raster_offset.x + cur_viewbox_scale.0 * tx,
                raster_offset.y + cur_viewbox_scale.1 * ty,
            );

            let t1 = Transform2D::<f32, (), ()>::translation(-vb.min_x, -vb.min_y);
            let s = Transform2D::<f32, (), ()>::scale(sx, sy);
            let t2 = Transform2D::<f32, (), ()>::translation(ox, oy);
            let combined = t1.then(&s).then(&t2);
            let lt = transform::to_layout_transform(&combined);
            cur_spatial_id = wr.push_reference_frame(
                vp_origin,
                cur_spatial_id,
                TransformStyle::Flat,
                PropertyBinding::Value(lt),
                ReferenceFrameKind::Transform {
                    is_2d_scale_translation: false,
                    should_snap: false,
                    paired_with_perspective: false,
                },
            );
            cur_origin = LayoutPoint::new(0.0, 0.0);
            pushed_count += 1;
            cur_viewbox_scale = (cur_viewbox_scale.0 * sx, cur_viewbox_scale.1 * sy);
        }
    }

    // Step 3 — Resolve effects (clip-path, mask, filter).
    let resolved = resolve_node_effects(
        node,
        providers,
        &cur_origin,
        cur_spatial_id,
        cur_clip_chain,
        wr,
    );

    // Step 4 — Render the element.
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
        cur_viewbox_scale,
        raster_offset,
        clip_rect,
        node_xform,
        rasters,
    );

    // Step 5 — Recurse into children.
    recurse_children(
        node,
        &cur_origin,
        cur_spatial_id,
        resolved.clip_chain,
        providers,
        accumulated_scale,
        wr,
        cur_viewbox_scale,
        raster_offset,
        clip_rect,
        node_xform,
        rasters,
    );

    // Step 6 — Pop transform reference frames.
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
        node,
        providers.clips,
        cur_origin,
        cur_spatial_id,
        parent_clip_chain,
        wr,
    );
    let mask_clips = build_mask_clips(
        node,
        providers.clips,
        cur_origin,
        cur_spatial_id,
        node_clip_chain,
        wr,
    );
    let filter_ops = get_filter_ops(node, providers.filters);

    ResolvedEffects {
        clip_chain: if node_clip_chain != parent_clip_chain {
            node_clip_chain
        } else {
            parent_clip_chain
        },
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
    viewbox_scale: (f32, f32),
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    node_xform: Transform2D<f32, (), ()>,
    rasters: &mut Vec<RasterizedImage>,
) {
    match &node.tag {
        SvgTag::Shape(shape) => emit_geometry(
            shape,
            node.style.clone(),
            cur_origin,
            cur_spatial_id,
            node_clip_chain,
            accumulated_scale,
            wr,
            params,
            viewbox_scale,
            raster_offset,
            clip_rect,
            node_xform,
            rasters,
        ),
        SvgTag::Text(text) => emit_leaf(
            text,
            node,
            cur_origin,
            cur_spatial_id,
            node_clip_chain,
            wr,
            params,
            viewbox_scale,
            raster_offset,
            rasters,
        ),
        SvgTag::Image(img) => emit_leaf(
            img,
            node,
            cur_origin,
            cur_spatial_id,
            node_clip_chain,
            wr,
            params,
            viewbox_scale,
            raster_offset,
            rasters,
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
    viewbox_scale: (f32, f32),
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    node_xform: Transform2D<f32, (), ()>,
    rasters: &mut Vec<RasterizedImage>,
) {
    if !style.is_visible() {
        return;
    }

    let pushed_filter = push_filter_context(params.filter_ops, cur_spatial_id, node_clip_chain, wr);

    let effective_clip = params
        .mask_clips
        .as_ref()
        .and_then(|c| c.first().copied())
        .unwrap_or(node_clip_chain);

    if let Some(clips) = params.mask_clips {
        for &mask_chain in clips {
            emit_shape(
                shape,
                &style,
                cur_origin,
                cur_spatial_id,
                mask_chain,
                accumulated_scale,
                params.paints,
                wr,
                viewbox_scale,
                raster_offset,
                clip_rect,
                node_xform,
                rasters,
            );
        }
    } else {
        emit_shape(
            shape,
            &style,
            cur_origin,
            cur_spatial_id,
            effective_clip,
            accumulated_scale,
            params.paints,
            wr,
            viewbox_scale,
            raster_offset,
            clip_rect,
            node_xform,
            rasters,
        );
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
    viewbox_scale: (f32, f32),
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    node_xform: Transform2D<f32, (), ()>,
    rasters: &mut Vec<RasterizedImage>,
) {
    // All painted shapes are rasterized via vello_cpu into one ordered list.
    // Keeping every shape in that single list preserves z-order between
    // transformed and untransformed shapes (native WebRender primitives are
    // emitted inline, so mixing them with the deferred rasterized images would
    // break paint order), and gives fill/stroke/dash and gradient support
    // uniformly — the native primitives don't handle dashed strokes.
    let has_paint = style.fill.is_some() || style.stroke.is_some();
    if has_paint {
        if let Some(bez) = shape.to_bez_path() {
            // The node transform (translate/scale/rotate/…) is applied to the
            // path inside `rasterize_bez`; `raster_offset` carries only the
            // viewBox translation, and the document-space origin is added back
            // when the rasters are finalized in `render_svg_tree`.
            let raster_origin = raster_offset;
            rasterize_bez(
                &bez,
                style.fill.as_ref(),
                style.stroke.as_ref(),
                style.opacity,
                &raster_origin,
                viewbox_scale,
                node_xform,
                clip_rect,
                paints,
                rasters,
            );
            return;
        }
    }

    let mut ctx = RenderContext {
        style,
        svg_origin: *svg_origin,
        spatial_id,
        clip_chain_id,
        wr: &mut *wr,
        paints,
        accumulated_scale,
        viewbox_scale,
        raster_offset,
        native_rendering: false,
        rasters,
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
        spatial_id,
        PrimitiveFlags::default(),
        clip_chain_option(clip_chain_id),
        TransformStyle::Flat,
        MixBlendMode::Normal,
        ops,
        &[],
        RasterSpace::Screen,
        StackingContextFlags::empty(),
        None,
    );
    true
}

// ======================= Non-Geometric Rendering =======================

/// Emit a leaf element (text, image) with filter and mask support.
fn emit_leaf<T: crate::renderer::Render>(
    item: &T,
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
    params: &EffectParams,
    viewbox_scale: (f32, f32),
    raster_offset: LayoutPoint,
    rasters: &mut Vec<RasterizedImage>,
) {
    if !node.style.is_visible() {
        return;
    }

    // Apply filter stacking context if filter ops are present.
    let pushed_filter = push_filter_context(params.filter_ops, cur_spatial_id, clip_chain_id, wr);

    let effective_clip = params
        .mask_clips
        .as_ref()
        .and_then(|c| c.first().copied())
        .unwrap_or(clip_chain_id);

    let mut ctx = RenderContext {
        style: &node.style,
        svg_origin: *cur_origin,
        spatial_id: cur_spatial_id,
        clip_chain_id: effective_clip,
        wr: &mut *wr,
        paints: params.paints,
        accumulated_scale: 1.0,
        viewbox_scale,
        raster_offset,
        native_rendering: false,
        rasters,
    };
    item.render(&mut ctx);

    if pushed_filter {
        wr.pop_stacking_context();
    }
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
    viewbox_scale: (f32, f32),
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    node_xform: Transform2D<f32, (), ()>,
    rasters: &mut Vec<RasterizedImage>,
) {
    // <defs> and <symbol> children are only rendered when referenced
    // via <use>, never directly during tree traversal.
    if matches!(&node.tag, SvgTag::Container(Container::Defs | Container::Symbol)) {
        return;
    }
    for child in &node.children {
        render_node(
            child,
            cur_origin,
            cur_spatial_id,
            clip_chain,
            wr,
            providers,
            accumulated_scale,
            viewbox_scale,
            raster_offset,
            clip_rect,
            node_xform,
            rasters,
        );
    }
}

// ======================= ViewBox Math =======================

/// Compute the viewBox → viewport scale and translation.
/// Returns `(scale_x, scale_y, offset_x, offset_y)`.
pub(crate) fn compute_viewbox_transform(
    vb_w: f32,
    vb_h: f32,
    vp_w: f32,
    vp_h: f32,
    ar: Option<&AspectRatio>,
) -> (f32, f32, f32, f32) {
    if vb_w <= 0.0 || vb_h <= 0.0 {
        return (1.0, 1.0, 0.0, 0.0);
    }
    // No `preserveAspectRatio` specified → SVG spec default `xMidYMid meet`.
    // (Explicit `preserveAspectRatio="none"` is represented as
    // `Some(AspectRatio { align: AspectAlign::None, .. })`, which is distinct.)
    let ar = ar.copied().unwrap_or_default();
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
