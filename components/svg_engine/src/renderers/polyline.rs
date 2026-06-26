/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, SpaceAndClipInfo,
    ImageKey, ImageMask,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::shapes::Line;
use crate::styles::*;

pub fn render_polyline(
    polyline: &crate::shapes::Polyline,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let points = &polyline.points;
    if points.len() < 2 {
        return;
    }

    // Convert to f32 with svg_origin offset for the fill clip mask.
    let layout_points: Vec<LayoutPoint> = points.iter().map(|p| {
        LayoutPoint::new(svg_origin.x + p.x as f32, svg_origin.y + p.y as f32)
    }).collect();

    // Convert to f32 without origin offset for stroke delegation
    // (render_line will add svg_origin internally).
    let local_points: Vec<(f32, f32)> = points.iter()
        .map(|p| (p.x as f32, p.y as f32))
        .collect();

    // ── FILL ──────────────────────────────────────────────────────────
    // Requires at least 3 points to form a filled area.
    if points.len() >= 3 {
        if let Some(fill) = &style.fill {
            if let Some(mut color) = fill.color {
                color.a *= fill.opacity;

                // Compute bounding box of the shifted points.
                let (min_x, max_x) = layout_points.iter()
                    .map(|p| p.x)
                    .fold((f32::MAX, f32::MIN), |(mn, mx), x| (mn.min(x), mx.max(x)));
                let (min_y, max_y) = layout_points.iter()
                    .map(|p| p.y)
                    .fold((f32::MAX, f32::MIN), |(mn, mx), y| (mn.min(y), mx.max(y)));
                let bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(min_x, min_y),
                    LayoutSize::new(max_x - min_x, max_y - min_y),
                );

                let wr_fill_rule = match fill.fill_rule {
                    FillRule::NonZero => webrender_api::FillRule::Nonzero,
                    FillRule::EvenOdd => webrender_api::FillRule::Evenodd,
                };

                let clip_id = wr.define_clip_image_mask(
                    spatial_id,
                    ImageMask {
                        image: ImageKey::DUMMY,
                        rect: bounds,
                    },
                    &layout_points,
                    wr_fill_rule,
                );

                let parent = match clip_chain_id {
                    ClipChainId::INVALID => None,
                    id => Some(id),
                };
                let poly_clip = wr.define_clip_chain(parent, [clip_id]);
                let common = CommonItemProperties::new(
                    bounds,
                    SpaceAndClipInfo { spatial_id, clip_chain_id: poly_clip },
                );
                wr.push_rect(&common, bounds, color);
            }
        }
    }

    // ── STROKE ────────────────────────────────────────────────────────
    if let Some(stroke) = &style.stroke {
        if stroke.color.is_some() && stroke.width > 0.0 {
            let stroke_style = NodeStyle {
                fill: None,
                stroke: Some(StrokeParams {
                    color: stroke.color,
                    opacity: stroke.opacity,
                    width: stroke.width,
                    line_cap: stroke.line_cap,
                    line_join: stroke.line_join,
                    miter_limit: stroke.miter_limit,
                    dash_array: stroke.dash_array.clone(),
                    dash_offset: stroke.dash_offset,
                }),
            };

            // Stroke each consecutive pair: (p0→p1), (p1→p2), ..., (pn-2→pn-1).
            for i in 0..local_points.len() - 1 {
                let line = Line {
                    x1: local_points[i].0,
                    y1: local_points[i].1,
                    x2: local_points[i + 1].0,
                    y2: local_points[i + 1].1,
                };
                super::render_line(&line, &stroke_style, svg_origin, spatial_id, clip_chain_id, wr);
            }
        }
    }
}
