/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conversion of Stylo/CSS transform operations into the SVG engine's
//! [`TransformOp`] type, plus parsing of the raw SVG `transform` attribute.

use svg_engine::style::transform_ops::TransformOp;
use svgtypes::{TransformListParser, TransformListToken};

/// Convert a CSS `transform` property from Stylo's [`ComputedValues`] into
/// the SVG engine's [`TransformOp`] list.
pub(crate) fn css_transform_from_computed(
    values: &style::properties::ComputedValues,
) -> Vec<TransformOp> {
    let list = &values.get_box().transform;
    if list.0.is_empty() {
        return Vec::new();
    }
    convert_transform_operations(&list.0)
}

/// Convert a slice of Stylo [`TransformOperation`]s into SVG engine [`TransformOp`]s.
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

/// Parse a raw SVG `transform` attribute string into a list of [`TransformOp`]s.
///
/// Delegates to [`svgtypes::TransformListParser`] for SVG-spec-compliant parsing,
/// then maps each token to [`TransformOp`].  Expands `rotate(a, cx, cy)` into a
/// single [`TransformOp::Rotate`] rather than the three‑token decomposition
/// that `svgtypes` produces by default.
pub(crate) fn parse_transform_str(attr: &str) -> Vec<TransformOp> {
    let parser = TransformListParser::from(attr);
    let tokens: Vec<TransformListToken> = parser.filter_map(|r| r.ok()).collect();
    let mut ops = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        // Collapse svgtypes's 3‑token expand for rotate(a, cx, cy):
        //   Translate(cx, cy) + Rotate(a) + Translate(-cx, -cy) → Rotate(a, cx, cy)
        if i + 2 < tokens.len() &&
            let (
                TransformListToken::Translate { tx: cx, ty: cy },
                TransformListToken::Rotate { angle },
                TransformListToken::Translate { tx: nx, ty: ny },
            ) = (&tokens[i], &tokens[i + 1], &tokens[i + 2]) &&
            (nx + cx).abs() < f64::EPSILON &&
            (ny + cy).abs() < f64::EPSILON
        {
            ops.push(TransformOp::Rotate(*angle as f32, *cx as f32, *cy as f32));
            i += 3;
            continue;
        }
        match tokens[i] {
            TransformListToken::Translate { tx, ty } => {
                ops.push(TransformOp::Translate(tx as f32, ty as f32));
            },
            TransformListToken::Rotate { angle } => {
                ops.push(TransformOp::Rotate(angle as f32, 0.0, 0.0));
            },
            TransformListToken::Scale { sx, sy } => {
                ops.push(TransformOp::Scale(sx as f32, sy as f32));
            },
            TransformListToken::SkewX { angle } => {
                ops.push(TransformOp::SkewX(angle as f32));
            },
            TransformListToken::SkewY { angle } => {
                ops.push(TransformOp::SkewY(angle as f32));
            },
            TransformListToken::Matrix { a, b, c, d, e, f } => {
                ops.push(TransformOp::Matrix([
                    a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
                ]));
            },
        }
        i += 1;
    }
    ops
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_translate() {
        let ops = parse_transform_str("translate(30,20)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Translate(x, y) => {
                assert_eq!(*x, 30.0);
                assert_eq!(*y, 20.0);
            },
            _ => panic!("expected Translate"),
        }
    }

    #[test]
    fn transform_translate_one_arg() {
        let ops = parse_transform_str("translate(10)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Translate(x, y) => {
                assert_eq!(*x, 10.0);
                assert_eq!(*y, 0.0);
            },
            _ => panic!("expected Translate"),
        }
    }

    #[test]
    fn transform_scale_uniform() {
        let ops = parse_transform_str("scale(2)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Scale(sx, sy) => {
                assert_eq!(*sx, 2.0);
                assert_eq!(*sy, 2.0);
            },
            _ => panic!("expected Scale"),
        }
    }

    #[test]
    fn transform_scale_nonuniform() {
        let ops = parse_transform_str("scale(2,3)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Scale(sx, sy) => {
                assert_eq!(*sx, 2.0);
                assert_eq!(*sy, 3.0);
            },
            _ => panic!("expected Scale"),
        }
    }

    #[test]
    fn transform_rotate_origin() {
        let ops = parse_transform_str("rotate(45)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Rotate(a, cx, cy) => {
                assert_eq!(*a, 45.0);
                assert_eq!(*cx, 0.0);
                assert_eq!(*cy, 0.0);
            },
            _ => panic!("expected Rotate"),
        }
    }

    #[test]
    fn transform_rotate_with_center() {
        let ops = parse_transform_str("rotate(90, 50, 100)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Rotate(a, cx, cy) => {
                assert_eq!(*a, 90.0);
                assert_eq!(*cx, 50.0);
                assert_eq!(*cy, 100.0);
            },
            _ => panic!("expected Rotate"),
        }
    }

    #[test]
    fn transform_chained() {
        let ops = parse_transform_str("translate(10,20) scale(2) rotate(45)");
        assert_eq!(ops.len(), 3);
        assert!(matches!(ops[0], TransformOp::Translate(..)));
        assert!(matches!(ops[1], TransformOp::Scale(..)));
        assert!(matches!(ops[2], TransformOp::Rotate(..)));
    }

    #[test]
    fn transform_empty() {
        assert!(parse_transform_str("").is_empty());
    }

    #[test]
    fn transform_whitespace_only() {
        assert!(parse_transform_str("  ").is_empty());
    }

    #[test]
    fn transform_skewx() {
        let ops = parse_transform_str("skewX(10)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::SkewX(a) => assert!((*a - 10.0).abs() < 0.001),
            _ => panic!("expected SkewX"),
        }
    }

    #[test]
    fn transform_skewy() {
        let ops = parse_transform_str("skewY(20)");
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], TransformOp::SkewY(_)));
    }

    #[test]
    fn transform_matrix() {
        let ops = parse_transform_str("matrix(1,0,0,1,10,20)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Matrix(m) => {
                assert_eq!(m[0], 1.0);
                assert_eq!(m[4], 10.0);
                assert_eq!(m[5], 20.0);
            },
            _ => panic!("expected Matrix"),
        }
    }

    #[test]
    fn transform_all_types_chained() {
        let ops = parse_transform_str(
            "translate(10,0) skewX(15) scale(2) matrix(1,0,0,1,0,0) rotate(45)",
        );
        assert_eq!(ops.len(), 5);
        assert!(matches!(ops[0], TransformOp::Translate(..)));
        assert!(matches!(ops[1], TransformOp::SkewX(..)));
        assert!(matches!(ops[2], TransformOp::Scale(..)));
        assert!(matches!(ops[3], TransformOp::Matrix(..)));
        assert!(matches!(ops[4], TransformOp::Rotate(..)));
    }

    #[test]
    fn transform_whitespace_variants() {
        let ops = parse_transform_str("  translate( 10 , 20 )  ");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Translate(x, y) => {
                assert_eq!(*x, 10.0);
                assert_eq!(*y, 20.0);
            },
            _ => panic!("expected Translate"),
        }
    }

    #[test]
    fn transform_rotate_with_center_roundtrip() {
        let ops = parse_transform_str("rotate(30, 10, 20)");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            TransformOp::Rotate(a, cx, cy) => {
                assert!((*a - 30.0).abs() < 0.001);
                assert!((*cx - 10.0).abs() < 0.001);
                assert!((*cy - 20.0).abs() < 0.001);
            },
            _ => panic!("expected Rotate"),
        }
    }

    #[test]
    fn transform_semicolon_separator() {
        let ops = parse_transform_str("translate(10,20); scale(2)");
        assert_eq!(ops.len(), 1);
    }
}
