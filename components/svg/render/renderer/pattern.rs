/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG pattern fill rendering.

use euclid::Transform2D;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform};
use webrender_api::{PropertyBinding, ReferenceFrameKind, TransformStyle};

use crate::model::document::{PatternContentUnits, PatternDef, PatternUnits};
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

    // `patternTransform` is applied in the user space (the referencing element's
    // coordinate system), so its scale/rotate is about the SVG origin, not the
    // tile origin (ox, oy). Transform the tile origin into the user-space frame
    // and apply only the linear part of `transform_matrix` (scale/rotate/skew) —
    // the translation part is carried by the reference frame's own transform
    // below. This matters for `objectBoundingBox` patterns (where ox != the SVG
    // origin): a `scale` would otherwise shrink toward the bbox corner instead
    // of the viewport origin, shifting the whole tile grid.
    let anchor = if has_transform {
        let ox_local = ox - ctx.svg_origin.x;
        let oy_local = oy - ctx.svg_origin.y;
        LayoutPoint::new(
            ctx.svg_origin.x + transform_matrix.m11 * ox_local + transform_matrix.m21 * oy_local,
            ctx.svg_origin.y + transform_matrix.m12 * ox_local + transform_matrix.m22 * oy_local,
        )
    } else {
        LayoutPoint::new(ox, oy)
    };

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
            |x: f32, y: f32| inverse.transform_point(euclid::Point2D::new(x - anchor.x, y - anchor.y));
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

    // A `viewBox` overrides `patternContentUnits` (SVG 1.1 §13.3), so only
    // apply the objectBoundingBox scale when there is no viewBox. The scale is
    // the bounding box of the element being filled, not the tile size.
    let content_scale = match def.pattern_content_units {
        PatternContentUnits::ObjectBoundingBox if def.view_box.is_none() => {
            Some((bounds.size().width, bounds.size().height))
        },
        _ => None,
    };

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
            anchor,
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

    // Render every tile via the native shape primitives. (The repeating-image
    // fast path was removed: WebRender's repeating-image primitive drops curved
    // circle/ellipse tile content in the multi-tile case.)

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
            // are in a 0..1 unit space scaled by the host element's bounding
            // box (not the tile size).
            let (origin, spatial, pushed) = if let Some((scale_x, scale_y)) = content_scale {
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


