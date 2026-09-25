/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG elements — every kind of node that can appear in the render tree.
//!
//! This module holds the *element* types (anything written as `<element>` in
//! SVG): shapes, images, text, and the container/`SvgNode`/`SvgTag` machinery
//! that ties them into a tree. It is pure data — no WebRender/vello dependency.

pub mod image;
pub mod shape;
pub mod text;

pub use self::image::SvgImage;
pub use self::shape::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rectangle, Shape};
pub use self::text::{DominantBaseline, ShapedGlyph, TextAnchor, TextSpan};

use crate::model::document::viewport::SvgViewport;
use crate::model::style::transform::TransformOp;
use crate::model::style::NodeStyle;
use crate::model::units::Id;

/// A single node in the SVG render tree.
#[derive(Debug)]
pub struct SvgNode {
    pub id: Option<Id>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    /// SVG transforms applied to this node (CSS transform + `transform` attribute).
    /// These are structural (affect coordinate system), not paint-level style.
    pub transforms: Vec<TransformOp>,
    /// Nested `<svg>` viewport (viewBox + x/y/width/height + preserveAspectRatio).
    /// `None` for the root `<svg>` (handled via [`SvgTree::viewport`]) and
    /// for every non-`<svg>` node.
    pub viewport: Option<SvgViewport>,
    pub children: Vec<SvgNode>,
}

/// The kind of content an [`SvgNode`] carries.
#[derive(Debug)]
pub enum SvgTag {
    Shape(Shape),
    Text(TextSpan),
    Image(SvgImage),
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

/// A container element — a node whose children form an ordered group.
#[derive(Debug)]
pub enum Container {
    Group,
    Svg,
    /// `<defs>` — definitions container whose children are not rendered directly.
    Defs,
    /// `<use>` — references another element by its `#id`.
    Use,
    /// `<symbol>` — a re-usable viewBox'd container referenced by `<use>`.
    Symbol,
    /// `<text>` — a logical text element whose children are the inline
    /// `<tspan>`/bare-text runs of the line. Unlike `<g>`, the children are
    /// ordered text runs laid out on a single baseline with cumulative
    /// advance (each `TextSpan` carries its own `advance_offset`).
    Text,
}

// ======================= Visitor Pattern =======================

/// Traversal decision for the visitor pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisitDecision {
    /// Continue traversal into children.
    Continue,
    /// Skip children but continue traversal at the parent's next sibling.
    SkipChildren,
    /// Stop all traversal entirely.
    Stop,
}

/// Visitor for read-only operations on the render tree.
pub trait SvgTreeVisitor {
    /// Called for each node. Return `VisitDecision` to control traversal.
    fn visit_node(&mut self, node: &SvgNode) -> VisitDecision;
}

/// Visitor for mutation operations on the render tree.
pub trait SvgTreeVisitorMut {
    /// Called for each node with mutable access. Return `VisitDecision` to control traversal.
    fn visit_node_mut(&mut self, node: &mut SvgNode) -> VisitDecision;
}

impl SvgNode {
    /// Accept a read-only visitor, traversing the tree in pre-order.
    pub fn accept(&self, visitor: &mut dyn SvgTreeVisitor) {
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

    /// Accept a mutable visitor, traversing the tree in pre-order.
    pub fn accept_mut(&mut self, visitor: &mut dyn SvgTreeVisitorMut) {
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
