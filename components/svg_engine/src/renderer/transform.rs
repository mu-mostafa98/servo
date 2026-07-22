/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Transform2D;
use webrender_api::units::{LayoutPoint, LayoutTransform};
use webrender_api::{
    DisplayListBuilder, PropertyBinding, ReferenceFrameKind, SpatialId, TransformStyle,
};

use crate::style::transform_ops::TransformOp;

pub(crate) struct TransformResult {
    pub child_origin: LayoutPoint,
    pub child_spatial_id: SpatialId,
    pub pushed_frame: bool,
}

pub(crate) fn apply_transform_op(
    op: &TransformOp,
    origin: LayoutPoint,
    spatial_id: SpatialId,
    wr: &mut DisplayListBuilder,
) -> TransformResult {
    match op {
        TransformOp::Translate(tx, ty) => TransformResult {
            child_origin: LayoutPoint::new(origin.x + tx, origin.y + ty),
            child_spatial_id: spatial_id,
            pushed_frame: false,
        },
        TransformOp::Scale(sx, sy) => {
            let lt = LayoutTransform::scale(*sx, *sy, 1.0);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
        TransformOp::Rotate(angle_deg, cx, cy) => {
            let lt = build_rotation_transform(*angle_deg, *cx, *cy);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
        TransformOp::SkewX(angle_deg) => {
            let radians = angle_deg.to_radians();
            let tan_a = radians.tan();
            let xform: Transform2D<f32, (), ()> = Transform2D::new(1.0, 0.0, tan_a, 1.0, 0.0, 0.0);
            let lt = to_layout_transform(&xform);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
        TransformOp::SkewY(angle_deg) => {
            let radians = angle_deg.to_radians();
            let tan_a = radians.tan();
            let xform: Transform2D<f32, (), ()> = Transform2D::new(1.0, tan_a, 0.0, 1.0, 0.0, 0.0);
            let lt = to_layout_transform(&xform);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
        TransformOp::Matrix([a, b, c, d, e, f]) => {
            let xform: Transform2D<f32, (), ()> = Transform2D::new(*a, *b, *c, *d, *e, *f);
            let lt = to_layout_transform(&xform);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
    }
}

fn push_reference_frame(
    origin: LayoutPoint,
    parent_spatial_id: SpatialId,
    transform: LayoutTransform,
    wr: &mut DisplayListBuilder,
) -> SpatialId {
    wr.push_reference_frame(
        origin,
        parent_spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(transform),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    )
}

fn build_rotation_transform(angle_deg: f32, cx: f32, cy: f32) -> LayoutTransform {
    let radians = angle_deg.to_radians();
    let (s, c) = radians.sin_cos();
    let t1: Transform2D<f32, (), ()> = Transform2D::translation(cx, cy);
    let rotate: Transform2D<f32, (), ()> = Transform2D::new(c, -s, s, c, 0.0, 0.0);
    let t2: Transform2D<f32, (), ()> = Transform2D::translation(-cx, -cy);
    let combined = t1.then(&rotate).then(&t2);
    to_layout_transform(&combined)
}

pub(crate) fn compute_transform_scale(ops: &[TransformOp]) -> f32 {
    let mut scale_x: f32 = 1.0;
    let mut scale_y: f32 = 1.0;
    for op in ops {
        match op {
            TransformOp::Scale(sx, sy) => {
                scale_x *= sx.abs();
                scale_y *= sy.abs();
            },
            TransformOp::Matrix([a, b, c, d, _, _]) => {
                let det = (a * d - b * c).abs();
                let s = if det > 0.0 { det.sqrt() } else { 1.0 };
                scale_x *= s;
                scale_y *= s;
            },
            _ => {},
        }
    }
    scale_x.max(scale_y)
}

pub(crate) fn to_layout_transform(xform: &Transform2D<f32, (), ()>) -> LayoutTransform {
    LayoutTransform::new(
        xform.m11, xform.m21, 0.0, xform.m31, xform.m12, xform.m22, 0.0, xform.m32, 0.0, 0.0, 1.0,
        0.0, 0.0, 0.0, 0.0, 1.0,
    )
}
