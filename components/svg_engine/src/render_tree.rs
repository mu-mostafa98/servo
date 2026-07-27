/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use svgtypes::ViewBox as SvgViewBox;

pub use crate::image::SvgImage;
use crate::renderer::{ClipMaskProvider, FilterProvider, PaintResourceProvider};
use crate::shapes::Shape;
use crate::style::NodeStyle;
use crate::style::gradient::GradientDef;
use crate::style::transform_ops::TransformOp;
pub use crate::text::TextSpan;

// ======================= PreserveAspectRatio =======================

/// SVG `preserveAspectRatio` alignment type.
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

/// SVG `preserveAspectRatio` meet-or-slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeetOrSlice {
    Meet,
    Slice,
}

/// Parsed `preserveAspectRatio` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectRatio {
    pub align: AspectAlign,
    pub meet_or_slice: MeetOrSlice,
}

impl Default for AspectRatio {
    fn default() -> Self {
        // SVG spec: viewBox alone implies preserveAspectRatio="xMidYMid meet".
        AspectRatio {
            align: AspectAlign::XMidYMid,
            meet_or_slice: MeetOrSlice::Meet,
        }
    }
}

/// The SVG render tree — a tree of [`SvgRenderNode`]s plus viewport info
/// and gradient/clip-path/pattern/mask/filter definitions collected from `<defs>`.
#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
    /// Gradient definitions keyed by their `id` (without the `#` prefix).
    pub gradients: HashMap<String, GradientDef>,
    /// Clip path definitions keyed by their `id` (without the `#` prefix).
    pub clip_paths: HashMap<String, ClipPathDef>,
    /// Pattern definitions keyed by their `id` (without the `#` prefix).
    pub patterns: HashMap<String, PatternDef>,
    /// Mask definitions keyed by their `id` (without the `#` prefix).
    pub masks: HashMap<String, MaskDef>,
    /// Filter definitions keyed by their `id` (without the `#` prefix).
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
    /// SVG transforms applied to this node (CSS transform + `transform` attribute).
    /// These are structural (affect coordinate system), not paint-level style.
    pub transforms: Vec<TransformOp>,
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
    /// Parsed preserveAspectRatio (defaults to xMidYMid meet).
    pub aspect_ratio: Option<AspectRatio>,
}

/// A clip path definition collected from `<clipPath>`.
#[derive(Debug)]
pub struct ClipPathDef {
    /// The shapes that make up the clipping region.
    pub shapes: Vec<Shape>,
    /// Coordinate system for the clip path.
    pub clip_path_units: ClipPathUnits,
}

/// Coordinate system for clip path contents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipPathUnits {
    /// Coordinates are relative to the object's bounding box (0..1 range).
    ObjectBoundingBox,
    /// Coordinates are in the current user coordinate system.
    UserSpaceOnUse,
}

/// Coordinate system for pattern tile sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A mask definition collected from `<mask>`.
#[derive(Debug)]
pub struct MaskDef {
    /// The content shapes and their styles.
    pub shapes: Vec<(Shape, NodeStyle)>,
}

/// A single SVG filter primitive operation.
#[derive(Debug)]
pub enum FilterPrimitive {
    /// Gaussian blur: std_deviation_x, std_deviation_y.
    GaussianBlur(f32, f32),
    /// Drop shadow: dx, dy, std_deviation, color_r, color_g, color_b, color_a.
    DropShadow(f32, f32, f32, f32, f32, f32, f32),
    /// Full color matrix: 20 values (5 columns × 4 rows).
    ColorMatrix([f32; 20]),
    /// Saturate: single saturation value (0.0 = grayscale, 1.0 = normal, >1.0 = oversaturate).
    Saturate(f32),
    /// Luminance-to-alpha: converts luminance to alpha channel.
    LuminanceToAlpha,
    /// Offset: shifts the input by (dx, dy).
    Offset(f32, f32),
    /// Flood: fills the filter subregion with a solid color (RGBA).
    Flood(f32, f32, f32, f32),
    /// Composite: combines two inputs with an arithmetic composite (k1-k4)
    /// or a Porter-Duff operator.
    Composite(FeCompositeKind),
    /// Tile: repeats the input to fill the filter subregion.
    Tile,
    /// Image: renders an external image or referenced element as a filter input.
    Image(FeImageKind),
}

/// The kind of composite operation for `feComposite`.
#[derive(Debug)]
pub enum FeCompositeKind {
    /// Arithmetic composite: result = k1*i1*i2 + k2*i1 + k3*i2 + k4.
    Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
    /// Porter-Duff `over` operator.
    Over,
    /// Porter-Duff `in` operator.
    In,
    /// Porter-Duff `out` operator.
    Out,
    /// Porter-Duff `atop` operator.
    Atop,
    /// Porter-Duff `xor` operator.
    Xor,
    /// Lighter (additive) composite.
    Lighter,
}

/// The kind of image source for `feImage`.
#[derive(Debug)]
pub enum FeImageKind {
    /// Reference to another element via URL fragment (e.g., `#myElement`).
    FragmentRef(String),
    /// External image URL.
    ExternalUrl(String),
}

/// A filter definition collected from `<filter>`.
#[derive(Debug)]
pub struct FilterDef {
    /// Filter primitives in order (applied left-to-right).
    pub primitives: Vec<FilterPrimitive>,
    /// Filter bounds (x, y, width, height) - may be negative for drop-shadows.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Coordinate system for pattern content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A pattern definition collected from `<pattern>`.
#[derive(Debug, Clone)]
pub struct PatternDef {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub pattern_units: PatternUnits,
    pub pattern_content_units: PatternContentUnits,
    /// The content shapes and their styles that form the pattern tile.
    pub shapes: Vec<(Shape, NodeStyle)>,
}

// ======================= AspectRatio Parsing =======================

/// Parse a `preserveAspectRatio` attribute value.
///
/// SVG spec: `<align> <meetOrSlice>?`
/// Defaults to `xMidYMid meet`.
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

// ======================= ViewBox Parsing =======================

/// Parse the `viewBox` attribute value into a [`ViewBox`].
///
/// Delegates to [`svgtypes::ViewBox`] for spec-compliant parsing.
/// Handles formats: `"0 0 200 200"`, `"0,0 200,200"`, etc.
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
    /// Called for each node. Return `VisitDecision` to control traversal.
    fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision;
}

/// Visitor for mutation operations on the render tree.
pub trait SvgRenderTreeVisitorMut {
    /// Called for each node with mutable access. Return `VisitDecision` to control traversal.
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
        // svgtypes::ViewBox tolerates trailing data by spec (it stops at the 4th number).
        // If there are at least 4 valid numbers, it parses OK.
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
