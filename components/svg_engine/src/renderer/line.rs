/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Line;
use crate::renderer::{Render, RenderContext};
use crate::renderer::stroke;

/// Renders an SVG `<line>`.
///
/// LSP contract:
/// - SVG specification: `<line>` has no fill geometry.
/// - Only emits stroke commands (via [`stroke::stroke_line_segment`]).
/// - Correctly ignores `ctx.style.fill` even when `Some`.
/// - Handles both solid-color and gradient paint servers.
/// - This is a **spec-mandated behavioral difference**, not a violation
///   of LSP — the [`Render`] trait contract explicitly permits shapes
///   to omit fill when the SVG spec requires it.
impl Render for Line {
    fn render(&self, ctx: &mut RenderContext) {
        let Some(stroke) = &ctx.style.stroke else { return };
        if (!stroke.color.is_some() && stroke.paint_server.is_none()) || stroke.width <= 0.0 {
            return;
        }

        // Delegate to the shared line-segment helper with absolute coordinates.
        stroke::stroke_line_segment(
            ctx.svg_origin.x + self.x1,
            ctx.svg_origin.y + self.y1,
            ctx.svg_origin.x + self.x2,
            ctx.svg_origin.y + self.y2,
            ctx,
        );
    }
}
