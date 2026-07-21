/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[derive(Debug, Clone)]
pub struct TextSpan {
    pub text: String,
    // pub x: f32,
    // pub y: f32,
    // pub dx: Vec<f32>,
    // pub dy: Vec<f32>,
    // pub glyphs: Vec<ShapedGlyph>,
    // pub text_anchor: TextAnchor,
    // pub font_instance_key: Option<webrender_api::FontInstanceKey>,
}

// TODO: implement TextAnchor enum and alignment_offset()
