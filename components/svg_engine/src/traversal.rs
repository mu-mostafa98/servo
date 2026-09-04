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
    ClipMaskProvider, FilterProvider, MarkerProvider, PaintResourceProvider, Render, RenderContext,
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
    device_scale: f32,
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
        markers: tree,
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
        device_scale,
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
    markers: &'a dyn MarkerProvider,
}

/// Bundled effect parameters — reduces argument count for `emit_geometry`.
struct EffectParams<'a> {
    mask_clips: &'a Option<Vec<ClipChainId>>,
    filter_ops: &'a Option<Vec<webrender_api::FilterOp>>,
    paints: &'a dyn PaintResourceProvider,
    markers: &'a dyn MarkerProvider,
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
    device_scale: f32,
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
        markers: providers.markers,
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
        device_scale,
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
        device_scale,
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
    device_scale: f32,
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
            device_scale,
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
            device_scale,
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
            device_scale,
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
    device_scale: f32,
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
                params.markers,
                wr,
                viewbox_scale,
                device_scale,
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
            params.markers,
            wr,
            viewbox_scale,
            device_scale,
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
    markers: &dyn MarkerProvider,
    wr: &mut DisplayListBuilder,
    viewbox_scale: (f32, f32),
    device_scale: f32,
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
    // Pattern fills/strokes can't be rasterized by vello_cpu (it only handles
    // solid colors and gradients), so route them through the native renderer,
    // which tiles the pattern via `fill_rect_with_pattern_by_id`.
    let has_pattern = style_has_pattern(style);

    if has_paint && !has_pattern {
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
                device_scale,
                node_xform,
                clip_rect,
                paints,
                rasters,
            );
        }
    }

    // Markers are drawn on top of the shape (paint-order default: fill → stroke
    // → markers).
    emit_markers(
        shape,
        style,
        markers,
        node_xform,
        viewbox_scale,
        device_scale,
        raster_offset,
        clip_rect,
        paints,
        rasters,
    );

    if !has_paint || has_pattern {
        // Native path — used for pattern paint servers (which vello_cpu can't
        // rasterize) and as a no-op fallback for shapes with neither paint nor
        // a bez path.
        let mut ctx = RenderContext {
            style,
            svg_origin: *svg_origin,
            spatial_id,
            clip_chain_id,
            wr: &mut *wr,
            paints,
            accumulated_scale,
            viewbox_scale,
            device_scale,
            raster_offset,
            native_rendering: false,
            rasters,
        };
        shape.render(&mut ctx);
    }
}

/// Whether the style uses a `<pattern>` paint server for its fill or stroke.
fn style_has_pattern(style: &crate::style::NodeStyle) -> bool {
    use crate::style::gradient::PaintServer;
    let fill_pattern = style
        .fill
        .as_ref()
        .and_then(|f| f.paint_server.as_ref())
        .is_some_and(|p| matches!(p, PaintServer::Pattern(_)));
    let stroke_pattern = style
        .stroke
        .as_ref()
        .and_then(|s| s.paint_server.as_ref())
        .is_some_and(|p| matches!(p, PaintServer::Pattern(_)));
    fill_pattern || stroke_pattern
}

// ======================= Marker Rendering =======================

/// Compute the vertices of a marker-bearing shape (`line`/`polyline`/`polygon`/
/// `path`), in the shape's local coordinate space. Returns `None` for shapes
/// that don't carry markers (rect/circle/ellipse).
fn shape_vertices(shape: &crate::shapes::Shape) -> Option<Vec<(f32, f32)>> {
    use crate::shapes::Shape;
    match shape {
        Shape::Rect(_) | Shape::Circle(_) | Shape::Ellipse(_) => None,
        _ => {
            let bez = shape.to_bez_path()?;
            let mut vertices = Vec::new();
            for el in bez.elements() {
                match el {
                    kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                        vertices.push((p.x as f32, p.y as f32));
                    },
                    kurbo::PathEl::QuadTo(_, p) => vertices.push((p.x as f32, p.y as f32)),
                    kurbo::PathEl::CurveTo(_, _, p) => vertices.push((p.x as f32, p.y as f32)),
                    kurbo::PathEl::ClosePath => {},
                }
            }
            if vertices.len() >= 2 {
                Some(vertices)
            } else {
                None
            }
        },
    }
}

