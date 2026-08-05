/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Simple shape emitter — dispatches rect/circle/ellipse/line to paint commands.
//!
//! Shapes with solid-color fills/strokes produce lightweight [`PaintCommand`]
//! records. Shapes with gradient paints are rasterized via vello_cpu and
//! emitted as [`PaintCommand::DrawImage`].

use usvg::SimpleShapeKind;

use vello_cpu::kurbo::BezPath;
use vello_cpu::{Pixmap, RenderContext, Resources};

use super::{
    color_from_usvg, convert_linear_gradient, convert_radial_gradient,
    gradient_fallback_color, is_gradient_paint, PaintTransform, Emit, EmitContext,
    PaintColor, PaintCommand, RoundedClip, RoundedRadii,
};

// ======================= Emit impl =======================

impl Emit for usvg::SimpleShape {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }

        // Check if any paint uses a gradient — if so, rasterize via vello_cpu.
        let has_gradient_fill = self.fill().map_or(false, |f| is_gradient_paint(f.paint()));
        let has_gradient_stroke = self.stroke().map_or(false, |s| is_gradient_paint(s.paint()));

        if has_gradient_fill || has_gradient_stroke {
            rasterize_simple_shape_with_gradient(self, ctx, commands);
            return;
        }

        // Fast path: solid-color paints only.
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

// ======================= Gradient Rasterization =======================

/// Rasterize a simple shape that uses gradient fill and/or stroke.
fn rasterize_simple_shape_with_gradient(
    shape: &usvg::SimpleShape,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    let b = shape.abs_bounding_box();
    let w = (b.width().ceil() as u16).max(1);
    let h = (b.height().ceil() as u16).max(1);

    let mut context = RenderContext::new(w, h);
    let mut resources = Resources::new();
    let mut target = Pixmap::new(w, h);

    let bx = b.x() as f64;
    let by = b.y() as f64;

    // Fallback color from fill or stroke.
    let fallback = shape.fill().and_then(|f| match f.paint() {
        usvg::Paint::Color(c) => Some(color_from_usvg(c, f.opacity().get())),
        usvg::Paint::LinearGradient(lg) => Some(gradient_fallback_color(lg.stops())),
        usvg::Paint::RadialGradient(rg) => Some(gradient_fallback_color(rg.stops())),
        _ => None,
    }).or_else(|| shape.stroke().and_then(|s| match s.paint() {
        usvg::Paint::Color(c) => Some(color_from_usvg(c, s.opacity().get())),
        usvg::Paint::LinearGradient(lg) => Some(gradient_fallback_color(lg.stops())),
        usvg::Paint::RadialGradient(rg) => Some(gradient_fallback_color(rg.stops())),
        _ => None,
    })).unwrap_or(PaintColor { r: 0.5, g: 0.5, b: 0.5, a: 1.0 });

    // Build the shape as a BezPath in pixmap-local coordinates.
    match shape.kind() {
        SimpleShapeKind::Rect { x, y, width, height, rx, ry } => {
            let r = rx.or(*ry).unwrap_or(0.0);
            let ry_val = ry.or(*rx).unwrap_or(r);
            draw_rect_path(&mut context, shape, *x, *y, *width, *height, r, ry_val, bx, by, b);
        }
        SimpleShapeKind::Circle { cx, cy, r } => {
            let x = cx - r;
            let y = cy - r;
            draw_ellipse_path(&mut context, shape, x, y, r * 2.0, r * 2.0, *r, *r, bx, by, b);
        }
        SimpleShapeKind::Ellipse { cx, cy, rx, ry } => {
            let x = cx - rx;
            let y = cy - ry;
            draw_ellipse_path(&mut context, shape, x, y, rx * 2.0, ry * 2.0, *rx, *ry, bx, by, b);
        }
        SimpleShapeKind::Line { x1, y1, x2, y2 } => {
            draw_line_path(&mut context, shape, *x1, *y1, *x2, *y2, bx, by, b);
        }
    }

    context.flush();
    context.render_to_pixmap(&mut resources, &mut target);

    let rgba: Vec<u8> = target.data().iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();

    commands.push(PaintCommand::DrawImage {
        x: ctx.svg_origin.x + b.x(),
        y: ctx.svg_origin.y + b.y(),
        w: w as u32,
        h: h as u32,
        data: rgba,
        fallback_color: fallback,
    });
}

