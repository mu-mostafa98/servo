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
        if self.rx.get() <= 0.0 || self.ry.get() <= 0.0 {
            return;
        }

        // An ellipse is rendered as a rounded rectangle with 100% corner radii.
        let rect = Rectangle {
            x: Length::new(self.cx.get() - self.rx.get()),
            y: Length::new(self.cy.get() - self.ry.get()),
            width: Length::new(self.rx.get() * 2.0),
            height: Length::new(self.ry.get() * 2.0),
            rx: Some(self.rx),
            ry: Some(self.ry),
        };
        rect.render(ctx);
    }
}
