/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Transform2D;
use webrender_api::units::LayoutPoint;

use crate::renderer::path::rasterize_bez;
use crate::renderer::polyline::points_to_bez;
use crate::renderer::{Render, RenderContext};
use crate::shapes::Polygon;

/// Renders an SVG `<polygon>` as a closed path.
///
/// - Default: vello_cpu rasterization.
/// - `native_rendering` (pattern content): WebRender primitives.
impl Render for Polygon {
    fn render(&self, ctx: &mut RenderContext) {
        if ctx.native_rendering {
            // Close the point list and delegate to Polyline's native path.
            let mut closed_points = self.points.clone();
            if let Some(first) = self.points.first() {
                closed_points.push(*first);
            }
            crate::renderer::polyline::render_native_polyline(&closed_points, ctx, true);
            return;
        }

        let bez = points_to_bez(&self.points, true);
        // CPU-rasterized shapes bypass reference frames, so fold the nested
        // viewBox translation into the raster position explicitly.
        let raster_origin = LayoutPoint::new(
            ctx.svg_origin.x + ctx.raster_offset.x,
            ctx.svg_origin.y + ctx.raster_offset.y,
        );
        rasterize_bez(
            &bez,
            ctx.style.fill.as_ref(),
            ctx.style.stroke.as_ref(),
            ctx.style.opacity,
            &raster_origin,
            ctx.viewbox_scale,
            ctx.device_scale,
            Transform2D::identity(),
            None,
            ctx.paints,
            ctx.rasters,
        );
    }
}
