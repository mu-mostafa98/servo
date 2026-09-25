/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::units::Length;

/// SVG `<line>` element.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: Length,
    pub y1: Length,
    pub x2: Length,
    pub y2: Length,
}
