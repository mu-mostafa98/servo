/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Shape;
use crate::styles::NodeStyle;

#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
}

/// A single SVG transform operation, in the order it was specified.
#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32),  // (angle_deg, cx, cy)
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

#[derive(Debug)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
}