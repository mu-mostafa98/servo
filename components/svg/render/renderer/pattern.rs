/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG pattern fill rendering.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use euclid::Transform2D;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform};
use webrender_api::{
    AlphaType, ClipChainId, ColorF, CommonItemProperties, ImageRendering, PrimitiveFlags,
    PropertyBinding, ReferenceFrameKind, SpatialId, TransformStyle,
};

use crate::model::document::{PatternContentUnits, PatternDef, PatternUnits};
use crate::render::effects::group::{GroupLeaf, collect_leaves, prepare_leaves, render_leaves};
use crate::render::renderer::{
    MAX_PAINT_SERVER_DEPTH, Render, RenderContext, clip_chain_option, transform,
};
use crate::render::traversal::compute_viewbox_transform;

/// Fill a rectangle with a repeating pattern.
pub(crate) fn fill_rect_with_pattern(
    def: &PatternDef,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
) {
    if ctx.def_depth >= MAX_PAINT_SERVER_DEPTH {
        return;
    }
    if def.root.children.is_empty() {
        return;
    }

    let (tile_w, tile_h) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (
            def.width.to_object_bbox() * bounds.size().width,
            def.height.to_object_bbox() * bounds.size().height,
        ),
        PatternUnits::UserSpaceOnUse => (
            def.width.to_user_space(bounds.size().width),
            def.height.to_user_space(bounds.size().height),
        ),
    };

    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }

    let (ox, oy) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (
            bounds.min.x + def.x.to_object_bbox() * bounds.size().width,
            bounds.min.y + def.y.to_object_bbox() * bounds.size().height,
        ),
        PatternUnits::UserSpaceOnUse => {
            // Per SVG spec, pattern x/y are in user space (the SVG viewport
            // coordinate system), not relative to the element being filled.
            // Convert from SVG user space to document layout space by adding
            // the SVG viewport origin.
            (
                ctx.svg_origin.x + def.x.to_user_space(bounds.size().width),
                ctx.svg_origin.y + def.y.to_user_space(bounds.size().height),
            )
        },
    };

    let has_transform = !def.transform.is_empty();
    let transform_matrix = transform::compute_transform_matrix(&def.transform);

    // Determine the range of tile indices covering the host shape. The tiles
    // live in the pattern's local coordinate system, so map the shape's bounds
    // into that space with the inverse of `patternTransform` and take the
    // resulting bounding box. This is required for rotated/skewed patterns: a
    // margin around the untransformed bounds is not enough because the shape
    // can extend arbitrarily far along the rotated axes.
    let (start_col, start_row, end_col, end_row) = if has_transform {
        let Some(inverse) = transform_matrix.inverse() else {
            return;
        };
        let to_local =
            |x: f32, y: f32| inverse.transform_point(euclid::Point2D::new(x - ox, y - oy));
        let corners = [
            to_local(bounds.min.x, bounds.min.y),
            to_local(bounds.max.x, bounds.min.y),
            to_local(bounds.max.x, bounds.max.y),
            to_local(bounds.min.x, bounds.max.y),
        ];
        let mut min = corners[0];
        let mut max = corners[0];
        for corner in &corners[1..] {
            min.x = min.x.min(corner.x);
            min.y = min.y.min(corner.y);
            max.x = max.x.max(corner.x);
            max.y = max.y.max(corner.y);
        }
        (
            (min.x / tile_w).floor() as i32,
            (min.y / tile_h).floor() as i32,
            (max.x / tile_w).ceil() as i32,
            (max.y / tile_h).ceil() as i32,
        )
    } else {
        (
            ((bounds.min.x - ox) / tile_w).floor() as i32,
            ((bounds.min.y - oy) / tile_h).floor() as i32,
            ((bounds.max.x - ox) / tile_w).ceil() as i32,
            ((bounds.max.y - oy) / tile_h).ceil() as i32,
        )
    };

    // Clip the entire pattern fill to the shape bounds once.
    let bounds_clip_id = ctx.wr.define_clip_rect(ctx.spatial_id, bounds);
    let bounds_clip = ctx
        .wr
        .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [bounds_clip_id]);

    let scale_for_content = matches!(
        def.pattern_content_units,
        PatternContentUnits::ObjectBoundingBox
    );

    // Apply patternTransform once (outside the tile loop) so the pattern
    // coordinate system is transformed, then tile inside it. A single
    // reference frame anchored at the pattern origin is used rather than one
    // frame per operation, so the tile grid and the shape bounds share the
    // exact same transform.
    let mut base_spatial = ctx.spatial_id;
    let mut base_pushed: u32 = 0;
    if has_transform {
        let lt = transform::to_layout_transform(&transform_matrix);
        base_spatial = ctx.wr.push_reference_frame(
            LayoutPoint::new(ox, oy),
            ctx.spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(lt),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );
        base_pushed = 1;
    }

    // Prefer rendering the tile content once into a texture and repeating it in
    // the pattern's (possibly rotated/skewed) coordinate system. Besides being
    // much cheaper than drawing every tile individually, this avoids the
    // antialiasing seams and double-compositing that per-tile drawing produces
    // where adjacent translucent tiles meet.
    let repeat_bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(
            if has_transform {
                start_col as f32 * tile_w
            } else {
                ox + start_col as f32 * tile_w
            },
            if has_transform {
                start_row as f32 * tile_h
            } else {
                oy + start_row as f32 * tile_h
            },
        ),
        LayoutSize::new(
            (end_col - start_col) as f32 * tile_w,
            (end_row - start_row) as f32 * tile_h,
        ),
    );
    if push_pattern_tile_image(
        def,
        tile_w,
        tile_h,
        repeat_bounds,
        scale_for_content,
        base_spatial,
        bounds_clip,
        ctx,
    ) {
        for _ in 0..base_pushed {
            ctx.wr.pop_reference_frame();
        }
        return;
    }

    for row in start_row..end_row {
        for col in start_col..end_col {
            // Tile origin in the (transformed) pattern coordinate system.
            // For the no-transform case, the (ox, oy) offset is applied
            // directly to the tile origin (no reference frame carries it).
            let tile_origin = if has_transform {
                LayoutPoint::new(col as f32 * tile_w, row as f32 * tile_h)
            } else {
                LayoutPoint::new(ox + col as f32 * tile_w, oy + row as f32 * tile_h)
            };

            // Clip this tile's content to its own tile bounds (in the tile's
            // local space, after patternTransform) so shapes don't bleed into
            // adjacent tiles.
            let tile_bounds =
                LayoutRect::from_origin_and_size(tile_origin, LayoutSize::new(tile_w, tile_h));
            let tile_clip_id = ctx.wr.define_clip_rect(base_spatial, tile_bounds);
            let tile_clip = ctx
                .wr
                .define_clip_chain(clip_chain_option(bounds_clip), [tile_clip_id]);

            // Apply the pattern viewBox → tile transform (with
            // preserveAspectRatio) so content is scaled into the tile.
            let mut t_origin = tile_origin;
            let mut t_spatial = base_spatial;
            let mut pushed_frames: u32 = 0;
            if let Some(vb) = &def.view_box {
                let (sx, sy, ox, oy) = compute_viewbox_transform(
                    vb.width.get(),
                    vb.height.get(),
                    tile_w,
                    tile_h,
                    def.aspect_ratio.as_ref(),
                );
                let t1 = Transform2D::<f32, (), ()>::translation(-vb.min_x.get(), -vb.min_y.get());
                let s = Transform2D::<f32, (), ()>::scale(sx, sy);
                let t2 = Transform2D::<f32, (), ()>::translation(ox, oy);
                let combined = t1.then(&s).then(&t2);
                let lt = transform::to_layout_transform(&combined);
                let fid = ctx.wr.push_reference_frame(
                    t_origin,
                    t_spatial,
                    TransformStyle::Flat,
                    PropertyBinding::Value(lt),
                    ReferenceFrameKind::Transform {
                        is_2d_scale_translation: false,
                        should_snap: false,
                        paired_with_perspective: false,
                    },
                );
                t_origin = LayoutPoint::zero();
                t_spatial = fid;
                pushed_frames += 1;
            }

            // When patternContentUnits="objectBoundingBox", shape coordinates
            // are in a 0..1 unit space — scale by tile dimensions.
            let (origin, spatial, pushed) = if scale_for_content && tile_w > 0.0 && tile_h > 0.0 {
                let scale_x = tile_w;
                let scale_y = tile_h;
                let fid = ctx.wr.push_reference_frame(
                    t_origin,
                    t_spatial,
                    TransformStyle::Flat,
                    PropertyBinding::Value(LayoutTransform::scale(scale_x, scale_y, 1.0)),
                    ReferenceFrameKind::Transform {
                        is_2d_scale_translation: true,
                        should_snap: false,
                        paired_with_perspective: false,
                    },
                );
                (LayoutPoint::zero(), fid, true)
            } else {
                (t_origin, t_spatial, false)
            };

            def.root.for_each_shape_leaf(&mut |shape, shape_style| {
                if !shape_style.is_visible() {
                    return;
                }
                let mut shape_ctx = RenderContext {
                    style: shape_style,
                    svg_origin: origin,
                    spatial_id: spatial,
                    clip_chain_id: tile_clip,
                    wr: &mut *ctx.wr,
                    accumulated_scale: ctx.accumulated_scale,
                    viewbox_scale: ctx.viewbox_scale,
                    device_scale: ctx.device_scale,
                    raster_offset: ctx.raster_offset,
                    native_rendering: true,
                    sink: ctx.sink,
                    defs: ctx.defs,
                    def_depth: ctx.def_depth + 1,
                };
                shape.render(&mut shape_ctx);
            });

            if pushed {
                ctx.wr.pop_reference_frame();
            }
            for _ in 0..pushed_frames {
                ctx.wr.pop_reference_frame();
            }
        }
    }

    // Pop the patternTransform reference frames.
    for _ in 0..base_pushed {
        ctx.wr.pop_reference_frame();
    }
}

