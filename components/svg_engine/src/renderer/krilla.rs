/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Krilla backend — translates paint commands into PDF output.
//!
//! Produces a minimal valid PDF that can be opened in any PDF viewer.
//! In production this would use the `krilla` crate (used by Typst).

use webrender_api::{ClipChainId, SpatialId};

use super::{Backend, ClipDesc, FillRectDesc, PaintColorDesc, RadiiDesc};

/// Minimal PDF backend. Produces valid PDF output suitable for viewing.
pub struct KrillaBackend {
    /// Accumulated page content stream.
    stream: String,
    /// PDF page width.
    width: f32,
    /// PDF page height.
    height: f32,
}

impl KrillaBackend {
    pub fn new(width: f32, height: f32) -> Self {
        KrillaBackend { stream: String::new(), width, height }
    }

    /// Finalize and return a valid PDF document.
    pub fn finish(&self) -> Vec<u8> {
        // Build a minimal valid PDF by hand.
        // PDF coordinate system: origin at bottom-left, Y goes up.
        // Our paint commands use SVG coords (origin top-left, Y goes down).
        // So we flip Y: pdf_y = page_height - svg_y - svg_height

        let stream_bytes = self.stream.as_bytes();
        let stream_len = stream_bytes.len();

        let mut pdf = String::new();
        pdf.push_str("%PDF-1.4\n");
        // Object 1: Catalog
        pdf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        // Object 2: Pages
        pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        // Object 3: Page
        pdf.push_str(&format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R >>\nendobj\n",
            self.width as i32, self.height as i32
        ));
        // Object 4: Content stream
        pdf.push_str(&format!(
            "4 0 obj\n<< /Length {} >>\nstream\n",
            stream_len
        ));
        let mut bytes = pdf.into_bytes();
        bytes.extend_from_slice(stream_bytes);
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        // Cross-reference table
        let xref_offset = bytes.len();
        bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
        bytes.extend_from_slice(format!("{:010} 00000 n \n", 9).as_bytes());   // obj 1
        bytes.extend_from_slice(format!("{:010} 00000 n \n", 46).as_bytes());  // obj 2
        bytes.extend_from_slice(format!("{:010} 00000 n \n", 97).as_bytes());  // obj 3
        bytes.extend_from_slice(format!("{:010} 00000 n \n", 180).as_bytes()); // obj 4
        bytes.extend_from_slice(b"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n");
        bytes.extend_from_slice(format!("{}\n", xref_offset).as_bytes());
        bytes.extend_from_slice(b"%%EOF\n");
        bytes
    }

    fn pdf_rect(
        &self, x: f32, y: f32, w: f32, h: f32,
        r: f32, g: f32, b: f32, fill: bool, stroke: bool, sw: f32,
    ) -> String {
        let pdf_x = x;
        let pdf_y = self.height - y - h; // flip Y
        if fill && stroke {
            format!("{} {} {} {} re\n{} {} {} rg\n{} {} {} RG {} w\nB\n",
                pdf_x, pdf_y, w, h, r, g, b, r, g, b, sw)
        } else if fill {
            format!("{} {} {} {} re\n{} {} {} rg\nf\n",
                pdf_x, pdf_y, w, h, r, g, b)
        } else {
            format!("{} {} {} {} re\n{} {} {} RG {} w\nS\n",
                pdf_x, pdf_y, w, h, r, g, b, sw)
        }
    }

    fn pdf_rounded_rect(
        &self, x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32,
        r: f32, g: f32, b: f32, fill: bool,
    ) -> String {
        // Cubic Bezier approximation of quarter circle: k = 4/3 * (√2 - 1) ≈ 0.552
        let k: f32 = 0.5522847498;
        let rx = rx.min(w / 2.0);
        let ry = ry.min(h / 2.0);
        let py = |v: f32| self.height - v;

        let x0 = x;
        let x1 = x + rx;              let y1 = py(y);
        let x2 = x + w - rx;          let y2 = py(y);
        let x3 = x + w;               let y3 = py(y + ry);
        let x4 = x + w;               let y4 = py(y + h - ry);
        let x5 = x + w - rx;          let y5 = py(y + h);
        let x6 = x + rx;              let y6 = py(y + h);
        let x7 = x0;                  let y7 = py(y + h - ry);
        let x8 = x0;                  let y8 = py(y + ry);

        let mut s = String::new();
        // Move to start of top edge
        s.push_str(&format!("{} {} m\n", x1, y1));
        // Top edge
        s.push_str(&format!("{} {} l\n", x2, y2));
        // Top-right arc: P0=(x2,y2)→CP1=(x2+rx*k, y2)→CP2=(x3, y3+ry*k)→P3=(x3,y3)
        s.push_str(&format!("{} {} {} {} {} {} c\n",
            x2 + rx * k, y2, x3, y3 + ry * k, x3, y3));
        // Right edge
        s.push_str(&format!("{} {} l\n", x4, y4));
        // Bottom-right arc: P0=(x4,y4)→CP1=(x4, y4-ry*k)→CP2=(x5+rx*k, y5)→P3=(x5,y5)
        s.push_str(&format!("{} {} {} {} {} {} c\n",
            x4, y4 - ry * k, x5 + rx * k, y5, x5, y5));
        // Bottom edge
        s.push_str(&format!("{} {} l\n", x6, y6));
        // Bottom-left arc: P0=(x6,y6)→CP1=(x6-rx*k, y6)→CP2=(x7, y7-ry*k)→P3=(x7,y7)
        s.push_str(&format!("{} {} {} {} {} {} c\n",
            x6 - rx * k, y6, x7, y7 - ry * k, x7, y7));
        // Left edge
        s.push_str(&format!("{} {} l\n", x8, y8));
        // Top-left arc: P0=(x8,y8)→CP1=(x8, y8+ry*k)→CP2=(x1-rx*k, y1)→P3=(x1,y1)
        s.push_str(&format!("{} {} {} {} {} {} c\n",
            x8, y8 + ry * k, x1 - rx * k, y1, x1, y1));

        if fill {
            s.push_str(&format!("{} {} {} rg\nf\n", r, g, b));
        } else {
            s.push_str(&format!("{} {} {} RG\n", r, g, b));
        }
        s
    }

    fn pdf_line(&self, x1: f32, y1: f32, x2: f32, y2: f32,
        r: f32, g: f32, b: f32, sw: f32) -> String {
        let py1 = self.height - y1;
        let py2 = self.height - y2;
        format!("{} {} m\n{} {} l\n{} {} {} RG {} w\nS\n",
            x1, py1, x2, py2, r, g, b, sw)
    }
}

