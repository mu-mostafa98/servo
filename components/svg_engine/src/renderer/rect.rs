/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use crate::renderer::{Render, RenderContext, fill};
use crate::shapes::Rectangle;

impl Render for Rectangle {
    fn render(&self, ctx: &mut RenderContext) {
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(ctx.svg_origin.x + self.x, ctx.svg_origin.y + self.y),
            LayoutSize::new(self.width, self.height),
        );

        fill::fill_rect(bounds, ctx);
        // stroke::stroke_rect(bounds, radii, ctx);
    }
}
