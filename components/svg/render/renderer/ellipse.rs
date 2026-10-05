/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering for SVG `<ellipse>`.

use crate::render::renderer::{Render, RenderContext};
use crate::model::element::shape::Ellipse;

/// Renders an SVG `<ellipse>`.
///
/// LSP contract:
/// - Delegates to [`Rectangle::render`] with 100% corner radii.
/// - All LSP invariants are preserved through the delegation chain.
impl Render for Ellipse {
    fn render(&self, ctx: &mut RenderContext) {
        // An ellipse is rendered as a rounded rectangle with 100% corner radii.
        let Some(rect) = self.to_rect() else {
            return;
        };
        rect.render(ctx);
    }
}
