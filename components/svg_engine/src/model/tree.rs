/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::sync::Arc;

pub use crate::model::image::SvgImage;
use crate::model::shapes::Shape;
use crate::model::style::NodeStyle;
use crate::model::style::gradient::GradientDef;
use crate::model::style::transform_ops::TransformOp;
use crate::model::units::{Id, Length};
pub use crate::model::text::TextSpan;

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

/// The SVG render tree — a tree of [`SvgNode`]s plus viewport info
/// and gradient/clip-path/pattern/mask/filter definitions collected from `<defs>`.
#[derive(Debug)]
pub struct SvgTree {
    pub root: SvgNode,
    pub viewport: ViewportInfo,
    /// Gradient definitions keyed by their `id` (without the `#` prefix).
    pub gradients: HashMap<String, Arc<GradientDef>>,
    /// Clip path definitions keyed by their `id` (without the `#` prefix).
    pub clip_paths: HashMap<String, Arc<ClipPathDef>>,
    /// Pattern definitions keyed by their `id` (without the `#` prefix).
    pub patterns: HashMap<String, Arc<PatternDef>>,
    /// Mask definitions keyed by their `id` (without the `#` prefix).
    pub masks: HashMap<String, Arc<MaskDef>>,
    /// Filter definitions keyed by their `id` (without the `#` prefix).
    pub filters: HashMap<String, Arc<FilterDef>>,
    /// Marker definitions keyed by their `id` (without the `#` prefix).
    pub markers: HashMap<String, Arc<MarkerDef>>,
}

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
    /// `<text>` — a logical text element whose children are the inline
    /// `<tspan>`/bare-text runs of the line. Unlike `<g>`, the children are
    /// ordered text runs laid out on a single baseline with cumulative
    /// advance (each `TextSpan` carries its own `advance_offset`).
    Text,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub min_x: Length,
    pub min_y: Length,
    pub width: Length,
    pub height: Length,
}

#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: Length,
    pub height: Length,
    pub view_box: Option<ViewBox>,
    /// When true, the viewport clip is omitted (CSS `overflow: visible`).
    pub overflow_visible: bool,
    /// Parsed preserveAspectRatio (defaults to xMidYMid meet).
    pub aspect_ratio: Option<AspectRatio>,
}

/// Viewport established by a nested `<svg>` element.
///
/// Unlike the root [`ViewportInfo`] (whose size is imposed by layout), a nested
/// `<svg>` carries its own `x`/`y`/`width`/`height` attributes that position and
/// size the sub-viewport in the parent user coordinate system, plus an optional
/// `viewBox` and `preserveAspectRatio` that map content into it.
#[derive(Debug, Clone)]
pub struct SvgViewport {
    /// Position of the viewport in the parent user coordinate system.
    pub x: Length,
    pub y: Length,
    /// Size of the viewport (from the `width`/`height` attributes).
    pub width: Length,
    pub height: Length,
    pub view_box: Option<ViewBox>,
    /// Parsed preserveAspectRatio (defaults to xMidYMid meet via the renderer).
    pub aspect_ratio: Option<AspectRatio>,
    /// When true, the sub-viewport clip is omitted (`overflow: visible`).
    pub overflow_visible: bool,
}

/// A clip path definition collected from `<clipPath>`.
#[derive(Debug)]
pub struct ClipPathDef {
    /// The clipping region, stored as a nested node subtree so that `<g>`,
    /// `<use>`, `<text>` and nested-`<defs>` children are not silently dropped.
    pub root: SvgNode,
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

/// The `mask-type` of a `<mask>`: whether the mask value is derived from the
/// mask content's luminance or its alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    /// `mask-type="luminance"` (the SVG default): mask value = luminance of the
    /// mask content's composited color.
    Luminance,
    /// `mask-type="alpha"`: mask value = alpha channel of the mask content.
    Alpha,
}

