/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Numeric caps on SVG geometry and expansion (security layer 4).
//!
//! A malicious or pathological SVG can turn a tiny source into unbounded work
//! at render time: a `viewBox` scaled up to enormous dimensions, a `d` path
//! with millions of segments, a `points` list with millions of entries, a
//! stroke width in the thousands, or a filter `stdDeviation` that forces a
//! huge Gaussian kernel. These constants clamp the inputs at the DOM →
//! render-tree boundary, so the renderer always sees bounded geometry.
//!
//! The values sit far above anything a legitimate document needs but low
//! enough that the worst case stays within a few megabytes of tessellation
//! and a few million output pixels.

/// Maximum width or height of a `viewBox` / nested viewport, in user units.
/// `1e7` keeps the coordinate space well inside `f32`'s precision and bounds
/// any proportional scaling done downstream.
pub(crate) const MAX_VIEWPORT_DIMENSION: f32 = 1.0e7;

/// Maximum number of path segments accepted from a `d` attribute. A path with
/// more segments than this is rejected and renders nothing.
pub(crate) const MAX_PATH_SEGMENTS: usize = 1_000_000;

/// Maximum number of coordinate pairs accepted from a `points` attribute.
/// Extra pairs past this count are dropped.
pub(crate) const MAX_POINTS: usize = 1_000_000;

/// Maximum stroke width, in user units.
pub(crate) const MAX_STROKE_WIDTH: f32 = 1.0e7;

/// Maximum `stroke-miterlimit`. Miter joins past this behave like a bevel
/// fallback downstream, but capping here keeps the input bounded regardless.
pub(crate) const MAX_MITER_LIMIT: f32 = 1000.0;

/// Maximum number of entries in a `stroke-dasharray`. Extra entries past this
/// count are dropped.
pub(crate) const MAX_DASH_COUNT: usize = 1024;

/// Maximum magnitude of a single dash length or dash offset, in user units.
pub(crate) const MAX_DASH_LENGTH: f32 = 1.0e7;

/// Maximum `stdDeviation` for `feGaussianBlur` / `feDropShadow`. Beyond this
/// the Gaussian kernel is effectively a solid blur, so the value is clamped
/// rather than letting it grow the rasterization cost unboundedly.
pub(crate) const MAX_STD_DEVIATION: f32 = 1024.0;

/// Maximum resolved `font-size`, in CSS pixels. Chrome (Skia) empirically
/// clamps huge font sizes to ~10000 — glyphs at 10000/20000/40000/100000 all
/// rasterize identically — so this matches that ceiling. Glyphs above
/// WebRender's `FONT_SIZE_LIMIT` (320 px) are still routed through the glyph
/// outline (vello CPU) path by the renderer instead of `push_text`, so they
/// never hit the 320 px rasterize-then-upscale banding. The clamp here bounds
/// the font instance, shaping advances, glyph outlines and ascent/descent to a
/// fixed size regardless of the authored `font-size`.
pub(crate) const MAX_FONT_SIZE: f32 = 10000.0;
