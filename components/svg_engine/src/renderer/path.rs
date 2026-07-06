/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::{BezPath, PathEl, Point as KurboPoint};

use crate::shapes::{Path, Polyline};
use crate::renderer::{Render, RenderContext};

/// Tolerance for flattening bezier curves into line segments.
/// Lower values = smoother curves, more segments.
/// 0.1 px is invisible to the user at any reasonable zoom level.
const FLATTEN_TOLERANCE: f64 = 0.1;

/// Renders an SVG `<path>`.
///
/// LSP contract:
/// - Flattens bezier curves into line segments (via [`kurbo::flatten`]),
///   then delegates to [`Polyline::render`].
/// - All LSP invariants are preserved through the delegation chain.
impl Render for Path {
    fn render(&self, ctx: &mut RenderContext) {
        // Flatten curves into straight line segments, then extract points.
        let points = flatten_path(&self.path);

        if points.len() < 2 {
            return;
        }

        let polyline = Polyline { points };
        polyline.render(ctx);
    }
}

/// Flatten a `BezPath` into a sequence of points by converting curves
/// to straight line segments.
fn flatten_path(path: &BezPath) -> Vec<KurboPoint> {
    let mut points: Vec<KurboPoint> = Vec::new();
    let mut subpath_start: Option<KurboPoint> = None;

    kurbo::flatten(path.elements().iter().copied(), FLATTEN_TOLERANCE, |el| {
        match el {
            PathEl::MoveTo(p) => {
                points.push(p);
                subpath_start = Some(p);
            }
            PathEl::LineTo(p) => {
                points.push(p);
            }
            PathEl::ClosePath => {
                if let Some(start) = subpath_start {
                    points.push(start);
                }
                subpath_start = None;
            }
            _ => {}
        }
    });

    points
}
