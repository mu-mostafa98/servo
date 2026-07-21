/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub use crate::image::SvgImage;
use crate::shapes::Shape;
pub use crate::text::TextSpan;

#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    // pub viewport: ViewportInfo,
    // pub gradients: HashMap<String, GradientDef>,
    // pub clip_paths: HashMap<String, ClipPathDef>,
    // pub patterns: HashMap<String, PatternDef>,
    // pub masks: HashMap<String, MaskDef>,
    // pub filters: HashMap<String, FilterDef>,
}

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    // pub style: NodeStyle,
    // pub transforms: Vec<TransformOp>,
    pub children: Vec<SvgRenderNode>,
}

#[derive(Debug)]
pub enum SvgTag {
    Shape(Shape),
    Text(TextSpan),
    Image(SvgImage),
    Container(Container),
}


#[derive(Debug)]
pub enum Container {
    Group,
    Svg,
    Defs,
    Use,
    Symbol,
}
