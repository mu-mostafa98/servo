/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<text>` element — text content with positioning.
//!
//! Text spec: <https://www.w3.org/TR/SVG2/text.html>

use crate::model::resource::ResourceKey;

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
    pub font_instance_key: Option<ResourceKey>,
}

/// A single text span within an SVG `<text>` or `<tspan>` element.
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// The text content.
    pub text: String,
    /// Per-character absolute X coordinates (SVG `x`, §11.5.2). An empty list
    /// means "no explicit X": the text flows from the current position. Each
    /// listed value repositions the current text position for the matching
    /// character; characters beyond the list keep accumulating normally.
    pub x: Vec<f32>,
    /// Per-character absolute Y coordinates (SVG `y`), like [`Self::x`].
    pub y: Vec<f32>,
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
    /// Opaque font instance key for glyph rendering.
    /// When `Some`, the renderer uses `push_text` for real glyph shapes.
    pub font_instance_key: Option<ResourceKey>,
    /// Horizontal pen offset accumulated from preceding sibling runs in the
    /// same `<text>` inline flow. Set by the builder so that a run begins where
    /// the previous run ended. For a standalone `<text>` (no tspans) this is
    /// `0.0`; for the first run it carries the whole-line `text-anchor` shift.
    pub advance_offset: f32,
    /// Resolved font size in CSS pixels, captured during shaping. Used by the
    /// renderer to size the glyph clip rect's ascent/descent so glyphs are not
    /// clipped when the font is larger than the fallback height estimate.
    pub font_size: f32,
}

impl TextSpan {
    /// The span's X origin — the first `x` value, or `0.0` when no explicit X.
    pub fn origin_x(&self) -> f32 {
        self.x.first().copied().unwrap_or(0.0)
    }

    /// The span's Y origin — the first `y` value, or `0.0` when no explicit Y.
    pub fn origin_y(&self) -> f32 {
        self.y.first().copied().unwrap_or(0.0)
    }

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
    /// Top of the em box (`text-before-edge`).
    TextBeforeEdge,
    /// Bottom of the em box (`text-after-edge`).
    TextAfterEdge,
    /// Hanging baseline (top of the em box, for scripts such as Devanagari).
    Hanging,
    /// Middle of the x-height (a little above `Central`).
    Middle,
    /// Center of the em box.
    Central,
    /// Ideographic baseline (bottom of the ideographic em box).
    Ideographic,
    /// Alphabetic baseline (explicit `alphabetic`, same as the default).
    Alphabetic,
    /// Mathematical baseline (center of the math em box).
    Mathematical,
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
