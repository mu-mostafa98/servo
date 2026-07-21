/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone)]
pub struct SvgImage {
    // pub x: f32,
    // pub y: f32,
    // pub width: f32,
    // pub height: f32,
    /// The `href` (or `xlink:href`) attribute value — may be a URL or data URI.
    pub href: Option<String>,
}
