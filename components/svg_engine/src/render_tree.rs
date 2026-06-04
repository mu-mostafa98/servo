/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Shape;
use crate::styles::NodeStyle;

pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
}

pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    pub children: Vec<SvgRenderNode>,
}

pub enum SvgTag {
    Shape(Shape),
    Container(Container),
}

pub enum Container {
    Group,
    Svg,
}

pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
}