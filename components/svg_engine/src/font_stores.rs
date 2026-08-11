/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Font-handle-based text plumbing.
//!
//! usvg's text nodes carry an opaque `font_handle: Option<usize>` (set by the
//! integration layer) so that usvg itself stays free of any WebRender
//! dependency. This module owns the two side-tables that give those handles
//! meaning:
//!
//! - [`FontKeyRegistry`] — maps a handle to a WebRender
//!   [`FontInstanceKey`](webrender_api::FontInstanceKey). The key is resolved
//!   once, during SVG tree construction, using Servo's `FontContext`.
//! - [`GlyphStore`] — maps a handle to pre-shaped glyph data produced
//!   during construction using Servo's font APIs.

use std::collections::HashMap;

use webrender_api::FontInstanceKey;

/// A pre-shaped glyph positioned for rendering.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID within the font. This is the value WebRender's
    /// `GlyphInstance::index` expects.
    pub glyph_id: u32,
    /// X position (cumulative — includes preceding advances).
    pub x: f32,
    /// Y position (baseline-relative).
    pub y: f32,
    /// Horizontal advance to the next glyph.
    pub advance: f32,
}

/// Per-span shaped-glyph data.
#[derive(Debug, Clone)]
pub struct ShapedSpan {
    pub glyphs: Vec<ShapedGlyph>,
    /// Total horizontal advance (for text-anchor alignment).
    pub total_advance: f32,
    /// Font size in CSS pixels.
    pub font_size: f32,
}

/// Maps opaque font handles to WebRender FontInstanceKeys.
#[derive(Debug, Default)]
pub struct FontKeyRegistry {
    next_handle: usize,
    map: HashMap<usize, FontInstanceKey>,
}

impl FontKeyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, key: FontInstanceKey) -> usize {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.map.insert(handle, key);
        handle
    }

    pub fn lookup(&self, handle: usize) -> Option<FontInstanceKey> {
        self.map.get(&handle).copied()
    }
}

/// Maps opaque font handles to pre-shaped glyph data.
#[derive(Debug, Default)]
pub struct GlyphStore {
    map: HashMap<usize, ShapedSpan>,
}

impl GlyphStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: usize, span: ShapedSpan) {
        self.map.insert(handle, span);
    }

    pub fn get(&self, handle: usize) -> Option<&ShapedSpan> {
        self.map.get(&handle)
    }
}

/// Bundle: usvg tree + font side-tables.
#[derive(Debug)]
pub struct SvgRenderData {
    pub tree: std::sync::Arc<usvg::Tree>,
    pub font_keys: std::sync::Arc<FontKeyRegistry>,
    pub glyphs: std::sync::Arc<GlyphStore>,
}

impl SvgRenderData {
    pub fn new(tree: std::sync::Arc<usvg::Tree>) -> Self {
        Self {
            tree,
            font_keys: std::sync::Arc::new(FontKeyRegistry::new()),
            glyphs: std::sync::Arc::new(GlyphStore::new()),
        }
    }
}
