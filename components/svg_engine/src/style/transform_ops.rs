/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG transform types and attribute parsing powered by [`svgtypes`](https://docs.rs/svgtypes).
//!
//! SVG spec: <https://www.w3.org/TR/SVG2/coords.html#InterfaceSVGTransform>
//!
//! **No WebRender dependency** — pure data types and string parsing only.
//! WebRender integration lives in [`crate::renderer::transform`].

use svgtypes::{TransformListParser, TransformListToken};

/// A single SVG transform operation, in the order it was specified.
#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32), // (angle_deg, cx, cy)
    /// Skew along the X axis by the given angle in degrees.
    SkewX(f32),
    /// Skew along the Y axis by the given angle in degrees.
    SkewY(f32),
    /// Arbitrary 2D transform matrix: matrix(a, b, c, d, e, f).
    /// Represents the transform: [a c e; b d f; 0 0 1]
    Matrix([f32; 6]),
}

// ======================= Transform string parsing =======================

/// Parse a raw SVG `transform` attribute string into a list of [`TransformOp`]s.
///
/// Delegates to [`svgtypes::TransformListParser`] for SVG-spec-compliant parsing,
/// then maps each token to [`TransformOp`].  Expands `rotate(a, cx, cy)` into a
/// single [`TransformOp::Rotate`] rather than the three‑token decomposition
/// that `svgtypes` produces by default.
pub fn parse_transform_str(attr: &str) -> Vec<TransformOp> {
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
        // SVG: "If <ty> is not provided, it is assumed to be zero."
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
        let ops = parse_transform_str("");
        assert!(ops.is_empty());
    }

    #[test]
    fn transform_whitespace_only() {
        let ops = parse_transform_str("  ");
        assert!(ops.is_empty());
    }

    #[test]
    fn transform_skewx() {
        let ops = parse_transform_str("skewX(10)");
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], TransformOp::SkewX(_)));
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
        // rotate(a, cx, cy) must produce exactly one Rotate op, not three.
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
        // svgtypes (per SVG spec) does NOT treat semicolons as separators,
        // so "translate(10,20);" parses translate, then "scale(2)" is treated
        // as unknown data and skipped.
        let ops = parse_transform_str("translate(10,20); scale(2)");
        assert_eq!(ops.len(), 1);
    }
}
