/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use lyon::math::Point as LyonPoint;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::renderer::{Render, RenderContext, fill, paint_order_stroke_before_fill, stroke};
use crate::shapes::Polyline;
use crate::style::FillRule;

impl Render for Polyline {
    fn render(&self, ctx: &mut RenderContext) {
        let points = &self.points;
        if points.len() < 2 {
            return;
        }

        let shifted_pts: Vec<LyonPoint> = points
            .iter()
            .map(|p| LyonPoint::new(ctx.svg_origin.x + p.x as f32, ctx.svg_origin.y + p.y as f32))
            .collect();

        let fill_rule = ctx
            .style
            .fill
            .as_ref()
            .map(|f| f.fill_rule)
            .unwrap_or(FillRule::NonZero);
        let stroke_before_fill = paint_order_stroke_before_fill(ctx);

        if stroke_before_fill {
            let stroke_pts: Vec<LyonPoint> = points
                .iter()
                .map(|p| LyonPoint::new(p.x as f32, p.y as f32))
                .collect();
            stroke::stroke_polyline(&stroke_pts, ctx);

            if shifted_pts.len() >= 3 {
                let (bx, by, bw, bh) = fill::points_bounds(&shifted_pts);
                let bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(bx, by),
                    LayoutSize::new(bw, bh),
                );
                fill::fill_polygon(&shifted_pts, bounds, fill_rule, ctx);
            }
        } else {
            if shifted_pts.len() >= 3 {
                let (bx, by, bw, bh) = fill::points_bounds(&shifted_pts);
                let bounds = LayoutRect::from_origin_and_size(
                    LayoutPoint::new(bx, by),
                    LayoutSize::new(bw, bh),
                );
                fill::fill_polygon(&shifted_pts, bounds, fill_rule, ctx);
            }

            let stroke_pts: Vec<LyonPoint> = points
                .iter()
                .map(|p| LyonPoint::new(p.x as f32, p.y as f32))
                .collect();
            stroke::stroke_polyline(&stroke_pts, ctx);
        }
    }
}
