/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::{TransformListParser, TransformListToken};

#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32),
    SkewX(f32),
    SkewY(f32),
    Matrix([f32; 6]),
}

pub fn parse_transform_str(attr: &str) -> Vec<TransformOp> {
    let parser = TransformListParser::from(attr);
    let tokens: Vec<TransformListToken> = parser.filter_map(|r| r.ok()).collect();
    let mut ops = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
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
