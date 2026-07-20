/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::ViewBox as SvgViewBox;

pub use crate::image::SvgImage;
use crate::shapes::Shape;
pub use crate::text::TextSpan;

// TODO: AspectAlign, MeetOrSlice, AspectRatio, parse_aspect_ratio

/// The SVG render tree — a tree of [`SvgRenderNode`]s plus viewport info.
#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
    // TODO: definition maps
    // pub gradients: HashMap<String, GradientDef>,
    // pub clip_paths: HashMap<String, ClipPathDef>,
    // pub patterns: HashMap<String, PatternDef>,
    // pub masks: HashMap<String, MaskDef>,
    // pub filters: HashMap<String, FilterDef>,
}

// TODO: PaintResourceProvider, ClipMaskProvider, FilterProvider impls

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    // TODO:
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
    /// `<defs>` — definitions container whose children are not rendered directly.
    Defs,
    /// `<use>` — references another element by its `#id`.
    Use,
    /// `<symbol>` — a re-usable viewBox'd container referenced by `<use>`.
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
    /// When true, the viewport clip is omitted (CSS `overflow: visible`).
    pub overflow_visible: bool,
    // TODO: aspect_ratio
}

// TODO: definition types (ClipPathDef, ClipPathUnits, MaskDef, PatternDef,
// PatternUnits, PatternContentUnits, FilterDef, FilterPrimitive,
// FeCompositeKind, FeImageKind)

// TODO: parse_aspect_ratio()

// ======================= ViewBox Parsing =======================

/// Parse the `viewBox` attribute value into a [`ViewBox`].
pub fn extract_viewbox(value: &str) -> Option<ViewBox> {
    value.parse::<SvgViewBox>().ok().map(|vb| ViewBox {
        min_x: vb.x as f32,
        min_y: vb.y as f32,
        width: vb.w as f32,
        height: vb.h as f32,
    })
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
pub trait SvgRenderTreeVisitor {
    fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision;
}

/// Visitor for mutation operations on the render tree.
pub trait SvgRenderTreeVisitorMut {
    fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision;
}

impl SvgRenderNode {
    /// Accept a read-only visitor, traversing the tree in pre-order.
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

    /// Accept a mutable visitor, traversing the tree in pre-order.
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
    /// Visit every node in the tree with a read-only visitor.
    pub fn visit(&self, visitor: &mut dyn SvgRenderTreeVisitor) {
        self.root.accept(visitor);
    }

    /// Visit every node in the tree with a mutable visitor.
    pub fn visit_mut(&mut self, visitor: &mut dyn SvgRenderTreeVisitorMut) {
        self.root.accept_mut(visitor);
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_valid() {
        let vb = extract_viewbox("0 0 200 200").unwrap();
        assert_eq!(vb.min_x, 0.0);
        assert_eq!(vb.min_y, 0.0);
        assert_eq!(vb.width, 200.0);
        assert_eq!(vb.height, 200.0);
    }

    #[test]
    fn viewbox_with_commas() {
        let vb = extract_viewbox("10,20 300,400").unwrap();
        assert_eq!(vb.min_x, 10.0);
        assert_eq!(vb.min_y, 20.0);
        assert_eq!(vb.width, 300.0);
        assert_eq!(vb.height, 400.0);
    }

    #[test]
    fn viewbox_invalid_too_few() {
        assert!(extract_viewbox("0 0 200").is_none());
    }

    #[test]
    fn viewbox_invalid_too_many() {
        assert!(extract_viewbox("0 0 200 200 100").is_some());
    }

    #[test]
    fn viewbox_zero_width() {
        assert!(extract_viewbox("0 0 0 200").is_none());
    }

    #[test]
    fn viewbox_negative_width() {
        assert!(extract_viewbox("0 0 -100 200").is_none());
    }

    #[test]
    fn viewbox_negative_coords() {
        let vb = extract_viewbox("-100 -100 200 200").unwrap();
        assert_eq!(vb.min_x, -100.0);
        assert_eq!(vb.min_y, -100.0);
    }

    #[test]
    fn viewbox_empty() {
        assert!(extract_viewbox("").is_none());
    }

    #[test]
    fn viewbox_garbage() {
        assert!(extract_viewbox("abc def ghi jkl").is_none());
    }
}
