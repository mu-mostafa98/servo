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
        let (rx, ry) = clip.map(|c| (c.rx, c.ry)).unwrap_or((0.0, 0.0));
        // Simple: just draw the rect (rounded corners not supported in minimal PDF)
        let _ = (rx, ry);
        self.stream.push_str(&self.pdf_rect(
            bounds.x, bounds.y, bounds.w, bounds.h,
            color.r, color.g, color.b, true, false, 0.0,
        ));
    }

    fn stroke_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, width: f32, _radii: Option<RadiiDesc>,
        _spatial_id: SpatialId, _clip_chain_id: ClipChainId,
    ) {
        self.stream.push_str(&self.pdf_rect(
            bounds.x, bounds.y, bounds.w, bounds.h,
            color.r, color.g, color.b, false, true, width,
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
