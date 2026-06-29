/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Shape;
use crate::styles::NodeStyle;
use crate::transform::TransformOp;

#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
}

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    pub transforms: Vec<TransformOp>,
    pub children: Vec<SvgRenderNode>,
}

#[derive(Debug)]
pub enum SvgTag {
    Shape(Shape),
    Container(Container),
}

#[derive(Debug)]
pub enum Container {
    Group,
    Svg,
}

#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
    /// Parsed viewBox: `(min_x, min_y, width, height)` in user units.
    pub view_box: Option<(f32, f32, f32, f32)>,
}

// ======================= ViewBox Parsing =======================

/// Parse the `viewBox` attribute value into `(min_x, min_y, width, height)`.
/// Expected format: `"0 0 200 200"` or `"0,0 200,200"`.
pub fn extract_viewbox(value: &str) -> Option<(f32, f32, f32, f32)> {
    let parts: Vec<f32> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();
    if parts.len() == 4 && parts[2] > 0.0 && parts[3] > 0.0 {
        Some((parts[0], parts[1], parts[2], parts[3]))
    } else {
        None
    }
}