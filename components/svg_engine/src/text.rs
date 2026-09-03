/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<text>` element — text content with positioning.
//! Reference: https://svgwg.org/svg2-draft/text.html

/// A pre-shaped glyph with position and advance.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// X position of this glyph (cumulative, includes previous advances).
    pub x: f32,
    /// Y position (baseline).
    pub y: f32,
    /// Advance width to the next glyph.
    pub advance: f32,
    /// Font-internal glyph ID for WebRender's `GlyphInstance`.
    pub glyph_id: u32,
    /// Character this glyph represents.
    pub character: char,
    /// The font this glyph was shaped with. `None` for fallback glyphs that
    /// have no resolved font (they are skipped during rendering). Mixed-script
    /// runs shape different characters with different fonts, so the key is
    /// stored per-glyph rather than once per span.
    pub font_instance_key: Option<webrender_api::FontInstanceKey>,
}

/// A single text span within an SVG `<text>` or `<tspan>` element.
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// The text content.
    pub text: String,
    /// X coordinate of the text anchor point.
    pub x: f32,
    /// Y coordinate (baseline position).
    pub y: f32,
    /// Per-character X offsets (SVG `dx` attribute).
    pub dx: Vec<f32>,
    /// Per-character Y offsets (SVG `dy` attribute).
    pub dy: Vec<f32>,
    /// Per-character rotation angles in degrees (SVG `rotate` attribute).
    pub rotate: Vec<f32>,
    /// Pre-shaped glyph positions (from font subsystem). If empty, falls back
    /// to estimated rectangle rendering.
    pub glyphs: Vec<ShapedGlyph>,
    /// Text alignment anchor.
    pub text_anchor: TextAnchor,
    /// Right-to-left text (`direction="rtl"`). The text is pre-reversed, and
    /// the anchor is mirrored so `start` aligns to the right edge.
    pub rtl: bool,
    /// Vertical baseline alignment (SVG `dominant-baseline`).
    pub dominant_baseline: DominantBaseline,
    /// WebRender font instance key for glyph rendering.
    /// When `Some`, the renderer uses `push_text` for real glyph shapes.
    pub font_instance_key: Option<webrender_api::FontInstanceKey>,
    /// Horizontal pen offset accumulated from preceding sibling runs in the
    /// same `<text>` inline flow. Set by the builder so that a run begins where
    /// the previous run ended. For a standalone `<text>` (no tspans) this is
    /// `0.0`; for the first run it carries the whole-line `text-anchor` shift.
    pub advance_offset: f32,
}

impl TextSpan {
    /// Total advance width of all glyphs in this span (or estimated text
    /// width when no glyphs are shaped yet).
    pub fn total_advance(&self) -> f32 {
        if let Some(last) = self.glyphs.last() {
            let total = last.x + last.advance;
            // Trailing whitespace is skipped during shaping (advance, no
            // glyph), so add its approximate advance back so the following run
            // is placed after the space.
            let trailing_ws = self
                .text
                .chars()
                .rev()
                .take_while(|c| c.is_whitespace())
                .count();
            total + trailing_ws as f32 * 4.0
        } else {
            self.text.chars().count() as f32 * 8.0
        }
    }
}

/// Vertical alignment of the text relative to the `y` coordinate
/// (SVG `dominant-baseline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DominantBaseline {
    /// Alphabetic baseline (the default).
    #[default]
    Auto,
    /// Hanging baseline (top of the em box).
    Hanging,
    /// Middle of the em box.
    Middle,
    /// Central baseline (middle of the em box, similar to `Middle`).
    Central,
}

/// Text alignment anchor point.
///
/// Controls how the text string is positioned relative to the `x` coordinate:
/// - `Start`: left-aligned (default for LTR text)
/// - `Middle`: centered on `x`
/// - `End`: right-aligned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

impl TextAnchor {
    /// Horizontal offset to apply so the text anchor aligns correctly.
    /// Returns a multiplier: `total_width * offset` gives the translation.
    pub fn alignment_offset(&self) -> f32 {
        match self {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => -0.5,
            TextAnchor::End => -1.0,
        }
    }
}
