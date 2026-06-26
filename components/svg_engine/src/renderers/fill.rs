/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared fill logic for SVG shapes.
//!
//! Tessellates a polygon (convex or concave) into triangles using
//! lyon, then renders each triangle by mapping a unit rect through
//! an affine transform via `push_reference_frame`.
//!
//! The transform maps `(0,0)→(1,1)` in unit space to the triangle
//! `(v0, v1, v2)` so that `push_rect((0,0)→(1,1))` fills exactly
//! the triangle area.

use lyon::tessellation::{FillTessellator, FillOptions, FillVertex, FillVertexConstructor, VertexBuffers, BuffersBuilder};
use lyon::math::Point as LyonPoint;
use lyon::path::polygon::Polygon;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    CommonItemProperties, PropertyBinding, ReferenceFrameKind,
    SpaceAndClipInfo, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::styles::FillRule;

/// Vertex constructor that extracts only the position from FillVertex.
struct PosCtor;

impl FillVertexConstructor<LyonPoint> for PosCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> LyonPoint {
        vertex.position()
    }
}

/// Fill an arbitrary polygon (convex or concave) using lyon tessellation.
/// Each resulting triangle is rendered via an affine transform.
pub fn fill_polygon(
    points: &[LyonPoint],
    fill_rule: FillRule,
    color: webrender_api::ColorF,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    if points.len() < 3 {
        return;
    }

    // Tessellate the polygon into triangles using lyon.
    let polygon = Polygon { points, closed: true };
    let mut tessellator = FillTessellator::new();
    let mut buffers: VertexBuffers<LyonPoint, u32> = VertexBuffers::new();

    let lyon_fill_rule = match fill_rule {
        FillRule::NonZero => lyon::path::FillRule::NonZero,
        FillRule::EvenOdd => lyon::path::FillRule::EvenOdd,
    };

    if tessellator.tessellate(
        polygon.path_events(),
        &FillOptions::default()
            .with_fill_rule(lyon_fill_rule)
            .with_tolerance(0.01),
        &mut BuffersBuilder::new(&mut buffers, PosCtor),
    ).is_err() {
        return;
    }

    // Render each triangle.
    for tri in buffers.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = buffers.vertices[tri[0] as usize];
        let v1 = buffers.vertices[tri[1] as usize];
        let v2 = buffers.vertices[tri[2] as usize];

        render_triangle(v0, v1, v2, color, spatial_id, clip_chain_id, wr);
    }
}

/// Render a single triangle using an affine transform.
///
/// A unit rect `(0,0)→(1,1)` is transformed by a matrix that maps:
///   - (0,0) → v0
///   - (1,0) → v1
///   - (0,1) → v2
///
/// The push_rect inside the transformed space fills exactly the triangle area.
fn render_triangle(
    v0: LyonPoint,
    v1: LyonPoint,
    v2: LyonPoint,
    color: webrender_api::ColorF,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Build the affine transform as a 4x4 matrix.
    //
    // WebRender applies:  parent = origin + transform * child
    //
    // We want:
    //   (0,0) child → v0  →  origin = v0,  transform*(0,0) = (0,0)
    //   (1,0) child → v1  →  transform*(1,0) = v1 - v0
    //   (0,1) child → v2  →  transform*(0,1) = v2 - v0
    //
    // The 4x4 matrix layout (row-major in new()):
    //   row 0: (col0.x, col1.x,  0, 0)
    //   row 1: (col0.y, col1.y,  0, 0)
    //   row 2: (     0,      0,  1, 0)
    //   row 3: (     0,      0,  0, 1)
    let col0x = v1.x - v0.x;
    let col0y = v1.y - v0.y;
    let col1x = v2.x - v0.x;
    let col1y = v2.y - v0.y;

    let transform = LayoutTransform::new(
        col0x, col1x, 0.0, 0.0,
        col0y, col1y, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    );

    let tri_spatial_id = wr.push_reference_frame(
        LayoutPoint::new(v0.x, v0.y),
        spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(transform),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    );

    // Push a unit rect in the transformed space — fills exactly the triangle.
    let unit_rect = LayoutRect::from_origin_and_size(
        LayoutPoint::new(0.0, 0.0),
        LayoutSize::new(1.0, 1.0),
    );
    let common = CommonItemProperties::new(
        unit_rect,
        SpaceAndClipInfo { spatial_id: tri_spatial_id, clip_chain_id },
    );
    wr.push_rect(&common, unit_rect, color);
    wr.pop_reference_frame();
}
