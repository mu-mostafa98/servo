use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{BorderRadius, ClipMode, ComplexClipRegion};

use crate::shapes::Rectangle;
use crate::renderer::{Render, RenderContext, clip_chain_option, fill, stroke};

impl Render for Rectangle {
    fn render(&self, ctx: &mut RenderContext) {
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(ctx.svg_origin.x + self.x, ctx.svg_origin.y + self.y),
            LayoutSize::new(self.width, self.height),
        );

        let rx = self
            .rx
            .or(self.ry)
            .unwrap_or(0.0)
            .clamp(0.0, self.width / 2.0);
        let ry = self
            .ry
            .or(self.rx)
            .unwrap_or(0.0)
            .clamp(0.0, self.height / 2.0);
        let has_radius = rx > 0.0 || ry > 0.0;
        let radii = has_radius.then(|| BorderRadius {
            top_left: LayoutSize::new(rx, ry),
            top_right: LayoutSize::new(rx, ry),
            bottom_left: LayoutSize::new(rx, ry),
            bottom_right: LayoutSize::new(rx, ry),
        });

        let clip = if let Some(r) = radii {
            let clip_id = ctx.wr.define_clip_rounded_rect(
                ctx.spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii: r,
                    mode: ClipMode::Clip,
                },
            );
            let parent = clip_chain_option(ctx.clip_chain_id);
            ctx.wr.define_clip_chain(parent, [clip_id])
        } else {
            ctx.clip_chain_id
        };

        fill::fill_rect(bounds, clip, ctx);
        stroke::stroke_rect(bounds, radii, ctx);
    }
}
