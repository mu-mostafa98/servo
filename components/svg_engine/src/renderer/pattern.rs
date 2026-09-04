/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG pattern fill rendering.

use euclid::Transform2D;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutTransform};
use webrender_api::{PropertyBinding, ReferenceFrameKind, TransformStyle};

use crate::render_tree::{PatternContentUnits, PatternUnits};
use crate::renderer::{Render, RenderContext, clip_chain_option, transform};
use crate::traversal::compute_viewbox_transform;

/// Fill a rectangle with a repeating pattern.
pub(crate) fn fill_rect_with_pattern_by_id(
    pattern_id: &str,
    bounds: LayoutRect,
    ctx: &mut RenderContext,
    _opacity: f32,
) {
    let def = match ctx.paints.pattern(pattern_id) {
        Some(d) => d,
        None => {
            log::warn!("SVG pattern \"{}\" not found in definitions", pattern_id);
            return;
        },
    };

    if def.shapes.is_empty() {
        return;
    }

    let (tile_w, tile_h) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (
            def.width * bounds.size().width,
            def.height * bounds.size().height,
        ),
        PatternUnits::UserSpaceOnUse => (def.width, def.height),
    };

    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }

    let (ox, oy) = match def.pattern_units {
        PatternUnits::ObjectBoundingBox => (
            bounds.min.x + def.x * bounds.size().width,
            bounds.min.y + def.y * bounds.size().height,
        ),
        PatternUnits::UserSpaceOnUse => {
            // Per SVG spec, pattern x/y are in user space (the SVG viewport
            // coordinate system), not relative to the element being filled.
            // Convert from SVG user space to document layout space by adding
            // the SVG viewport origin.
            (ctx.svg_origin.x + def.x, ctx.svg_origin.y + def.y)
        },
    };

    let has_transform = !def.transform.is_empty();

    // When a patternTransform is present, the tiles may be rotated/skewed, so
    // expand the tiling range to guarantee full coverage of the host shape.
    let margin = if has_transform {
        (tile_w * tile_w + tile_h * tile_h).sqrt()
    } else {
        0.0
    };
    let start_col = ((bounds.min.x - ox - margin) / tile_w).floor() as i32;
    let start_row = ((bounds.min.y - oy - margin) / tile_h).floor() as i32;
    let end_col = ((bounds.max.x - ox + margin) / tile_w).ceil() as i32;
    let end_row = ((bounds.max.y - oy + margin) / tile_h).ceil() as i32;

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
    // coordinate system is transformed, then tile inside it.
    let mut base_origin = LayoutPoint::new(ox, oy);
    let mut base_spatial = ctx.spatial_id;
    let mut base_pushed: u32 = 0;
    for op in &def.transform {
        let result = transform::apply_transform_op(op, base_origin, base_spatial, ctx.wr);
        base_origin = result.child_origin;
        base_spatial = result.child_spatial_id;
        if result.pushed_frame {
            base_pushed += 1;
        }
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
            let tile_bounds = LayoutRect::from_origin_and_size(
                tile_origin,
                webrender_api::units::LayoutSize::new(tile_w, tile_h),
            );
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
                    vb.width,
                    vb.height,
                    tile_w,
                    tile_h,
                    def.aspect_ratio.as_ref(),
                );
                let t1 = Transform2D::<f32, (), ()>::translation(-vb.min_x, -vb.min_y);
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

            for (shape, shape_style) in &def.shapes {
                if !shape_style.is_visible() {
                    continue;
                }
                let mut shape_ctx = RenderContext {
                    style: shape_style,
                    svg_origin: origin,
                    spatial_id: spatial,
                    clip_chain_id: tile_clip,
                    wr: &mut *ctx.wr,
                    paints: ctx.paints,
                    accumulated_scale: ctx.accumulated_scale,
                    viewbox_scale: ctx.viewbox_scale,
                    device_scale: ctx.device_scale,
                    raster_offset: ctx.raster_offset,
                    native_rendering: true,
                    rasters: &mut *ctx.rasters,
                };
                shape.render(&mut shape_ctx);
            }

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
