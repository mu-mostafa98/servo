/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::renderer::{Render, RenderContext, stroke};
use crate::shapes::Line;

impl Render for Line {
    fn render(&self, ctx: &mut RenderContext) {
        let Some(stroke) = &ctx.style.stroke else {
            return;
        };
        if (stroke.color.is_none() && stroke.paint_server.is_none()) || stroke.width <= 0.0 {
            return;
        }

        stroke::stroke_line_segment(
            ctx.svg_origin.x + self.x1,
            ctx.svg_origin.y + self.y1,
            ctx.svg_origin.x + self.x2,
            ctx.svg_origin.y + self.y2,
            ctx,
        );
    }
}
