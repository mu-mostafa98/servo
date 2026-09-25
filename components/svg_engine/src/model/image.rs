/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<image>` element — external image rendering.
//! Reference: https://svgwg.org/svg2-draft/embedded.html#ImageElement

use crate::render_tree::AspectRatio;

/// An SVG `<image>` element referencing an external raster or vector image.
#[derive(Debug, Clone)]
pub struct SvgImage {
    /// X position of the top-left corner.
    pub x: f32,
    /// Y position of the top-left corner.
    pub y: f32,
    /// Width of the rendered image rectangle.
    pub width: f32,
    /// Height of the rendered image rectangle.
    pub height: f32,
    /// The `href` (or `xlink:href`) attribute value — may be a URL or data URI.
    pub href: Option<String>,
    /// The WebRender image key for the decoded raster image, resolved by the
    /// layout layer's image cache during build. `None` when the image has not
    /// yet loaded (or failed to load) — in that case the renderer falls back to
    /// a placeholder. When the image loads, a reflow re-resolves this to `Some`.
    pub image_key: Option<webrender_api::ImageKey>,
    /// Intrinsic pixel width of the loaded raster image. `None` if the image
    /// hasn't loaded yet or is a vector image (no raster metadata available).
    pub natural_width: Option<u32>,
    /// Intrinsic pixel height of the loaded raster image.
    pub natural_height: Option<u32>,
    /// Parsed `preserveAspectRatio` attribute — how the image should be fitted
    /// within its viewport. Defaults to `xMidYMid meet` per the SVG spec.
    pub preserve_aspect_ratio: AspectRatio,
}