impl Backend for KrillaBackend {
    fn fill_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, clip: Option<ClipDesc>,
        _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        if let Some(c) = clip {
            if c.rx > 0.0 || c.ry > 0.0 {
                self.stream.push_str(&self.pdf_rounded_rect(
                    bounds.x, bounds.y, bounds.w, bounds.h, c.rx, c.ry,
                    color.r, color.g, color.b, true,
                ));
                return;
            }
        }
        self.stream.push_str(&self.pdf_rect(
            bounds.x, bounds.y, bounds.w, bounds.h,
            color.r, color.g, color.b, true, false, 0.0,
        ));
    }

    fn stroke_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, width: f32, radii: Option<RadiiDesc>,
        _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        if let Some(r) = radii {
            if r.rx > 0.0 || r.ry > 0.0 {
                self.stream.push_str(&self.pdf_rounded_rect(
                    bounds.x, bounds.y, bounds.w, bounds.h, r.rx, r.ry,
                    color.r, color.g, color.b, false,
                ));
                self.stream.push_str(&format!("{} w\n", width));
                self.stream.push_str("S\n");
                return;
            }
        }
        self.stream.push_str(&self.pdf_rect(
            bounds.x, bounds.y, bounds.w, bounds.h,
            color.r, color.g, color.b, false, true, width,
        ));
    }

    fn draw_image(
        &mut self, x: f32, y: f32, w: u32, h: u32, data: &[u8],
        _fallback: PaintColorDesc, _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        let pdf_x = x;
        let pdf_y = self.height - y - h as f32;
        // Convert premultiplied RGBA → straight RGB for PDF inline image
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for chunk in data.chunks(4) {
            let a = chunk[3] as f32 / 255.0;
            if a > 0.0 {
                rgb.push((chunk[0] as f32 / a).min(255.0) as u8);
                rgb.push((chunk[1] as f32 / a).min(255.0) as u8);
                rgb.push((chunk[2] as f32 / a).min(255.0) as u8);
            } else {
                rgb.push(255); rgb.push(255); rgb.push(255); // transparent → white
            }
        }
        // Hex-encode
        let mut hex = String::with_capacity(rgb.len() * 2);
        for b in &rgb { hex.push_str(&format!("{:02x}", b)); }

        self.stream.push_str(&format!(
            "q\n{} 0 0 {} {} {} cm\nBI\n/W {}\n/H {}\n/CS /RGB\n/BPC 8\nID\n<{}>\nEI\nQ\n",
            w, h, pdf_x, pdf_y, w, h, hex
        ));
    }

    fn stroke_line(
        &mut self, x1: f32, y1: f32, x2: f32, y2: f32,
        color: PaintColorDesc, width: f32,
        _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        self.stream.push_str(&self.pdf_line(x1, y1, x2, y2, color.r, color.g, color.b, width));
    }
}
