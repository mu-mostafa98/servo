/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Simple shape emitter — dispatches rect/circle/ellipse/line to paint commands.

use usvg::SimpleShapeKind;

use super::{color_from_usvg, Emit, EmitContext, PaintCommand, RoundedClip, RoundedRadii};

impl Emit for usvg::SimpleShape {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }
        match self.kind() {
            SimpleShapeKind::Rect { x, y, width, height, rx, ry } => {
                let r = rx.or(*ry).unwrap_or(0.0);
                emit_rect(self, *x, *y, *width, *height, r, ry.or(*rx).unwrap_or(r), ctx, commands);
            }
            SimpleShapeKind::Circle { cx, cy, r } =>
                emit_ellipse(self, cx - r, cy - r, r * 2.0, r * 2.0, *r, *r, ctx, commands),
            SimpleShapeKind::Ellipse { cx, cy, rx, ry } =>
                emit_ellipse(self, cx - rx, cy - ry, rx * 2.0, ry * 2.0, *rx, *ry, ctx, commands),
            SimpleShapeKind::Line { x1, y1, x2, y2 } =>
                emit_line(self, *x1, *y1, *x2, *y2, ctx, commands),
        }
    }
}

// ======================= Rect =======================

fn emit_rect(
    shape: &usvg::SimpleShape,
    x: f32, y: f32, w: f32, h: f32,
    rx: f32, ry: f32,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let bounds = super::FillRectBounds {
        x: ctx.svg_origin.x + x,
        y: ctx.svg_origin.y + y,
        w, h,
    };

    let has_radius = rx > 0.0 || ry > 0.0;
    let clip = has_radius.then_some(RoundedClip {
        rx: rx.clamp(0.0, w / 2.0),
        ry: ry.clamp(0.0, h / 2.0),
    });

    // Fill
    if let Some(fill) = shape.fill() {
        if let usvg::Paint::Color(c) = fill.paint() {
            let color = color_from_usvg(c, fill.opacity().get());
            commands.push(PaintCommand::FillRect { bounds, color, clip });
        }
    }

    // Stroke
    if let Some(stroke) = shape.stroke() {
        if let usvg::Paint::Color(c) = stroke.paint() {
            let color = color_from_usvg(c, stroke.opacity().get());
            let sw = stroke.width().get();
            commands.push(PaintCommand::StrokeRect {
                bounds,
                color,
                width: sw,
                radii: has_radius.then_some(RoundedRadii {
                    rx: rx.clamp(0.0, w / 2.0),
                    ry: ry.clamp(0.0, h / 2.0),
                }),
            });
        }
    }
}

// ======================= Ellipse (circle delegates here) =======================

fn emit_ellipse(
    shape: &usvg::SimpleShape,
    x: f32, y: f32, w: f32, h: f32,
    rx: f32, ry: f32,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    // Ellipse = rect with 100% corner radii
    emit_rect(shape, x, y, w, h, rx, ry, ctx, commands);
}

// ======================= Line =======================

fn emit_line(
    shape: &usvg::SimpleShape,
    x1: f32, y1: f32, x2: f32, y2: f32,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 0.01 {
        return;
    }

    let stroke = match shape.stroke() {
        Some(s) => s,
        None => return,
    };
    let c = match stroke.paint() {
        usvg::Paint::Color(c) => c,
        _ => return,
    };
    let color = color_from_usvg(c, stroke.opacity().get());
    let sw = stroke.width().get();

    commands.push(PaintCommand::StrokeLine {
        x1: ctx.svg_origin.x + x1,
        y1: ctx.svg_origin.y + y1,
        x2: ctx.svg_origin.x + x2,
        y2: ctx.svg_origin.y + y2,
        color,
        width: sw,
    });
}
