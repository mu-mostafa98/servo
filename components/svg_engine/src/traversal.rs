/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.

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

pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_chain = if !tree.viewport.overflow_visible {
        let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
        wr.define_clip_chain(clip_chain_option(clip_chain_id), [svg_clip_id])
    } else {
        clip_chain_id
    };

    let (root_origin, root_spatial_id, pop_frame) = if let Some(ref vb) = tree.viewport.view_box {
        let (sx, sy, ox, oy) = viewbox_transform(
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
        (LayoutPoint::new(0.0, 0.0), fid, true)
    } else {
        (*svg_origin, spatial_id, false)
    };

    let providers = RenderProviders {
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

/// Bundled resource providers for the rendering traversal.
/// Reduces [`render_node`] and [`recurse_children`] argument counts
/// from 9/10 to 7/8 (clippy: too_many_arguments).
struct RenderProviders<'a> {
    paints: &'a dyn PaintResourceProvider,
    clips: &'a dyn ClipMaskProvider,
    filters: &'a dyn FilterProvider,
}

fn render_node(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
    providers: &RenderProviders,
    parent_scale: f32,
) {
    if !node.style.is_displayed() {
        return;
    }
    let (cur_origin, cur_spatial_id, pushed_count, accumulated_scale) =
        apply_node_transforms(node, svg_origin, spatial_id, parent_scale, wr);
    let node_clip_chain = resolve_node_clip_path(
        node,
        providers.clips,
        &cur_origin,
        cur_spatial_id,
        clip_chain_id,
        wr,
    );
    let mask_clips = build_mask_clips(
        node,
        providers.clips,
        &cur_origin,
        cur_spatial_id,
        node_clip_chain,
        wr,
    );
    let filter_ops = get_filter_ops(node, providers.filters);
    let shape_params = ShapeRenderParams {
        mask_clips: &mask_clips,
        filter_ops: &filter_ops,
        paints: providers.paints,
    };
    render_shape(
        node,
        &cur_origin,
        cur_spatial_id,
        node_clip_chain,
        accumulated_scale,
        wr,
        &shape_params,
    );
    let child_clip_chain = if node_clip_chain != clip_chain_id {
        node_clip_chain
    } else {
        clip_chain_id
    };
    recurse_children(
        node,
        &cur_origin,
        cur_spatial_id,
        child_clip_chain,
        providers,
        accumulated_scale,
        wr,
    );
    for _ in 0..pushed_count {
        wr.pop_reference_frame();
    }
}

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

/// Bundled parameters for shape rendering.
/// Reduces [`render_shape`] argument count from 9 to 6 (clippy: too_many_arguments).
struct ShapeRenderParams<'a> {
    mask_clips: &'a Option<Vec<ClipChainId>>,
    filter_ops: &'a Option<Vec<webrender_api::FilterOp>>,
    paints: &'a dyn PaintResourceProvider,
}

fn render_shape(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    node_clip_chain: ClipChainId,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
    params: &ShapeRenderParams,
) {
    let SvgTag::Shape(shape) = &node.tag else {
        return;
    };
    if !node.style.is_visible() {
        return;
    }
    let pushed_filter = if let Some(ops) = params.filter_ops {
        wr.push_stacking_context(
            cur_spatial_id,
            PrimitiveFlags::default(),
            clip_chain_option(node_clip_chain),
            TransformStyle::Flat,
            MixBlendMode::Normal,
            ops,
            &[],
            RasterSpace::Screen,
            StackingContextFlags::empty(),
            None,
        );
        true
    } else {
        false
    };
    let effective_clip = if let Some(clips) = params.mask_clips {
        clips.first().copied().unwrap_or(node_clip_chain)
    } else {
        node_clip_chain
    };
    if let Some(clips) = params.mask_clips {
        for &mask_chain in clips {
            let mut ctx = RenderContext {
                style: &node.style,
                svg_origin: *cur_origin,
                spatial_id: cur_spatial_id,
                clip_chain_id: mask_chain,
                wr: &mut *wr,
                paints: params.paints,
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
            paints: params.paints,
            accumulated_scale,
        };
        shape.render(&mut ctx);
    }
    if pushed_filter {
        wr.pop_stacking_context();
    }
}

fn recurse_children(
    node: &SvgRenderNode,
    cur_origin: &LayoutPoint,
    cur_spatial_id: SpatialId,
    clip_chain: ClipChainId,
    providers: &RenderProviders,
    accumulated_scale: f32,
    wr: &mut DisplayListBuilder,
) {
    if let SvgTag::Container(Container::Defs) = &node.tag {
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
        );
    }
}

/// viewBox → viewport transform. None = non-uniform stretch (backward compat).
fn viewbox_transform(
    vb_w: f32,
    vb_h: f32,
    vp_w: f32,
    vp_h: f32,
    ar: Option<&crate::render_tree::AspectRatio>,
) -> (f32, f32, f32, f32) {
    if vb_w <= 0.0 || vb_h <= 0.0 {
        return (1.0, 1.0, 0.0, 0.0);
    }
    let Some(ar) = ar else {
        return (vp_w / vb_w, vp_h / vb_h, 0.0, 0.0);
    };
    use crate::render_tree::{AspectAlign, MeetOrSlice};
    match ar.align {
        AspectAlign::None => (vp_w / vb_w, vp_h / vb_h, 0.0, 0.0),
        _ => {
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
        },
    }
}
