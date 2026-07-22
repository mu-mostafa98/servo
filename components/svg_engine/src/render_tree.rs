/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use svgtypes::ViewBox as SvgViewBox;

use crate::renderer::{ClipMaskProvider, FilterProvider, PaintResourceProvider};
use crate::shapes::Shape;
use crate::style::NodeStyle;
use crate::style::gradient::GradientDef;
use crate::style::transform_ops::TransformOp;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectAlign {
    None,
    XMinYMin,
    XMidYMin,
    XMaxYMin,
    XMinYMid,
    XMidYMid,
    XMaxYMid,
    XMinYMax,
    XMidYMax,
    XMaxYMax,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeetOrSlice {
    Meet,
    Slice,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectRatio {
    pub align: AspectAlign,
    pub meet_or_slice: MeetOrSlice,
}

impl Default for AspectRatio {
    fn default() -> Self {
        AspectRatio {
            align: AspectAlign::XMidYMid,
            meet_or_slice: MeetOrSlice::Meet,
        }
    }
}

#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
    pub gradients: HashMap<String, GradientDef>,
    pub clip_paths: HashMap<String, ClipPathDef>,
    pub patterns: HashMap<String, PatternDef>,
    pub masks: HashMap<String, MaskDef>,
    pub filters: HashMap<String, FilterDef>,
}

impl PaintResourceProvider for SvgRenderTree {
    fn gradient(&self, id: &str) -> Option<&GradientDef> {
        self.gradients.get(id)
    }
    fn pattern(&self, id: &str) -> Option<&PatternDef> {
        self.patterns.get(id)
    }
}

impl ClipMaskProvider for SvgRenderTree {
    fn clip_path(&self, id: &str) -> Option<&ClipPathDef> {
        self.clip_paths.get(id)
    }
    fn mask(&self, id: &str) -> Option<&MaskDef> {
        self.masks.get(id)
    }
}

impl FilterProvider for SvgRenderTree {
    fn filter(&self, id: &str) -> Option<&FilterDef> {
        self.filters.get(id)
    }
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
    Group,
    Svg,
    Defs,
    Use,
    Symbol,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub min_x: f32,
    pub min_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
    pub view_box: Option<ViewBox>,
    pub overflow_visible: bool,
    pub aspect_ratio: Option<AspectRatio>,
}

#[derive(Debug)]
pub struct ClipPathDef {
    pub shapes: Vec<Shape>,
    pub clip_path_units: ClipPathUnits,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipPathUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

#[derive(Debug)]
pub struct MaskDef {
    pub shapes: Vec<(Shape, NodeStyle)>,
}

#[derive(Debug)]
pub enum FilterPrimitive {
    GaussianBlur(f32, f32),
    DropShadow(f32, f32, f32, f32, f32, f32, f32),
    ColorMatrix([f32; 20]),
    Saturate(f32),
    LuminanceToAlpha,
    Offset(f32, f32),
    Flood(f32, f32, f32, f32),
    Composite(FeCompositeKind),
    Tile,
}

#[derive(Debug)]
pub enum FeCompositeKind {
    Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
    Over,
    In,
    Out,
    Atop,
    Xor,
    Lighter,
}

#[derive(Debug)]
pub struct FilterDef {
    pub primitives: Vec<FilterPrimitive>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

#[derive(Debug, Clone)]
pub struct PatternDef {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub pattern_units: PatternUnits,
    pub pattern_content_units: PatternContentUnits,
    pub shapes: Vec<(Shape, NodeStyle)>,
}

pub fn parse_aspect_ratio(value: &str) -> AspectRatio {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return AspectRatio {
            align: AspectAlign::None,
            meet_or_slice: MeetOrSlice::Meet,
        };
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    let align = match parts.first().copied().unwrap_or("xMidYMid") {
        "none" => AspectAlign::None,
        "xMinYMin" => AspectAlign::XMinYMin,
        "xMidYMin" => AspectAlign::XMidYMin,
        "xMaxYMin" => AspectAlign::XMaxYMin,
        "xMinYMid" => AspectAlign::XMinYMid,
        "xMidYMid" => AspectAlign::XMidYMid,
        "xMaxYMid" => AspectAlign::XMaxYMid,
        "xMinYMax" => AspectAlign::XMinYMax,
        "xMidYMax" => AspectAlign::XMidYMax,
        "xMaxYMax" => AspectAlign::XMaxYMax,
        _ => AspectAlign::XMidYMid,
    };
    let meet_or_slice = parts
        .get(1)
        .copied()
        .and_then(|s| {
            if s.eq_ignore_ascii_case("slice") {
                Some(MeetOrSlice::Slice)
            } else {
                None
            }
        })
        .unwrap_or(MeetOrSlice::Meet);

    AspectRatio {
        align,
        meet_or_slice,
    }
}

pub fn extract_viewbox(value: &str) -> Option<ViewBox> {
    value.parse::<SvgViewBox>().ok().map(|vb| ViewBox {
        min_x: vb.x as f32,
        min_y: vb.y as f32,
        width: vb.w as f32,
        height: vb.h as f32,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisitDecision {
    Continue,
    SkipChildren,
    Stop,
}

pub trait SvgRenderTreeVisitor {
    fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision;
}

pub trait SvgRenderTreeVisitorMut {
    fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision;
}

impl SvgRenderNode {
    pub fn accept(&self, visitor: &mut dyn SvgRenderTreeVisitor) {
        let decision = visitor.visit_node(self);
        match decision {
            VisitDecision::Continue => {
                for child in &self.children {
                    child.accept(visitor);
                }
            },
            VisitDecision::SkipChildren => {},
            VisitDecision::Stop => (),
        }
    }

    pub fn accept_mut(&mut self, visitor: &mut dyn SvgRenderTreeVisitorMut) {
        let decision = visitor.visit_node_mut(self);
        match decision {
            VisitDecision::Continue => {
                for child in &mut self.children {
                    child.accept_mut(visitor);
                }
            },
            VisitDecision::SkipChildren => {},
            VisitDecision::Stop => (),
        }
    }
}

impl SvgRenderTree {
    pub fn visit(&self, visitor: &mut dyn SvgRenderTreeVisitor) {
        self.root.accept(visitor);
    }

    pub fn visit_mut(&mut self, visitor: &mut dyn SvgRenderTreeVisitorMut) {
        self.root.accept_mut(visitor);
    }
}
