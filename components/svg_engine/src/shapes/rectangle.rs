/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// TODO: will be restored in future PRs
// use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
// use crate::render_tree::ClipPathUnits;
// use crate::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE, all_equal_radius};

/// SVG `<rect>` element.
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rx: Option<f32>,
    pub ry: Option<f32>,
}

// TODO: clip_info() will be restored in future PRs
//
// impl Rectangle {
//     /// Clip geometry for this rectangle.
//     pub(crate) fn clip_info(
//         &self,
//         svg_origin: &LayoutPoint,
//         units: ClipPathUnits,
//     ) -> Option<ClipGeometry> {
//         ...
//     }
// }
