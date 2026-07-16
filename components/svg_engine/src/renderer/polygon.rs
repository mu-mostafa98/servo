/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::renderer::{Render, RenderContext};
use crate::shapes::{Polygon, Polyline};

/// Renders an SVG `<polygon>`.
///
/// LSP contract:
/// - Closes the point list by appending the first point, then delegates
///   to [`Polyline::render`].  This ensures the stroke closes back to start.
/// - All LSP invariants are preserved through the delegation chain.
impl Render for Polygon {
    fn render(&self, ctx: &mut RenderContext) {
        // A polygon is a closed shape: append the first point to the end so the
        // stroke renders an edge from the last point back to the first.
        // The fill is unaffected — the tessellator already treats vertices as
        // a closed polygon regardless of duplication.
        let mut closed_points = self.points.clone();
        if let Some(first) = self.points.first() {
            closed_points.push(*first);
        }

        let polyline = Polyline {
            points: closed_points,
        };
        polyline.render(ctx);
    }
}
