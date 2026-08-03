/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Text emitter — placeholder rendering.
//! TODO: Wire font shaping (HarfBuzz/fontdb) for proper glyph rendering.

use super::{Emit, EmitContext, FillRectBounds, PaintColor, PaintCommand};

impl Emit for usvg::Text {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        let ox = ctx.svg_origin.x;
        let oy = ctx.svg_origin.y;
        // Always draw a visible placeholder — text shaping requires fontdb
        commands.push(PaintCommand::FillRect {
            bounds: FillRectBounds { x: ox + 10.0, y: oy + 10.0, w: 80.0, h: 16.0 },
            color: PaintColor { r: 0.4, g: 0.4, b: 0.6, a: 0.8 },
            clip: None,
        });
    }
}
