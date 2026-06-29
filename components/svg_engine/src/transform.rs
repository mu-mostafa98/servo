/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG transform types, parsing, and WebRender integration.
//!
//! This module defines the [`TransformOp`] enum, parses the SVG `transform`
//! attribute string into an ordered list of operations, and provides helpers
//! to apply each operation (translate, scale, rotate) onto a WebRender
//! display list builder.

use euclid::Transform2D;
use webrender_api::{
    DisplayListBuilder, PropertyBinding, ReferenceFrameKind,
    SpatialId, TransformStyle,
    units::{LayoutPoint, LayoutTransform},
};

// ------------------- Transform types ------------------

/// A single SVG transform operation, in the order it was specified.
#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32),  // (angle_deg, cx, cy)
}

// ------------------- Parsing ------------------

/// Parse the full `transform` attribute into an ordered list of transform operations.
///
/// Supports: `translate(tx,ty)`, `scale(s)`, `scale(sx,sy)`, `rotate(a)`, `rotate(a,cx,cy)`.
/// Multiple functions can be chained: `"translate(30,20) rotate(45)"` → `[Translate, Rotate]`.
pub fn extract_transforms(get_attr: &dyn Fn(&str) -> Option<String>) -> Vec<TransformOp> {
    let attr = match get_attr("transform") {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut remaining = attr.trim().to_string();
    let mut ops = Vec::new();

    while !remaining.is_empty() {
        let paren_open = match remaining.find('(') {
            Some(i) => i,
            None => break,
        };
        let paren_close = match remaining.find(')') {
            Some(i) => i,
            None => break,
        };

        let name = remaining[..paren_open].trim().to_string();
        let args_str = &remaining[paren_open + 1..paren_close];
        let args: Vec<f32> = args_str
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.trim().parse::<f32>().ok())
            .collect();

        match name.as_str() {
            "translate" if args.len() == 2 => {
                ops.push(TransformOp::Translate(args[0], args[1]));
            },
            "scale" if args.len() == 1 => {
                ops.push(TransformOp::Scale(args[0], args[0]));
            },
            "scale" if args.len() == 2 => {
                ops.push(TransformOp::Scale(args[0], args[1]));
            },
            "rotate" if args.len() == 1 => {
                ops.push(TransformOp::Rotate(args[0], 0.0, 0.0));
            },
            "rotate" if args.len() == 3 => {
                ops.push(TransformOp::Rotate(args[0], args[1], args[2]));
            },
            _ => {},
        }

        remaining = remaining[paren_close + 1..].trim().to_string();
        remaining = remaining.trim_start_matches(|c: char| c == ';' || c == ',').to_string();
    }

    ops
}

// ------------------- WebRender integration ------------------

/// Result of applying a single transform operation.
pub(crate) struct TransformResult {
    pub child_origin: LayoutPoint,
    pub child_spatial_id: SpatialId,
    /// Whether a reference frame was pushed (caller must pop).
    pub pushed_frame: bool,
}

/// Apply a transform operation onto a WebRender display list builder.
///
/// Returns the new origin and spatial id for child elements, and whether
/// a reference frame was pushed (caller must call `wr.pop_reference_frame()`).
pub(crate) fn apply_transform_op(
    op: &TransformOp,
    origin: LayoutPoint,
    spatial_id: SpatialId,
    wr: &mut DisplayListBuilder,
) -> TransformResult {
    match op {
        TransformOp::Translate(tx, ty) => {
            // Translate is a simple coordinate shift — no reference frame.
            TransformResult {
                child_origin: LayoutPoint::new(origin.x + tx, origin.y + ty),
                child_spatial_id: spatial_id,
                pushed_frame: false,
            }
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
            // rotate(a, cx, cy) = translate(cx,cy) × rotate(a) × translate(-cx,-cy)
            let lt = build_rotation_transform(*angle_deg, *cx, *cy);
            let frame_id = push_reference_frame(origin, spatial_id, lt, wr);
            TransformResult {
                child_origin: LayoutPoint::new(0.0, 0.0),
                child_spatial_id: frame_id,
                pushed_frame: true,
            }
        },
    }
}

/// Push a reference frame with the given transform.
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

/// Build a combined matrix for `rotate(a, cx, cy)`:
///   translate(cx, cy) × rotate(a) × translate(-cx, -cy)
fn build_rotation_transform(angle_deg: f32, cx: f32, cy: f32) -> LayoutTransform {
    let radians = angle_deg.to_radians();
    let (s, c) = radians.sin_cos();
    let t1: Transform2D<f32, (), ()> = Transform2D::translation(-cx, -cy);
    let rotate: Transform2D<f32, (), ()> = Transform2D::new(c, -s, s, c, 0.0, 0.0);
    let t2: Transform2D<f32, (), ()> = Transform2D::translation(cx, cy);
    let combined = t1.then(&rotate).then(&t2);
    to_layout_transform(&combined)
}

/// Convert a `Transform2D` to a `LayoutTransform` suitable for WebRender.
fn to_layout_transform(xform: &Transform2D<f32, (), ()>) -> LayoutTransform {
    // Transform2D stores column-vector: P' = [m11 m21 m31; m12 m22 m32; 0 0 1] * P
    //   x' = m11*x + m21*y + m31
    //   y' = m12*x + m22*y + m32
    //
    // LayoutTransform (row-major new()):
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
