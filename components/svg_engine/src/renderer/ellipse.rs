/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::renderer::{Render, RenderContext};
use crate::shapes::{Ellipse, Rectangle};

impl Render for Ellipse {
    fn render(&self, ctx: &mut RenderContext) {
        if self.rx <= 0.0 || self.ry <= 0.0 {
            return;
        }

        let rect = Rectangle {
            x: self.cx - self.rx,
            y: self.cy - self.ry,
            width: self.rx * 2.0,
            height: self.ry * 2.0,
            rx: Some(self.rx),
            ry: Some(self.ry),
        };
        rect.render(ctx);
    }
}
