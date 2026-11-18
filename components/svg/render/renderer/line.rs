/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering for SVG `<line>`.

use crate::model::element::shape::Line;
use crate::render::renderer::{Render, RenderContext, ZERO_LENGTH_EPSILON, stroke};

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
        let Some(stroke) = &ctx.style.stroke else {
            return;
        };
        if stroke.paint_server.is_none() || stroke.width.get() <= 0.0 {
            return;
        }

        // Delegate to the shared line-segment helper with absolute coordinates.
        let x1 = ctx.svg_origin.x + self.x1.get();
        let y1 = ctx.svg_origin.y + self.y1.get();
        let x2 = ctx.svg_origin.x + self.x2.get();
        let y2 = ctx.svg_origin.y + self.y2.get();

        // A zero-length subpath is not stroked with a `butt` cap, but with a
        // `round` or `square` cap it is stroked as a dot centred on the
        // segment's point (SVG 1.1 §11.4).
        if (x2 - x1).abs() < ZERO_LENGTH_EPSILON && (y2 - y1).abs() < ZERO_LENGTH_EPSILON {
            stroke::stroke_zero_length_dot(x1, y1, ctx);
            return;
        }

        stroke::stroke_line_segment(x1, y1, x2, y2, ctx);
    }
}
