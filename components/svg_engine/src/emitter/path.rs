/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Path emitter — rasterizes usvg paths via Vello CPU with bitmap caching.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use vello_cpu::kurbo::BezPath;
use vello_cpu::{Pixmap, RenderContext, Resources};

use super::{Emit, EmitContext, PaintCommand};

// ======================= Cache =======================

#[allow(dead_code)]
pub(crate) struct BitmapCache {
    entries: HashMap<u64, Pixmap>,
}

#[allow(dead_code)]
impl BitmapCache {
    pub fn new() -> Self {
        BitmapCache { entries: HashMap::new() }
    }
}

#[allow(dead_code)]
fn path_key(path: &usvg::Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.data().len().hash(&mut h);
    if let Some(f) = path.fill() {
        if let usvg::Paint::Color(c) = f.paint() {
            c.red.hash(&mut h);
            c.green.hash(&mut h);
            c.blue.hash(&mut h);
        }
    }
    h.finish()
}

fn vello_color(c: &usvg::Color) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
    vello_cpu::color::AlphaColor::from_rgb8(c.red, c.green, c.blue)
}

fn to_bezpath(data: &usvg::tiny_skia_path::Path) -> BezPath {
    let mut bez = BezPath::new();
    for seg in data.segments() {
        match seg {
            usvg::tiny_skia_path::PathSegment::MoveTo(p) =>
                bez.move_to((p.x as f64, p.y as f64)),
            usvg::tiny_skia_path::PathSegment::LineTo(p) =>
                bez.line_to((p.x as f64, p.y as f64)),
            usvg::tiny_skia_path::PathSegment::QuadTo(p1, p) =>
                bez.quad_to((p1.x as f64, p1.y as f64), (p.x as f64, p.y as f64)),
            usvg::tiny_skia_path::PathSegment::CubicTo(p1, p2, p) =>
                bez.curve_to((p1.x as f64, p1.y as f64), (p2.x as f64, p2.y as f64), (p.x as f64, p.y as f64)),
            usvg::tiny_skia_path::PathSegment::Close =>
                bez.close_path(),
        }
    }
    bez
}


// ======================= Emit impl =======================

impl Emit for usvg::Path {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }

        let b = self.abs_bounding_box();

        let w = (b.width().ceil() as u16).max(1);
        let h = (b.height().ceil() as u16).max(1);

        let mut context = RenderContext::new(w, h);
        let mut resources = Resources::new();
        let mut target = Pixmap::new(w, h);

        // Convert path to kurbo BezPath, offset to fit within (0,0)-(w,h)
        let mut bez = to_bezpath(self.data());
        let bx = b.x() as f64;
        let by = b.y() as f64;
        bez.apply_affine(vello_cpu::kurbo::Affine::translate((-bx, -by)));

        // Fill
        if let Some(fill) = self.fill() {
            if let usvg::Paint::Color(c) = fill.paint() {
                context.set_paint(vello_color(c));
                context.fill_path(&bez);
            }
        }

        // Stroke
        if let Some(stroke) = self.stroke() {
            if let usvg::Paint::Color(c) = stroke.paint() {
                let sw = stroke.width().get() as f64;
                let vello_stroke = vello_cpu::kurbo::Stroke::new(sw);
                context.set_stroke(vello_stroke);
                context.set_paint(vello_color(c));
                context.stroke_path(&bez);
            }
        }

        context.flush();
        context.render_to_pixmap(&mut resources, &mut target);

        let rgba: Vec<u8> = target.data().iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();

        // Fallback color from first fill or stroke
        let fallback = self.fill().and_then(|f| match f.paint() {
            usvg::Paint::Color(c) => Some(super::color_from_usvg(c, f.opacity().get())),
            _ => None,
        }).unwrap_or_else(|| super::PaintColor { r: 0.5, g: 0.5, b: 0.5, a: 1.0 });

        commands.push(PaintCommand::DrawImage {
            x: ctx.svg_origin.x + b.x(),
            y: ctx.svg_origin.y + b.y(),
            w: w as u32,
            h: h as u32,
            data: rgba,
            fallback_color: fallback,
        });
    }
}
