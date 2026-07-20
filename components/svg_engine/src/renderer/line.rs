/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renders an SVG `<line>`.

use crate::renderer::Render;
use crate::shapes::Line;

impl Render for Line {
    fn render(&self) {
        eprintln!(
            "  line: x1={}, y1={}, x2={}, y2={}",
            self.x1, self.y1, self.x2, self.y2
        );
    }
}
