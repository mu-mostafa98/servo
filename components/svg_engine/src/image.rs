/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG `<image>` element — external image rendering.
//! Reference: https://svgwg.org/svg2-draft/embedded.html#ImageElement

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
}
