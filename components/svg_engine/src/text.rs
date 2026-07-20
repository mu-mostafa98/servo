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
    /// Pre-shaped glyph positions (from font subsystem). If empty, falls back
    /// to estimated rectangle rendering.
    pub glyphs: Vec<ShapedGlyph>,
    /// Text alignment anchor.
    pub text_anchor: TextAnchor,
    // TODO: font_instance_key for glyph rendering
    // pub font_instance_key: Option<webrender_api::FontInstanceKey>,
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
