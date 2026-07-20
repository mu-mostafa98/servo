/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The [`Render`] trait — the core rendering interface for SVG shapes.
//!
//! In future PRs, [`RenderContext`] will carry WebRender display list
//! parameters. Each shape type implements this trait so traversal can
//! call `shape.render(...)` without a central match.

use crate::shapes::Shape;

// TODO: implement RenderContext struct (style, svg_origin, spatial_id,
// clip_chain_id, wr, paints, accumulated_scale)

pub(crate) trait Render {
    fn render(&self);
}

impl Render for Shape {
    fn render(&self) {
        match self {
            Shape::Rect(r) => r.render(),
            Shape::Circle(c) => c.render(),
            Shape::Ellipse(e) => e.render(),
            Shape::Line(l) => l.render(),
            Shape::Polyline(p) => p.render(),
            Shape::Polygon(p) => p.render(),
            Shape::Path(p) => p.render(),
        }
    }
}
