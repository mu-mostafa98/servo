/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::render::renderer::{Render, RenderContext};
use crate::model::element::shape::{Ellipse, Rectangle};
use crate::model::units::Length;

/// Renders an SVG `<ellipse>`.
///
/// LSP contract:
/// - Delegates to [`Rectangle::render`] with 100% corner radii.
/// - All LSP invariants are preserved through the delegation chain.
impl Render for Ellipse {
    fn render(&self, ctx: &mut RenderContext) {
        let Some((rx, ry)) = self.resolved_radii() else {
            return;
        };
        if rx.get() <= 0.0 || ry.get() <= 0.0 {
            return;
        }

        // An ellipse is rendered as a rounded rectangle with 100% corner radii.
        let rect = Rectangle {
            x: Length::new(self.cx.get() - rx.get()),
            y: Length::new(self.cy.get() - ry.get()),
            width: Length::new(rx.get() * 2.0),
            height: Length::new(ry.get() * 2.0),
            rx: Some(rx),
            ry: Some(ry),
            path_length: self.path_length,
        };
        rect.render(ctx);
    }
}
