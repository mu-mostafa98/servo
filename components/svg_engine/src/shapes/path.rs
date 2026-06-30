/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::BezPath;

use crate::error::SvgResult;
use crate::extract::{Build, SvgBuildInput};
use crate::shapes::parse_path;

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

impl Build for Path {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        parse_path(&input.get_attr).map(|path| Path { path })
    }
}
