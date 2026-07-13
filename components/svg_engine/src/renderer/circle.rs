use crate::shapes::{Circle, Ellipse};
use crate::renderer::{Render, RenderContext};

impl Render for Circle {
    fn render(&self, ctx: &mut RenderContext) {
        let ellipse = Ellipse {
            cx: self.cx,
            cy: self.cy,
            rx: self.r,
            ry: self.r,
        };
        ellipse.render(ctx);
    }
}
