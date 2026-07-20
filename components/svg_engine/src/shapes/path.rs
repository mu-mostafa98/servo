/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// TODO: will be restored in future PRs
// use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
// use crate::render_tree::ClipPathUnits;
// use crate::shapes::{ClipGeometry, OBJECT_BBOX_REF_SIZE};
// use kurbo::ParamCurve;

use kurbo::BezPath;

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

// TODO: clip_info() will be restored in future PRs
//
// impl Path {
//     /// Clip geometry for this path (bounding box around all endpoint segments).
//     pub(crate) fn clip_info(
//         &self,
//         svg_origin: &LayoutPoint,
//         units: ClipPathUnits,
//     ) -> Option<ClipGeometry> {
//         ...
//     }
// }
