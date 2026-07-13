use lyon::math::Point as LyonPoint;

use crate::shapes::{Line, Polyline};
use crate::renderer::{Render, RenderContext, to_colorf};
use crate::tessellator;

impl Render for Polyline {
    fn render(&self, ctx: &mut RenderContext) {
        let points = &self.points;
        if points.len() < 2 {
            return;
        }

        let shifted_pts: Vec<LyonPoint> = points
            .iter()
            .map(|p| {
                LyonPoint::new(
                    ctx.svg_origin.x + p.x as f32,
                    ctx.svg_origin.y + p.y as f32,
                )
            })
            .collect();

        let local_points: Vec<(f32, f32)> = points
            .iter()
            .map(|p| (p.x as f32, p.y as f32))
            .collect();

        // FILL
        if points.len() >= 3 {
            if let Some(fill) = &ctx.style.fill {
                if let Some(svg_color) = fill.color {
                    let mut color = to_colorf(&svg_color);
                    color.a *= fill.opacity * ctx.style.opacity;
                    tessellator::tessellate_polygon(
                        &shifted_pts,
                        fill.fill_rule,
                        color,
                        ctx.spatial_id,
                        ctx.clip_chain_id,
                        ctx.wr,
                    );
                }
            }
        }

        // STROKE — each segment as an individual line
        if let Some(stroke) = &ctx.style.stroke {
            if let Some(svg_color) = stroke.color {
                if stroke.width > 0.0 {
                    let stroke_style = crate::style::NodeStyle {
                        fill: None,
                        stroke: Some(crate::style::StrokeParams {
                            color: Some(svg_color),
                            opacity: stroke.opacity,
                            width: stroke.width,
                            line_cap: stroke.line_cap,
                            line_join: stroke.line_join,
                            miter_limit: stroke.miter_limit,
                            dash_array: stroke.dash_array.clone(),
                            dash_offset: stroke.dash_offset,
                        }),
                        ..Default::default()
                    };

                    for i in 0..local_points.len() - 1 {
                        let line = Line {
                            x1: local_points[i].0,
                            y1: local_points[i].1,
                            x2: local_points[i + 1].0,
                            y2: local_points[i + 1].1,
                        };
                        let mut seg_ctx = RenderContext {
                            style: &stroke_style,
                            svg_origin: ctx.svg_origin,
                            spatial_id: ctx.spatial_id,
                            clip_chain_id: ctx.clip_chain_id,
                            wr: &mut *ctx.wr,
                        };
                        line.render(&mut seg_ctx);
                    }
                }
            }
        }
    }
}
