/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Shape;
use crate::style::NodeStyle;

#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
}

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    pub children: Vec<SvgRenderNode>,
}

#[derive(Debug)]
pub enum SvgTag {
    Shape(Shape),
    Container(Container),
}

impl From<Shape> for SvgTag {
    fn from(shape: Shape) -> Self {
        SvgTag::Shape(shape)
    }
}

impl From<Container> for SvgTag {
    fn from(container: Container) -> Self {
        SvgTag::Container(container)
    }
}

#[derive(Debug)]
pub enum Container {
    Svg,
    // Group,
    // Defs,
    // Use,
    // Symbol,
}