/// Set the appropriate paint on the RenderContext: solid color or gradient.
/// Returns an optional [`PaintTransform`] for radial gradients on non-square bboxes.
fn set_shape_paint(
    context: &mut RenderContext,
    paint: &usvg::Paint,
    opacity: f32,
    bbox: usvg::Rect,
) -> Option<PaintTransform> {
    match paint {
        usvg::Paint::Color(c) => {
            let vc = vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::from_rgba8(
                c.red, c.green, c.blue, (opacity * 255.0) as u8,
            );
            context.set_paint(vc);
            None
        }
        usvg::Paint::LinearGradient(lg) => {
            let g = convert_linear_gradient(lg, bbox);
            context.set_paint(g);
            None
        }
        usvg::Paint::RadialGradient(rg) => {
            let (g, pt) = convert_radial_gradient(rg, bbox);
            context.set_paint(g);
            pt
        }
        _ => {
            context.set_paint(vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::from_rgba8(128, 128, 128, 255));
            None
        }
    }
}

// ======================= Shape Drawing Helpers =======================

fn draw_rect_path(
    context: &mut RenderContext,
    shape: &usvg::SimpleShape,
    x: f32, y: f32, w: f32, h: f32,
    rx: f32, ry: f32,
    bx: f64, by: f64,
    bbox: usvg::Rect,
) {
    let local_x = (x as f64 - bx) as f64;
    let local_y = (y as f64 - by) as f64;
    let lw = w as f64;
    let lh = h as f64;
    let lrx = rx.min(w / 2.0) as f64;
    let lry = ry.min(h / 2.0) as f64;

    let path = if lrx > 0.0 || lry > 0.0 {
        rounded_rect_bezpath(local_x, local_y, lw, lh, lrx, lry)
    } else {
        let mut bez = BezPath::new();
        bez.move_to((local_x, local_y));
        bez.line_to((local_x + lw, local_y));
        bez.line_to((local_x + lw, local_y + lh));
        bez.line_to((local_x, local_y + lh));
        bez.close_path();
        bez
    };

    // Fill
    if let Some(fill) = shape.fill() {
        let pt = set_shape_paint(context, fill.paint(), fill.opacity().get(), bbox);
        if let Some(ref pt) = pt { pt.apply(context); }
        context.fill_path(&path);
        if pt.is_some() { context.reset_paint_transform(); }
    }

    // Stroke
    if let Some(stroke) = shape.stroke() {
        let pt = set_shape_paint(context, stroke.paint(), stroke.opacity().get(), bbox);
        if let Some(ref pt) = pt { pt.apply(context); }
        let sw = stroke.width().get() as f64;
        let vello_stroke = vello_cpu::kurbo::Stroke::new(sw);
        context.set_stroke(vello_stroke);
        context.stroke_path(&path);
        if pt.is_some() { context.reset_paint_transform(); }
    }
}

