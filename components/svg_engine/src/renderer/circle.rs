/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<circle>` renderer — delegates to ellipse with rx=ry=r.

use super::RenderContext;

/// Render an SVG `<circle>` via ellipse delegation.
pub(crate) fn render(
    shape: &usvg::SimpleShape,
    cx: f32, cy: f32, r: f32,
    ctx: &mut RenderContext,
) {
    super::ellipse::render(shape, cx, cy, r, r, ctx);
}
