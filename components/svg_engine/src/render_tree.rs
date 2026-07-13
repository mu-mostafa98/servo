/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::shapes::Shape;
use crate::style::NodeStyle;

/// The SVG render tree.
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

#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
}

// ======================= Visitor Pattern =======================

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
