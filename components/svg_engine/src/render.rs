/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Transform2D;
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
    let mut cur_spatial_id = spatial_id;
    let mut cur_origin = *svg_origin;
    let mut pop_count: u32 = 0;

    // Apply each transform operation in order.
    for op in &node.transforms {
        match op {
            TransformOp::Translate(tx, ty) => {
                // Translate is a simple shift — no reference frame needed.
                cur_origin = LayoutPoint::new(cur_origin.x + tx, cur_origin.y + ty);
            },
            TransformOp::Scale(sx, sy) => {
                let lt = LayoutTransform::scale(*sx, *sy, 1.0);
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
            },
            TransformOp::Rotate(angle_deg, cx, cy) => {
                // rotate(a, cx, cy) = translate(cx,cy) * rotate(a) * translate(-cx,-cy)
                let radians = angle_deg.to_radians();
                let (s, c) = radians.sin_cos();
                let t1: Transform2D<f32, (), ()> = Transform2D::translation(-*cx, -*cy);
                let rotate: Transform2D<f32, (), ()> = Transform2D::new(c, -s, s, c, 0.0, 0.0);
                let t2: Transform2D<f32, (), ()> = Transform2D::translation(*cx, *cy);
                let combined_2d = t1.then(&rotate).then(&t2);
                let lt = to_layout_transform(&combined_2d);

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
            },
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
    for _ in 0..pop_count {
        wr.pop_reference_frame();
    }
}

/// Convert a 2D affine matrix to a 4x4 LayoutTransform.
fn to_layout_transform(xform: &Transform2D<f32, (), ()>) -> LayoutTransform {
    // Transform2D stores column-vector: P' = [m11 m21 m31; m12 m22 m32; 0 0 1] * P
    //   x' = m11*x + m21*y + m31
    //   y' = m12*x + m22*y + m32
    //
    // LayoutTransform (row-major input): new(m11, m12, m13, m14, m21, m22, m23, m24, ...)
    //   Column j = (m1j, m2j, m3j, m4j)
    //   x' = m11*x + m21*y + m31*z + m41
    //   y' = m12*x + m22*y + m32*z + m42
    //
    // Mapping: col0=(m11_T, m12_T, 0, 0), col1=(m21_T, m22_T, 0, 0), col3=(m31_T, m32_T, 0, 1)
    //   m11=m11_T, m12=m21_T, m13=0, m14=m31_T
    //   m21=m12_T, m22=m22_T, m23=0, m24=m32_T
    LayoutTransform::new(
        xform.m11, xform.m21, 0.0, xform.m31,
        xform.m12, xform.m22, 0.0, xform.m32,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    )
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
