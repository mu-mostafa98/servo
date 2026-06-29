/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::render_tree::*;
use crate::shapes::*;
use crate::styles::NodeStyle;
use crate::transform;

use crate::renderers;

pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
){
    // Apply SVG viewport clip — anything outside the SVG's bounds is hidden.
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let svg_clip_chain = wr.define_clip_chain(
        if clip_chain_id == ClipChainId::INVALID { None } else { Some(clip_chain_id) },
        [svg_clip_id],
    );

    render_node(&tree.root, svg_origin, spatial_id, svg_clip_chain, wr);
}

fn render_node(
    node : &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
){
    let mut cur_spatial_id = spatial_id;
    let mut cur_origin = *svg_origin;
    let mut pushed_count: u32 = 0;

    // Apply each transform operation in order.
    for op in &node.transforms {
        let result = transform::apply_transform_op(op, cur_origin, cur_spatial_id, wr);
        cur_origin = result.child_origin;
        cur_spatial_id = result.child_spatial_id;
        if result.pushed_frame {
            pushed_count += 1;
        }
    }

    // Render this node and its children.
    if let SvgTag::Shape(shape) = &node.tag {
        render_dispatch(shape, &node.style, &cur_origin, cur_spatial_id, clip_chain_id, wr);
    }

    for child in &node.children {
        render_node(child, &cur_origin, cur_spatial_id, clip_chain_id, wr);
    }

    // Pop any reference frames in reverse order.
    for _ in 0..pushed_count {
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
