/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::sync::Arc;

use svgtypes::ViewBox as SvgViewBox;

pub use crate::image::SvgImage;
use crate::renderer::PaintResourceProvider;
use crate::shapes::Shape;
use crate::style::NodeStyle;
use crate::style::gradient::{GradientDef, PaintServer};
use crate::style::transform_ops::TransformOp;
use crate::units::{Id, Length};
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

impl PaintResourceProvider for SvgRenderTree {
    fn gradient(&self, id: &str) -> Option<&GradientDef> {
        self.gradients.get(id).map(|def| def.as_ref())
    }
    fn pattern(&self, id: &str) -> Option<&PatternDef> {
        self.patterns.get(id).map(|def| def.as_ref())
    }
}

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<Id>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    /// SVG transforms applied to this node (CSS transform + `transform` attribute).
    /// These are structural (affect coordinate system), not paint-level style.
    pub transforms: Vec<TransformOp>,
    /// Nested `<svg>` viewport (viewBox + x/y/width/height + preserveAspectRatio).
    /// `None` for the root `<svg>` (handled via [`SvgRenderTree::viewport`]) and
    /// for every non-`<svg>` node.
    pub viewport: Option<SvgViewport>,
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
    pub root: SvgRenderNode,
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
    /// The mask content, stored as a nested node subtree (see [`ClipPathDef::root`]).
    pub root: SvgRenderNode,
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
    pub transform: Vec<crate::style::transform_ops::TransformOp>,
    /// Optional `viewBox` on the pattern, mapped into the tile via
    /// `preserveAspectRatio`.
    pub view_box: Option<ViewBox>,
    pub aspect_ratio: Option<AspectRatio>,
    /// The pattern tile content, stored as a nested node subtree (see
    /// [`ClipPathDef::root`]).
    pub root: SvgRenderNode,
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
    pub root: SvgRenderNode,
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
/// typed [`Arc`] handle once the definition maps are collected (see
/// [`SvgRenderTree::resolve_references`]).
///
/// This mirrors [`PaintServer`]'s transient `Ref` variant: the build layer
/// emits [`DefRef::Ref`] and the post-build resolve pass rewrites it to
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
// `SvgRenderNode`.
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
        min_x: Length::new(vb.x as f32),
        min_y: Length::new(vb.y as f32),
        width: Length::new(vb.w as f32),
        height: Length::new(vb.h as f32),
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

    /// Flatten container groups and invoke `f(shape, style)` for every shape
    /// leaf under this node. Non-shape leaves (text, image, …) are skipped.
    ///
    /// Used by clip/mask collection and pattern/marker content rendering so
    /// that nested `<g>`/`<use>`/`<symbol>` wrappers inside a definition are
    /// honoured instead of being silently dropped.
    pub(crate) fn for_each_shape_leaf<F>(&self, f: &mut F)
    where
        F: FnMut(&Shape, &NodeStyle),
    {
        match &self.tag {
            SvgTag::Shape(shape) => f(shape, &self.style),
            // `<defs>` content is never rendered directly — skip it.
            SvgTag::Container(Container::Defs) => {},
            _ => {
                for child in &self.children {
                    child.for_each_shape_leaf(f);
                }
            },
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

    /// Rewrite every transient reference in the tree — [`PaintServer::Ref`]
    /// paint servers and [`DefRef::Ref`] clip-path/mask/filter/marker handles —
    /// into typed `Arc` handles using the collected definition maps.
    ///
    /// A paint-server reference that resolves to neither a gradient nor a
    /// pattern falls back to opaque black. A clip-path/mask/filter/marker
    /// reference that does not resolve is dropped (the effect/marker is
    /// omitted), matching SVG's ignore-broken-references behavior.
    pub fn resolve_references(&mut self) {
        let Self {
            root,
            gradients,
            patterns,
            clip_paths,
            masks,
            filters,
            markers: marker_defs,
            ..
        } = self;
        resolve_references_in(
            root,
            gradients,
            patterns,
            clip_paths,
            masks,
            filters,
            marker_defs,
        );
    }
}

fn resolve_references_in(
    node: &mut SvgRenderNode,
    gradients: &HashMap<String, Arc<GradientDef>>,
    patterns: &HashMap<String, Arc<PatternDef>>,
    clip_paths: &HashMap<String, Arc<ClipPathDef>>,
    masks: &HashMap<String, Arc<MaskDef>>,
    filters: &HashMap<String, Arc<FilterDef>>,
    marker_defs: &HashMap<String, Arc<MarkerDef>>,
) {
    if let Some(fill) = node.style.fill.as_mut() {
        if let Some(paint) = fill.paint_server.as_mut() {
            resolve_paint_server(paint, gradients, patterns);
        }
    }
    if let Some(stroke) = node.style.stroke.as_mut() {
        if let Some(paint) = stroke.paint_server.as_mut() {
            resolve_paint_server(paint, gradients, patterns);
        }
    }

    if let Some(effects) = node.style.effects.as_mut() {
        effects.clip_path = resolve_ref(effects.clip_path.take(), clip_paths);
        effects.mask = resolve_ref(effects.mask.take(), masks);
        effects.filter = resolve_ref(effects.filter.take(), filters);
    }
    if let Some(effects) = node.style.effects.as_ref() {
        if effects.clip_path.is_none() && effects.mask.is_none() && effects.filter.is_none() {
            node.style.effects = None;
        }
    }

    if let Some(refs) = node.style.markers.as_mut() {
        refs.start = resolve_ref(refs.start.take(), marker_defs);
        refs.mid = resolve_ref(refs.mid.take(), marker_defs);
        refs.end = resolve_ref(refs.end.take(), marker_defs);
    }
    if let Some(refs) = node.style.markers.as_ref() {
        if refs.start.is_none() && refs.mid.is_none() && refs.end.is_none() {
            node.style.markers = None;
        }
    }

    for child in &mut node.children {
        resolve_references_in(
            child,
            gradients,
            patterns,
            clip_paths,
            masks,
            filters,
            marker_defs,
        );
    }
}

fn resolve_paint_server(
    paint: &mut PaintServer,
    gradients: &HashMap<String, Arc<GradientDef>>,
    patterns: &HashMap<String, Arc<PatternDef>>,
) {
    let id = match paint {
        PaintServer::Ref(id) => id.clone(),
        _ => return,
    };
    *paint = if let Some(def) = gradients.get(id.as_str()) {
        PaintServer::Gradient(def.clone())
    } else if let Some(def) = patterns.get(id.as_str()) {
        PaintServer::Pattern(def.clone())
    } else {
        PaintServer::Solid(svgtypes::Color::new_rgb(0, 0, 0))
    };
}

/// Resolve a transient [`DefRef::Ref`] into a typed handle using `map`.
/// An unresolved reference is dropped (returned as `None`); an already-resolved
/// handle is passed through unchanged.
fn resolve_ref<T>(reference: Option<DefRef<T>>, map: &HashMap<String, Arc<T>>) -> Option<DefRef<T>> {
    match reference {
        Some(DefRef::Ref(id)) => map.get(id.as_str()).cloned().map(DefRef::Resolved),
        other => other,
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_valid() {
        let vb = extract_viewbox("0 0 200 200").unwrap();
        assert_eq!(vb.min_x.get(), 0.0);
        assert_eq!(vb.min_y.get(), 0.0);
        assert_eq!(vb.width.get(), 200.0);
        assert_eq!(vb.height.get(), 200.0);
    }

    #[test]
    fn viewbox_with_commas() {
        let vb = extract_viewbox("10,20 300,400").unwrap();
        assert_eq!(vb.min_x.get(), 10.0);
        assert_eq!(vb.min_y.get(), 20.0);
        assert_eq!(vb.width.get(), 300.0);
        assert_eq!(vb.height.get(), 400.0);
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
        assert_eq!(vb.min_x.get(), -100.0);
        assert_eq!(vb.min_y.get(), -100.0);
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
