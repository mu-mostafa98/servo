/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renders SVG `<text>` and `<tspan>` elements.

use crate::renderer::Render;
use crate::text::TextSpan;

impl Render for TextSpan {
    fn render(&self) {
        eprintln!(
            "  text: \"{}\" x={}, y={}, text_anchor={:?}, glyphs={}",
            self.text, self.x, self.y, self.text_anchor, self.glyphs.len()
        );
    }
}