/// Render the pattern's tile content once with the CPU rasterizer and push it as
/// a repeating image in the pattern's coordinate space.
///
/// This matches how browsers implement patterns (rasterize one tile, then
/// repeat the texture) and avoids both the antialiasing seams where adjacent
/// translucent tiles meet and the soft edges that result from antialiasing every
/// rotated tile separately. Returns `false` when the content cannot be flattened
/// (nested patterns/images/effects), letting the caller fall back to per-tile
/// rendering.
#[allow(clippy::too_many_arguments)]
fn push_pattern_tile_image(
    def: &PatternDef,
    tile_w: f32,
    tile_h: f32,
    repeat_bounds: LayoutRect,
    scale_for_content: bool,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    ctx: &mut RenderContext,
) -> bool {
    let device_scale = ctx.device_scale;
    let w = ((tile_w * device_scale).round() as i64).max(1);
    let h = ((tile_h * device_scale).round() as i64).max(1);
    if w > u16::MAX as i64 || h > u16::MAX as i64 {
        return false;
    }

    // Tile content coordinates -> tile space, including `viewBox` /
    // patternContentUnits and any objectBoundingBox scaling.
    let mut content = Transform2D::<f32, (), ()>::identity();
    if scale_for_content {
        content = Transform2D::<f32, (), ()>::scale(tile_w, tile_h);
    }
    if let Some(vb) = &def.view_box {
        let (sx, sy, tx, ty) = compute_viewbox_transform(
            vb.width.get(),
            vb.height.get(),
            tile_w,
            tile_h,
            def.aspect_ratio.as_ref(),
        );
        let view_box = Transform2D::<f32, (), ()>::translation(-vb.min_x.get(), -vb.min_y.get())
            .then(&Transform2D::<f32, (), ()>::scale(sx, sy))
            .then(&Transform2D::<f32, (), ()>::translation(tx, ty));
        content = content.then(&view_box);
    }

    let mut flat = true;
    let mut leaves: Vec<GroupLeaf> = Vec::new();
    for child in &def.root.children {
        collect_leaves(child, content, 1.0, ctx.defs, &mut flat, &mut leaves);
    }
    if !flat || leaves.is_empty() {
        return false;
    }
    if prepare_leaves(&mut leaves, ctx.viewbox_scale).is_none() {
        return false;
    }

    let rgba = render_leaves(
        &leaves,
        0.0,
        0.0,
        w as u16,
        h as u16,
        device_scale,
        ctx.viewbox_scale,
        ctx.defs,
    );

    let mut hasher = DefaultHasher::new();
    rgba.hash(&mut hasher);
    let Some(key) = ctx.sink.uploader.upload(hasher.finish(), rgba, w as u32, h as u32) else {
        return false;
    };

    let common = CommonItemProperties {
        clip_rect: repeat_bounds,
        spatial_id,
        clip_chain_id,
        flags: PrimitiveFlags::default(),
    };
    ctx.wr.push_repeating_image(
        &common,
        repeat_bounds,
        LayoutSize::new(tile_w, tile_h),
        LayoutSize::zero(),
        ImageRendering::Auto,
        AlphaType::PremultipliedAlpha,
        key,
        ColorF::WHITE,
    );
    true
}

