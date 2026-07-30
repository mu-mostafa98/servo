/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<ellipse>` renderer — delegates to rect with 100% corner radii.

use super::RenderContext;

/// Render an SVG `<ellipse>` via rect delegation (100% corner radii).
pub(crate) fn render(
    shape: &usvg::SimpleShape,
    cx: f32, cy: f32, rx: f32, ry: f32,
    ctx: &mut RenderContext,
) {
    let x = cx - rx;
    let y = cy - ry;
    let width = rx * 2.0;
    let height = ry * 2.0;
    super::rect::render(shape, x, y, width, height, Some(rx), Some(ry), ctx);
}