fn draw_ellipse_path(
    context: &mut RenderContext,
    shape: &usvg::SimpleShape,
    x: f32, y: f32, w: f32, h: f32,
    rx: f32, ry: f32,
    bx: f64, by: f64,
    bbox: usvg::Rect,
) {
    let local_x = (x as f64 - bx) as f64;
    let local_y = (y as f64 - by) as f64;
    let lw = w as f64;
    let lh = h as f64;
    let lrx = (rx as f64).max(0.0);
    let lry = (ry as f64).max(0.0);

    // Use four cubic Beziers to approximate an ellipse (magic constant ≈ 0.55228).
    let k = 0.5522847498;
    let cx = local_x + lw / 2.0;
    let cy = local_y + lh / 2.0;
    let kx = k * lrx;
    let ky = k * lry;

    let mut bez = BezPath::new();
    bez.move_to((cx + lrx, cy));
    bez.curve_to((cx + lrx, cy - ky), (cx + kx, cy - lry), (cx, cy - lry));
    bez.curve_to((cx - kx, cy - lry), (cx - lrx, cy - ky), (cx - lrx, cy));
    bez.curve_to((cx - lrx, cy + ky), (cx - kx, cy + lry), (cx, cy + lry));
    bez.curve_to((cx + kx, cy + lry), (cx + lrx, cy + ky), (cx + lrx, cy));
    bez.close_path();

    // Fill
    if let Some(fill) = shape.fill() {
        let pt = set_shape_paint(context, fill.paint(), fill.opacity().get(), bbox);
        if let Some(ref pt) = pt { pt.apply(context); }
        context.fill_path(&bez);
        if pt.is_some() { context.reset_paint_transform(); }
    }

    // Stroke
    if let Some(stroke) = shape.stroke() {
        let pt = set_shape_paint(context, stroke.paint(), stroke.opacity().get(), bbox);
        if let Some(ref pt) = pt { pt.apply(context); }
        let sw = stroke.width().get() as f64;
        let vello_stroke = vello_cpu::kurbo::Stroke::new(sw);
        context.set_stroke(vello_stroke);
        context.stroke_path(&bez);
        if pt.is_some() { context.reset_paint_transform(); }
    }
}

fn draw_line_path(
    context: &mut RenderContext,
    shape: &usvg::SimpleShape,
    x1: f32, y1: f32, x2: f32, y2: f32,
    bx: f64, by: f64,
    bbox: usvg::Rect,
) {
    let local_x1 = x1 as f64 - bx;
    let local_y1 = y1 as f64 - by;
    let local_x2 = x2 as f64 - bx;
    let local_y2 = y2 as f64 - by;

    let mut bez = BezPath::new();
    bez.move_to((local_x1, local_y1));
    bez.line_to((local_x2, local_y2));

    // Lines only have stroke.
    if let Some(stroke) = shape.stroke() {
        let pt = set_shape_paint(context, stroke.paint(), stroke.opacity().get(), bbox);
        if let Some(ref pt) = pt { pt.apply(context); }
        let sw = stroke.width().get() as f64;
        let vello_stroke = vello_cpu::kurbo::Stroke::new(sw);
        context.set_stroke(vello_stroke);
        context.stroke_path(&bez);
        if pt.is_some() { context.reset_paint_transform(); }
    }
}

fn rounded_rect_bezpath(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> BezPath {
    let k = 0.5522847498; // magic constant for circular arc approximation
    let mut bez = BezPath::new();

    bez.move_to((x + rx, y));
    bez.line_to((x + w - rx, y));
    bez.curve_to((x + w - rx + k * rx, y), (x + w, y + ry - k * ry), (x + w, y + ry));
    bez.line_to((x + w, y + h - ry));
    bez.curve_to((x + w, y + h - ry + k * ry), (x + w - rx + k * rx, y + h), (x + w - rx, y + h));
    bez.line_to((x + rx, y + h));
    bez.curve_to((x + rx - k * rx, y + h), (x, y + h - ry + k * ry), (x, y + h - ry));
    bez.line_to((x, y + ry));
    bez.curve_to((x, y + ry - k * ry), (x + rx - k * rx, y), (x + rx, y));
    bez.close_path();

    bez
}

// ======================= Solid-Color Fast Path =======================

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

fn emit_ellipse(
    shape: &usvg::SimpleShape,
    x: f32, y: f32, w: f32, h: f32,
    rx: f32, ry: f32,
    ctx: &EmitContext,
    commands: &mut Vec<PaintCommand>,
) {
    emit_rect(shape, x, y, w, h, rx, ry, ctx, commands);
}

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
