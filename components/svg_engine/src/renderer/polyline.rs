use lyon::math::Point as LyonPoint;

use crate::shapes::Polyline;
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

        // FILL (only, stroke comes in Phase 6)
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
    }
}
