/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::BezPath;

use crate::error::{SvgEngineError, SvgResult};
use crate::builder::{Build, SvgBuildInput};

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

impl Build for Path {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let value = (input.get_attr)("d")
            .ok_or_else(|| SvgEngineError::MissingAttribute("d".to_owned()))?;
        let path = BezPath::from_svg(&value)
            .map_err(|e| SvgEngineError::ParseError(format!("path: {e}")))?;
        Ok(Path { path })
    }
}
