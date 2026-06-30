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
}

// ======================= Build impl =======================

impl Build for Vec<TransformOp> {
    fn build(input: &SvgBuildInput) -> SvgResult<Self> {
        let attr = match (input.get_attr)("transform") {
            Some(s) => s,
            None => return Ok(Vec::new()),
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
            remaining = remaining
                .trim_start_matches(|c: char| c == ';' || c == ',')
                .to_string();
        }

        Ok(ops)
    }
}
