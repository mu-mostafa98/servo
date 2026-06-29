/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Angle;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    PropertyBinding, ReferenceFrameKind, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize, LayoutTransform},
};

use crate::render_tree::*;
use crate::shapes::*;
use crate::styles::NodeStyle;

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
    // ── Apply translate offset ───────────────────────────────────────
    let origin = match node.translate {
        Some((tx, ty)) => LayoutPoint::new(svg_origin.x + tx, svg_origin.y + ty),
        None => *svg_origin,
    };
    let mut cur_spatial_id = spatial_id;
    let mut cur_origin = origin;
    let mut pop_count: u32 = 0;

    // ── Apply scale via reference frame ──────────────────────────────
    if let Some((sx, sy)) = node.scale {
        let lt = LayoutTransform::scale(sx, sy, 1.0);
        let frame_id = wr.push_reference_frame(
            cur_origin,
            cur_spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(lt),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );
        cur_spatial_id = frame_id;
        cur_origin = LayoutPoint::new(0.0, 0.0);
        pop_count += 1;
    }

    // ── Apply rotate via reference frame ─────────────────────────────
    if let Some((angle_deg, cx, cy)) = node.rotate {
        let angle = Angle::degrees(angle_deg);
        let lt = LayoutTransform::rotation(0.0, 0.0, 1.0, angle);
        let rotate_origin = LayoutPoint::new(cur_origin.x + cx, cur_origin.y + cy);
        let frame_id = wr.push_reference_frame(
            rotate_origin,
            cur_spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(lt),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
        );
        cur_spatial_id = frame_id;
        cur_origin = LayoutPoint::new(0.0, 0.0);
        pop_count += 1;
    }

    // ── Render this node and its children ────────────────────────────
    if let SvgTag::Shape(shape) = &node.tag {
        render_dispatch(shape, &node.style, &cur_origin, cur_spatial_id, clip_chain_id, wr);
    }

    for child in &node.children {
        render_node(child, &cur_origin, cur_spatial_id, clip_chain_id, wr);
    }

    // ── Pop any reference frames ─────────────────────────────────────
    for _ in 0..pop_count {
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