/// Render the start/mid/end markers for a shape (on top of its fill/stroke).
#[allow(clippy::too_many_arguments)]
fn emit_markers(
    shape: &crate::shapes::Shape,
    style: &crate::style::NodeStyle,
    markers: &dyn MarkerProvider,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    paints: &dyn PaintResourceProvider,
    rasters: &mut Vec<RasterizedImage>,
) {
    let Some(refs) = &style.markers else { return };
    let Some(vertices) = shape_vertices(shape) else { return };
    let n = vertices.len();

    let stroke_width = style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0);

    if let Some(id) = &refs.start {
        let (x, y) = vertices[0];
        let (nx, ny) = vertices[1];
        emit_marker(
            id, x, y, nx - x, ny - y, true, markers, stroke_width,
            node_xform, viewbox_scale, device_scale, raster_offset, clip_rect, paints, rasters,
        );
    }
    if let Some(id) = &refs.mid {
        for i in 1..n - 1 {
            let (x, y) = vertices[i];
            let (nx, ny) = vertices[i + 1];
            emit_marker(
                id, x, y, nx - x, ny - y, false, markers, stroke_width,
                node_xform, viewbox_scale, device_scale, raster_offset, clip_rect, paints, rasters,
            );
        }
    }
    if let Some(id) = &refs.end {
        let (x, y) = vertices[n - 1];
        let (px, py) = vertices[n - 2];
        emit_marker(
            id, x, y, x - px, y - py, false, markers, stroke_width,
            node_xform, viewbox_scale, device_scale, raster_offset, clip_rect, paints, rasters,
        );
    }
}

/// Place a single marker at `(x, y)`, oriented along `(tangent_x, tangent_y)`.
#[allow(clippy::too_many_arguments)]
fn emit_marker(
    id: &str,
    x: f32,
    y: f32,
    tangent_x: f32,
    tangent_y: f32,
    is_start: bool,
    markers: &dyn MarkerProvider,
    stroke_width: f32,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    raster_offset: LayoutPoint,
    clip_rect: Option<LayoutRect>,
    paints: &dyn PaintResourceProvider,
    rasters: &mut Vec<RasterizedImage>,
) {
    let Some(def) = markers.marker(id) else { return };

    let unit_factor = match def.marker_units {
        MarkerUnits::StrokeWidth => stroke_width,
        MarkerUnits::UserSpaceOnUse => 1.0,
    };
    let (vb_w, vb_h) = match &def.view_box {
        Some(vb) => (vb.width, vb.height),
        None => (def.marker_width, def.marker_height),
    };
    if vb_w <= 0.0 || vb_h <= 0.0 {
        return;
    }
    let scale_x = def.marker_width * unit_factor / vb_w;
    let scale_y = def.marker_height * unit_factor / vb_h;

    let angle = match &def.orient {
        MarkerOrient::Auto => tangent_angle_deg(tangent_x, tangent_y),
        MarkerOrient::AutoStartReverse => {
            let a = tangent_angle_deg(tangent_x, tangent_y);
            if is_start {
                a + 180.0
            } else {
                a
            }
        },
        MarkerOrient::Angle(a) => *a,
    };

    // marker viewBox coords → shape local coords:
    //   T(vertex) · R(angle) · S(sx,sy) · T(-refX,-refY)
    let marker_local: Transform2D<f32, (), ()> =
        Transform2D::<f32, (), ()>::translation(-def.ref_x, -def.ref_y)
            .then(&Transform2D::<f32, (), ()>::scale(scale_x, scale_y))
            .then(&rotation_matrix(angle))
            .then(&Transform2D::<f32, (), ()>::translation(x, y));

    // Compose with the shape's accumulated node transform.
    let full_xform = marker_local.then(&node_xform);

    for (m_shape, m_style) in &def.shapes {
        if !m_style.is_visible() {
            continue;
        }
        let Some(bez) = m_shape.to_bez_path() else { continue };
        rasterize_bez(
            &bez,
            m_style.fill.as_ref(),
            m_style.stroke.as_ref(),
            m_style.opacity,
            &raster_offset,
            viewbox_scale,
            device_scale,
            full_xform,
            clip_rect,
            paints,
            rasters,
        );
    }
}

/// Angle (degrees) of a direction vector, falling back to 0 for a zero vector.
fn tangent_angle_deg(dx: f32, dy: f32) -> f32 {
    dy.atan2(dx).to_degrees()
}

/// Clockwise rotation matrix (SVG y-down) by `angle_deg` degrees.
fn rotation_matrix(angle_deg: f32) -> Transform2D<f32, (), ()> {
    let (s, c) = angle_deg.to_radians().sin_cos();
    Transform2D::new(c, s, -s, c, 0.0, 0.0)
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
    device_scale: f32,
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
        device_scale,
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
    device_scale: f32,
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
            device_scale,
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
