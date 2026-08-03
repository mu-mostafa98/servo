/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Image emitter — renders usvg::Image nodes.

use super::{Emit, EmitContext, FillRectBounds, PaintColor, PaintCommand};

impl Emit for usvg::Image {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }
        let b = self.abs_bounding_box();
        commands.push(PaintCommand::FillRect {
            bounds: FillRectBounds {
                x: ctx.svg_origin.x + b.x(),
                y: ctx.svg_origin.y + b.y(),
                w: b.width().max(1.0),
                h: b.height().max(1.0),
            },
            color: PaintColor { r: 0.8, g: 0.8, b: 0.2, a: 0.6 },
            clip: None,
        });
    }
}
