/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::renderer::{Render, RenderContext};
use crate::shapes::{Circle, Ellipse};

/// Renders an SVG `<circle>`.
///
/// LSP contract:
/// - Delegates to [`Ellipse::render`] with `rx = ry = r`.
/// - All LSP invariants are preserved through the delegation chain.
impl Render for Circle {
    fn render(&self, ctx: &mut RenderContext) {
        // A circle is an ellipse with equal rx and ry.
        let ellipse = Ellipse {
            cx: self.cx,
            cy: self.cy,
            rx: self.r,
            ry: self.r,
        };
        ellipse.render(ctx);
    }
}
