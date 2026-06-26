/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    units::LayoutPoint,
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
    if let SvgTag::Shape(shape) = &node.tag {
        render_dispatch(shape, &node.style, svg_origin, spatial_id, clip_chain_id, wr);
    }

    for child in &node.children {
        render_node(child, svg_origin, spatial_id, clip_chain_id, wr);
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

