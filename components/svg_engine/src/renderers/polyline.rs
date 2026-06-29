/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use lyon::math::Point as LyonPoint;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
};

use crate::shapes::Line;
use crate::styles::*;

use crate::renderers::polygon_tessellator::tessellate_polygon;

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

    // Convert to f32 with svg_origin offset for fill.
    let shifted_pts: Vec<LyonPoint> = points.iter().map(|p| {
        LyonPoint::new(svg_origin.x + p.x as f32, svg_origin.y + p.y as f32)
    }).collect();

    // Convert to f32 without origin offset for stroke delegation
    // (render_line will add svg_origin internally).
    let local_points: Vec<(f32, f32)> = points.iter()
        .map(|p| (p.x as f32, p.y as f32))
        .collect();

    // ── FILL ──────────────────────────────────────────────────────────
    if points.len() >= 3 {
        if let Some(fill) = &style.fill {
            if let Some(mut color) = fill.color {
                color.a *= fill.opacity;

                tessellate_polygon(&shifted_pts, fill.fill_rule, color, spatial_id, clip_chain_id, wr);
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
            // For a polyline with sparse straight segments individual line-caps
            // at vertices are visually fine.
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
