/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renders an SVG `<rect>`.

use crate::renderer::Render;
use crate::shapes::Rectangle;

impl Render for Rectangle {
    fn render(&self) {
        eprintln!(
            "  rect: x={}, y={}, width={}, height={}, rx={:?}, ry={:?}",
            self.x, self.y, self.width, self.height, self.rx, self.ry
        );
    }
}
