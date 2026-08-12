/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Image emitter — renders usvg::Image nodes via vello_cpu + upload pipeline.

use vello_cpu::kurbo::Rect as KrRect;
use vello_cpu::{Pixmap, RenderContext, RenderSettings, Resources};

use super::{Emit, EmitContext, PaintColor, PaintCommand};

impl Emit for usvg::Image {
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>) {
        if !self.is_visible() {
            return;
        }
        let b = self.abs_bounding_box();
        let ox = ctx.svg_origin.x + b.x();
        let oy = ctx.svg_origin.y + b.y();
        let w = (b.width().ceil() as u16).max(1);
        let h = (b.height().ceil() as u16).max(1);

        // Use element bounds for output size (image scales to fit)
        let out_w = w as u32;
        let out_h = h as u32;
        let (src_rgba, src_w, src_h) = match self.kind() {
            usvg::ImageKind::PNG(data) => {
                if let Some(decoded) = decode_png_to_rgba(data) {
                    (decoded.rgba, decoded.w, decoded.h)
                } else {
                    (vec![0u8; 4], 1, 1)
                }
            }
            usvg::ImageKind::JPEG(_) | usvg::ImageKind::GIF(_) | usvg::ImageKind::WEBP(_) => {
                (vec![0u8; (w as u32 * h as u32 * 4) as usize], w as u32, h as u32)
            }
            usvg::ImageKind::SVG(_tree) => {
                let mut context = RenderContext::new_with(w, h, RenderSettings {
                    num_threads: 0,
                    ..Default::default()
                });
                let mut resources = Resources::new();
                let mut target = Pixmap::new(w, h);
                let rect = KrRect::from_origin_size((0.0, 0.0), (w as f64, h as f64));
                context.set_paint(vello_cpu::color::palette::css::LIGHT_BLUE);
                context.fill_rect(&rect);
                context.flush();
                context.render_to_pixmap(&mut resources, &mut target);
                (target.data().iter().flat_map(|p| [p.r,p.g,p.b,p.a]).collect(), w as u32, h as u32)
            }
        };

        // Scale source to output dimensions (nearest-neighbor)
        let rgba = if src_w == out_w && src_h == out_h {
            src_rgba
        } else if src_w > 0 && src_h > 0 {
            let mut scaled = Vec::with_capacity((out_w * out_h * 4) as usize);
            for y in 0..out_h {
                let sy = (y as u64 * src_h as u64 / out_h as u64) as usize;
                for x in 0..out_w {
                    let sx = (x as u64 * src_w as u64 / out_w as u64) as usize;
                    let si = (sy * src_w as usize + sx) * 4;
                    scaled.extend_from_slice(&src_rgba[si..si+4]);
                }
            }
            scaled
        } else {
            src_rgba
        };

        commands.push(PaintCommand::DrawImage {
            x: ox, y: oy, w: out_w, h: out_h, data: rgba,
            fallback_color: PaintColor { r: 0.5, g: 0.7, b: 0.9, a: 1.0 },
        });
    }
}

struct DecodedImage {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
}

fn decode_png_to_rgba(data: &[u8]) -> Option<DecodedImage> {
    use png::Decoder;
    let decoder = Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut buf).ok()?;
    let info = reader.info();
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let pixel_count = buf.len() / 3;
            let mut rgba = Vec::with_capacity(pixel_count * 4);
            for chunk in buf.chunks(3) {
                rgba.push(chunk[0]); rgba.push(chunk[1]); rgba.push(chunk[2]); rgba.push(255);
            }
            rgba
        }
        _ => return None,
    };
    Some(DecodedImage { rgba, w: info.width, h: info.height })
}
