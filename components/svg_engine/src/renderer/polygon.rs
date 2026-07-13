use crate::shapes::{Polygon, Polyline};
use crate::renderer::{Render, RenderContext};

impl Render for Polygon {
    fn render(&self, ctx: &mut RenderContext) {
        let mut closed_points = self.points.clone();
        if let Some(first) = self.points.first() {
            closed_points.push(*first);
        }

        let polyline = Polyline { points: closed_points };
        polyline.render(ctx);
    }
}
