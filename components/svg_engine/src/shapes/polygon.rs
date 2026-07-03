/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::Point;

/// SVG `<polygon>` element — a closed shape formed by connected line segments.
#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point>,
}
