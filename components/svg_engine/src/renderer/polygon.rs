/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::renderer::{Render, RenderContext};
use crate::shapes::{Polygon, Polyline};

impl Render for Polygon {
    fn render(&self, ctx: &mut RenderContext) {
        let mut closed_points = self.points.clone();
        if let Some(first) = self.points.first() {
            closed_points.push(*first);
        }

        let polyline = Polyline {
            points: closed_points,
        };
        polyline.render(ctx);
    }
}
