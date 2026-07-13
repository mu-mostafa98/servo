use kurbo::{BezPath, PathEl, Point};
use lyon::math::Point as LyonPoint;

use crate::renderer::{Render, RenderContext};
use crate::tessellator;

const FLATTEN_TOLERANCE: f64 = 0.1;

impl Render for crate::shapes::Path {
    fn render(&self, ctx: &mut RenderContext) {
        let points = flatten_path(&self.path);
        if points.len() < 3 {
            return;
        }

        // Fill via tessellator
        if let Some(fill) = &ctx.style.fill {
            if let Some(svg_color) = fill.color {
                let shifted_pts: Vec<LyonPoint> = points
                    .iter()
                    .map(|p| {
                        LyonPoint::new(
                            ctx.svg_origin.x + p.x as f32,
                            ctx.svg_origin.y + p.y as f32,
                        )
                    })
                    .collect();

                let mut color = crate::renderer::to_colorf(&svg_color);
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

fn flatten_path(path: &BezPath) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::new();
    let mut subpath_start: Option<Point> = None;

    kurbo::flatten(path.elements().iter().copied(), FLATTEN_TOLERANCE, |el| {
        match el {
            PathEl::MoveTo(p) => {
                points.push(p);
                subpath_start = Some(p);
            },
            PathEl::LineTo(p) => {
                points.push(p);
            },
            PathEl::ClosePath => {
                if let Some(start) = subpath_start {
                    points.push(start);
                }
                subpath_start = None;
            },
            _ => {},
        }
    });

    points
}
