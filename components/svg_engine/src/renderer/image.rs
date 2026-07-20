/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renders an SVG `<image>` element.

use crate::image::SvgImage;
use crate::renderer::Render;

impl Render for SvgImage {
    fn render(&self) {
        eprintln!(
            "  image: href={:?}, x={}, y={}, width={}, height={}",
            self.href, self.x, self.y, self.width, self.height
        );
    }
}
