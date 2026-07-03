/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::BezPath;

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}
