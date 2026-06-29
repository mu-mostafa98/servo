/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use lyon::math::Point as LyonPoint;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
};

use crate::shapes::{Line, Polyline};
use crate::style::*;
use crate::tessellator;
use crate::renderer::Render;

impl Render for Polyline {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        let points = &self.points;
        if points.len() < 2 {
            return;
        }

        // Convert to f32 with svg_origin offset for fill tessellation.
        let shifted_pts: Vec<LyonPoint> = points
            .iter()
            .map(|p| LyonPoint::new(svg_origin.x + p.x as f32, svg_origin.y + p.y as f32))
            .collect();

        // ── FILL ──────────────────────────────────────────────────────────
        if points.len() >= 3 {
            if let Some(fill) = &style.fill {
                if let Some(mut color) = fill.color {
                    color.a *= fill.opacity;
                    tessellator::tessellate_polygon(
                        &shifted_pts,
                        fill.fill_rule,
                        color,
                        spatial_id,
                        clip_chain_id,
                        wr,
                    );
                }
            }
        }

        // ── STROKE ────────────────────────────────────────────────────────
        if let Some(stroke) = &style.stroke {
            if stroke.color.is_some() && stroke.width > 0.0 {
                // Build the stroke-only style once, outside the loop.
                let stroke_style = NodeStyle {
                    opacity: 1.0,
                    visibility: Visibility::Visible,
                    display: Display::Inline,
                    transform: Vec::new(),
                    fill: None,
                    render_hints: None,
                    effects: None,
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
                for pair in points.windows(2) {
                    let line = Line {
                        x1: pair[0].x as f32,
                        y1: pair[0].y as f32,
                        x2: pair[1].x as f32,
                        y2: pair[1].y as f32,
                    };
                    line.render(&stroke_style, svg_origin, spatial_id, clip_chain_id, wr);
                }
            }
        }
    }
}
