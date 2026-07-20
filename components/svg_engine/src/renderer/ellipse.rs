/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renders an SVG `<ellipse>`.

use crate::renderer::Render;
use crate::shapes::Ellipse;

impl Render for Ellipse {
    fn render(&self) {
        eprintln!(
            "  ellipse: cx={}, cy={}, rx={}, ry={}",
            self.cx, self.cy, self.rx, self.ry
        );
    }
}
