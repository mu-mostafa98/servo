/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::BezPath;
use webrender_api::units::LayoutPoint;

use crate::model::tree::ClipPathUnits;
use crate::model::shapes::{ClipGeometry, clip_path_geometry};

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

impl Path {
    /// Clip geometry for this path (the exact path outline, curves preserved).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        if self.path.elements().is_empty() {
            return None;
        }
        Some(clip_path_geometry(&self.path, svg_origin, units))
    }
}
