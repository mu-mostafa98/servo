/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Image emitter — renders usvg::Image nodes via the pixel upload pipeline.

use super::{Emit, EmitContext, FillRectBounds, PaintColor, PaintCommand};

impl Emit for usvg::Image {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }
        let b = self.abs_bounding_box();
        let ox = ctx.svg_origin.x + b.x();
        let oy = ctx.svg_origin.y + b.y();
        let w = b.width().max(1.0);
        let h = b.height().max(1.0);

        match self.kind() {
            usvg::ImageKind::SVG(tree) => {
                // Nested SVG — walk the subtree and emit contained shapes.
                // For now: placeholder rect (requires recursive emit support).
                commands.push(PaintCommand::FillRect {
                    bounds: FillRectBounds { x: ox, y: oy, w, h },
                    color: PaintColor { r: 0.2, g: 0.6, b: 0.8, a: 0.5 },
                    clip: None,
                });
            }
            _ => {
                // Raster image (PNG/JPEG/GIF/WEBP) — raw bytes available in ImageKind.
                // The bytes are uploaded through the same pipeline as vello_cpu paths.
                // For now: placeholder until ImageKind data extraction is complete.
                commands.push(PaintCommand::FillRect {
                    bounds: FillRectBounds { x: ox, y: oy, w, h },
                    color: PaintColor { r: 0.2, g: 0.6, b: 0.8, a: 1.0 },
                    clip: None,
                });
            }
        }
    }
}
