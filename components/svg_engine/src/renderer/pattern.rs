/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG pattern fill rendering.

use webrender_api::units::{LayoutPoint, LayoutRect};

use crate::render_tree::PatternUnits;
use crate::renderer::{Render, RenderContext, clip_chain_option};

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

    let start_col = ((bounds.min.x - ox) / tile_w).floor() as i32;
    let start_row = ((bounds.min.y - oy) / tile_h).floor() as i32;
    let end_col = ((bounds.max.x - ox) / tile_w).ceil() as i32;
    let end_row = ((bounds.max.y - oy) / tile_h).ceil() as i32;

    // Clip the entire pattern fill to the shape bounds once.
    let bounds_clip_id = ctx.wr.define_clip_rect(ctx.spatial_id, bounds);
    let bounds_clip = ctx
        .wr
        .define_clip_chain(clip_chain_option(ctx.clip_chain_id), [bounds_clip_id]);

    for row in start_row..end_row {
        for col in start_col..end_col {
            let tile_origin = LayoutPoint::new(ox + col as f32 * tile_w, oy + row as f32 * tile_h);

            for (shape, shape_style) in &def.shapes {
                if !shape_style.is_visible() {
                    continue;
                }
                let mut shape_ctx = RenderContext {
                    style: shape_style,
                    svg_origin: tile_origin,
                    spatial_id: ctx.spatial_id,
                    clip_chain_id: bounds_clip,
                    wr: &mut *ctx.wr,
                    paints: ctx.paints,
                    accumulated_scale: ctx.accumulated_scale,
                };
                shape.render(&mut shape_ctx);
            }
        }
    }
}
