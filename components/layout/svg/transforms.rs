/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svg_engine::style::transform_ops::TransformOp;

pub(crate) fn css_transform_from_computed(
    values: &style::properties::ComputedValues,
) -> Vec<TransformOp> {
    let list = &values.get_box().transform;
    if list.0.is_empty() {
        return Vec::new();
    }
    convert_transform_operations(&list.0)
}

fn convert_transform_operations(
    ops: &[style::values::computed::transform::TransformOperation],
) -> Vec<TransformOp> {
    use style::values::generics::transform::GenericTransformOperation::*;
    use style::values::generics::transform::ToAbsoluteLength;

    let mut result = Vec::new();
    for op in ops {
        match op {
            Rotate(angle) => {
                result.push(TransformOp::Rotate(angle.degrees(), 0.0, 0.0));
            },
            Translate(tx, ty) => {
                let px = ToAbsoluteLength::to_pixel_length(tx, None).unwrap_or(0.0);
                let py = ToAbsoluteLength::to_pixel_length(ty, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(px, py));
            },
            TranslateX(t) => {
                let px = ToAbsoluteLength::to_pixel_length(t, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(px, 0.0));
            },
            TranslateY(t) => {
                let py = ToAbsoluteLength::to_pixel_length(t, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(0.0, py));
            },
            Scale(sx, sy) => {
                result.push(TransformOp::Scale(*sx, *sy));
            },
            ScaleX(s) => {
                result.push(TransformOp::Scale(*s, 1.0));
            },
            ScaleY(s) => {
                result.push(TransformOp::Scale(1.0, *s));
            },
            SkewX(a) => {
                result.push(TransformOp::SkewX(a.degrees()));
            },
            SkewY(a) => {
                result.push(TransformOp::SkewY(a.degrees()));
            },
            Matrix(m) => {
                result.push(TransformOp::Matrix([m.a, m.b, m.c, m.d, m.e, m.f]));
            },
            _ => {},
        }
    }
    result
}
