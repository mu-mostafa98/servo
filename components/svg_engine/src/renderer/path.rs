/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::{BezPath, PathEl, Point};
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
};

use crate::shapes::{Path, Polyline};
use crate::style::NodeStyle;
use crate::renderer::Render;

/// Tolerance for flattening bezier curves into line segments.
/// Lower values = smoother curves, more segments.
/// 0.1 px is invisible to the user at any reasonable zoom level.
const FLATTEN_TOLERANCE: f64 = 0.1;

impl Render for Path {
    fn render(
        &self,
        style: &NodeStyle,
        svg_origin: &LayoutPoint,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
        wr: &mut DisplayListBuilder,
    ) {
        // Flatten curves into straight line segments, then extract points.
        let points = flatten_path(&self.path);

        if points.len() < 2 {
            return;
        }

        let polyline = Polyline { points };
        polyline.render(style, svg_origin, spatial_id, clip_chain_id, wr);
    }
}

/// Flatten a `BezPath` into a sequence of points by converting curves
/// to straight line segments.
fn flatten_path(path: &BezPath) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::new();
    let mut subpath_start: Option<Point> = None;

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
