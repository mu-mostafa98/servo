/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::{BezPath, PathEl, Point as KurboPoint};

use crate::renderer::{Render, RenderContext};
use crate::shapes::{Path, Polyline};

const FLATTEN_TOLERANCE: f64 = 0.1;

impl Render for Path {
    fn render(&self, ctx: &mut RenderContext) {
        let points = flatten_path(&self.path);

        if points.len() < 2 {
            return;
        }

        let polyline = Polyline { points };
        polyline.render(ctx);
    }
}

fn flatten_path(path: &BezPath) -> Vec<KurboPoint> {
    let mut points: Vec<KurboPoint> = Vec::new();
    let mut subpath_start: Option<KurboPoint> = None;

    kurbo::flatten(
        path.elements().iter().copied(),
        FLATTEN_TOLERANCE,
        |el| match el {
            PathEl::MoveTo(p) => {
                points.push(p);
                subpath_start = Some(p);
            },
            PathEl::LineTo(p) => {
                points.push(p);
            },
            PathEl::ClosePath => {
                if let Some(start) = subpath_start {
                    points.push(start);
                }
                subpath_start = None;
            },
            _ => {},
        },
    );

    points
}
