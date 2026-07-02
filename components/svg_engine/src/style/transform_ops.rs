/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG transform types and attribute parsing.
//!
//! This module defines the [`TransformOp`] enum with its [`Build`] impl
//! that parses the SVG `transform` attribute string.
//!
//! **No WebRender dependency** — pure data types and string parsing only.
//! WebRender integration lives in [`crate::renderer::transform`].

use crate::error::SvgResult;
use crate::builder::{Build, SvgBuildInput};

/// A single SVG transform operation, in the order it was specified.
#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32),  // (angle_deg, cx, cy)
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
/// Supports: `translate(tx,ty)`, `scale(s)`, `scale(sx,sy)`, `rotate(a)`, `rotate(a,cx,cy)`,
/// `skewX(a)`, `skewY(a)`, `matrix(a,b,c,d,e,f)`.
pub(crate) fn parse_transform_str(attr: &str) -> Vec<TransformOp> {
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
            "skewX" if args.len() == 1 => {
                ops.push(TransformOp::SkewX(args[0]));
            },
            "skewY" if args.len() == 1 => {
                ops.push(TransformOp::SkewY(args[0]));
            },
            "matrix" if args.len() == 6 => {
                ops.push(TransformOp::Matrix([args[0], args[1], args[2], args[3], args[4], args[5]]));
            },
            _ => {},
        }

        remaining = remaining[paren_close + 1..].trim().to_string();
        remaining = remaining
            .trim_start_matches(|c: char| c == ';' || c == ',')
            .to_string();
    }

    ops
}

// ======================= Build impl =======================

impl Build for Vec<TransformOp> {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let attr = match (input.get_attr)("transform") {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        Ok(parse_transform_str(&attr))
    }
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
    fn transform_unknown_ignored() {
        // skewX is now valid — use a truly unknown function name.
        let ops = parse_transform_str("unknownFunc(10)");
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
        let ops = parse_transform_str("translate(10,0) skewX(15) scale(2) matrix(1,0,0,1,0,0) rotate(45)");
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
}
