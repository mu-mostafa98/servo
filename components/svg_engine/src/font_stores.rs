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
//!   once, during SVG tree construction, using Servo's `FontContext` (which
//!   handles font fallback and registers the font with WebRender's resource
//!   cache). `FontContext` caches keys internally, so re-resolving the same
//!   font on a later frame is cheap.
//! - [`GlyphStore`] — maps a handle to the pre-shaped glyph data (glyph IDs +
//!   positions + total advance) produced during construction using Servo's
//!   font APIs (`font.glyph_index`, `font.glyph_h_advance`).
//!
//! Both tables are filled by the integration layer (`layout::svg`) and read by
//! the emitter + WebRender backend during rendering. They live for the lifetime
//! of one `Arc<usvg::Tree>` and are bundled with it in
//! [`crate::SvgRenderData`].

use std::collections::HashMap;
use std::sync::Arc;

use webrender_api::FontInstanceKey;

/// A pre-shaped glyph positioned for rendering.
///
/// Coordinates are relative to the span's text origin (in SVG user units,
/// already scaled to the requested font size).
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID within the font, as returned by `fonts::Font::glyph_index`.
    /// This is the value WebRender's `GlyphInstance::index` expects.
    pub glyph_id: u32,
    /// X position of this glyph (cumulative — includes preceding advances).
    pub x: f32,
    /// Y position (baseline-relative).
    pub y: f32,
    /// Horizontal advance to the next glyph.
    pub advance: f32,
}

/// Per-span shaped-glyph data, looked up by the same handle as the
/// [`FontKeyRegistry`] entry.
#[derive(Debug, Clone)]
pub struct ShapedSpan {
    /// Pre-shaped glyphs in render order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Total horizontal advance of all glyphs (for text-anchor alignment).
    pub total_advance: f32,
    /// Font size in CSS pixels (used for the glyph clip bounds).
    pub font_size: f32,
}

/// Maps opaque font handles (stored on usvg text spans) to WebRender
/// [`FontInstanceKey`]s resolved via Servo's `FontContext`.
#[derive(Debug, Default)]
pub struct FontKeyRegistry {
    next_handle: usize,
    map: HashMap<usize, FontInstanceKey>,
}

impl FontKeyRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resolved `FontInstanceKey` and returns the opaque handle
    /// that the integration layer should store on the usvg text span.
    pub fn register(&mut self, key: FontInstanceKey) -> usize {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.map.insert(handle, key);
        handle
    }

    /// Looks up the `FontInstanceKey` for a handle. Returns `None` if the
    /// handle was never registered (which means the renderer should fall back
    /// to path-based text).
    pub fn lookup(&self, handle: usize) -> Option<FontInstanceKey> {
        self.map.get(&handle).copied()
    }
}

/// Maps opaque font handles to pre-shaped glyph data, produced during SVG
/// tree construction using Servo's font APIs.
#[derive(Debug, Default)]
pub struct GlyphStore {
    map: HashMap<usize, ShapedSpan>,
}

impl GlyphStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores shaped glyphs for a handle (the same handle returned by
    /// [`FontKeyRegistry::register`]).
    pub fn insert(&mut self, handle: usize, span: ShapedSpan) {
        self.map.insert(handle, span);
    }

    /// Looks up the shaped glyphs for a handle. Returns `None` if no entry
    /// exists (the renderer should fall back to path-based text).
    pub fn get(&self, handle: usize) -> Option<&ShapedSpan> {
        self.map.get(&handle)
    }
}

/// A bundle of everything the renderer needs to render an SVG tree's text
/// using native glyphs: the usvg tree plus the two side-tables that give the
/// `font_handle` fields on its text spans their meaning.
///
/// This is what the integration layer attaches to the fragment in place of a
/// bare `Arc<usvg::Tree>`. usvg remains a pure data model with no WebRender
/// dependency; the handles on its text spans are only ever resolved here.
pub struct SvgRenderData {
    /// The usvg tree (shapes + text nodes carrying `font_handle` tokens).
    pub tree: Arc<usvg::Tree>,
    /// Handle → `FontInstanceKey` (WebRender backend only).
    pub font_keys: Arc<FontKeyRegistry>,
    /// Handle → pre-shaped glyphs (emitter reads these for simple text).
    pub glyphs: Arc<GlyphStore>,
}

impl SvgRenderData {
    /// Creates an empty data bundle wrapping the given tree with no resolved
    /// fonts — all text will fall back to path-based rendering.
    pub fn new(tree: Arc<usvg::Tree>) -> Self {
        Self {
            tree,
            font_keys: Arc::new(FontKeyRegistry::new()),
            glyphs: Arc::new(GlyphStore::new()),
        }
    }
}
