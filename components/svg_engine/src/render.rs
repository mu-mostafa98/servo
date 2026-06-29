/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    PropertyBinding, ReferenceFrameKind, TransformStyle,
    units::{LayoutPoint, LayoutTransform},
};

use crate::render_tree::*;
use crate::shapes::*;
use crate::styles::NodeStyle;

use crate::renderers;


pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
){
    render_node(&tree.root, svg_origin, spatial_id, clip_chain_id, wr);
}

fn render_node(
    node : &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
){
    // Apply translate offset if present.
    let origin = match node.translate {
        Some((tx, ty)) => LayoutPoint::new(svg_origin.x + tx, svg_origin.y + ty),
        None => *svg_origin,
    };

    // Apply scale via reference frame if present.
    let (child_origin, child_spatial_id, needs_pop) = if let Some((sx, sy)) = node.scale {
        let transform = LayoutTransform::scale(sx, sy, 1.0);
        let frame_id = wr.push_reference_frame(
            origin,
            spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(transform),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );
        (LayoutPoint::new(0.0, 0.0), frame_id, true)
    } else {
        (origin, spatial_id, false)
    };

    if let SvgTag::Shape(shape) = &node.tag {
        render_dispatch(shape, &node.style, &child_origin, child_spatial_id, clip_chain_id, wr);
    }

    for child in &node.children {
        render_node(child, &child_origin, child_spatial_id, clip_chain_id, wr);
    }

    if needs_pop {
        wr.pop_reference_frame();
    }
}

fn render_dispatch(
    shape: &Shape,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    match shape {
        Shape::Rect(rect) => renderers::render_rect(rect, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Ellipse(ellipse) => renderers::render_ellipse(ellipse, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Circle(circle) => renderers::render_circle(circle, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Line(line) => renderers::render_line(line, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Polyline(polyline) => renderers::render_polyline(polyline, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Polygon(polygon) => renderers::render_polygon(polygon, style, svg_origin, spatial_id, clip_chain_id, wr),
        Shape::Path(path) => renderers::render_path(&path.path, style, svg_origin, spatial_id, clip_chain_id, wr),
    }
}