/// Coordinate system for `<mask>` content (`maskContentUnits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A mask definition collected from `<mask>`.
#[derive(Debug)]
pub struct MaskDef {
    /// The mask content, stored as a nested node subtree (see [`ClipPathDef::root`]).
    pub root: SvgNode,
    /// Whether the mask value is luminance- or alpha-derived (`mask-type`).
    pub mask_type: MaskType,
    /// Coordinate system for the mask content (`maskContentUnits`).
    pub content_units: MaskContentUnits,
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
#[derive(Debug)]
pub struct PatternDef {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub pattern_units: PatternUnits,
    pub pattern_content_units: PatternContentUnits,
    /// The `patternTransform` attribute, applied to the tile coordinate system.
    pub transform: Vec<crate::model::style::transform_ops::TransformOp>,
    /// Optional `viewBox` on the pattern, mapped into the tile via
    /// `preserveAspectRatio`.
    pub view_box: Option<ViewBox>,
    pub aspect_ratio: Option<AspectRatio>,
    /// The pattern tile content, stored as a nested node subtree (see
    /// [`ClipPathDef::root`]).
    pub root: SvgNode,
}

/// Coordinate system for marker sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerUnits {
    /// Marker size scales with the referencing shape's stroke width (default).
    StrokeWidth,
    /// Marker size is fixed in the current user coordinate system.
    UserSpaceOnUse,
}

/// Orientation of a marker relative to the path.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerOrient {
    /// Rotate to align with the path tangent (default).
    Auto,
    /// Like `Auto`, but the marker at the path *start* is flipped 180°.
    AutoStartReverse,
    /// Fixed rotation angle in degrees.
    Angle(f32),
}

impl Default for MarkerOrient {
    fn default() -> Self {
        MarkerOrient::Auto
    }
}

/// A marker definition collected from `<marker>`.
#[derive(Debug)]
pub struct MarkerDef {
    /// Marker content, stored as a nested node subtree (see [`ClipPathDef::root`]).
    pub root: SvgNode,
    /// Optional `viewBox` establishing the marker's coordinate system.
    pub view_box: Option<ViewBox>,
    /// Reference point (in viewBox coords) aligned with the path vertex.
    pub ref_x: f32,
    pub ref_y: f32,
    /// Rendered marker viewport width (scaled per `marker_units`).
    pub marker_width: f32,
    pub marker_height: f32,
    pub marker_units: MarkerUnits,
    pub orient: MarkerOrient,
}

/// A reference to a definition (clip-path, mask, filter, or marker) that
/// starts as a raw `#id` string during tree building and is rewritten to a
/// typed [`Arc`] handle once the definition maps are collected, during the
/// post-build resolve pass.
///
/// This mirrors the transient `PaintServer::Ref` variant: the build layer
/// emits [`DefRef::Ref`] and the resolve pass rewrites it to
/// [`DefRef::Resolved`], so render-time consumers only ever see a resolved
/// handle.
#[derive(Debug)]
pub enum DefRef<T> {
    /// Raw `#id` (without the `#` prefix), not yet resolved.
    Ref(Id),
    /// Typed definition handle, resolved after build.
    Resolved(Arc<T>),
}

// Manual `Clone` rather than a derived one: both variants clone without
// cloning `T` itself (`Arc<T>` clones the handle, `String` clones the id),
// so `DefRef<T>` is `Clone` for *any* `T` — including definitions like
// `ClipPathDef`/`MaskDef`/`FilterDef`/`MarkerDef` that embed a non-`Clone`
// `SvgNode`.
impl<T> Clone for DefRef<T> {
    fn clone(&self) -> Self {
        match self {
            DefRef::Ref(id) => DefRef::Ref(id.clone()),
            DefRef::Resolved(def) => DefRef::Resolved(Arc::clone(def)),
        }
    }
}

impl<T> DefRef<T> {
    /// The resolved definition, or `None` if this reference is still
    /// unresolved (which should not happen after the resolve pass).
    pub fn resolved(&self) -> Option<&T> {
        match self {
            DefRef::Resolved(def) => Some(def.as_ref()),
            DefRef::Ref(_) => None,
        }
    }
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

impl SvgTree {
    /// Visit every node in the tree with a read-only visitor.
    pub fn visit(&self, visitor: &mut dyn SvgTreeVisitor) {
        self.root.accept(visitor);
    }

    /// Visit every node in the tree with a mutable visitor.
    pub fn visit_mut(&mut self, visitor: &mut dyn SvgTreeVisitorMut) {
        self.root.accept_mut(visitor);
    }
}
