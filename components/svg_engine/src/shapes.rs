use euclid::Transform2D;
use kurbo::{BezPath, Point as KurboPoint};
use malloc_size_of_derive::MallocSizeOf;
use webrender_api::ColorF;

use crate::lengths::SvgLength;

// ── SvgTag hierarchy ─────────────────────────────────────────────

/// SVG element type discriminant — determines how the renderer processes each node.
#[derive(Debug, Clone, PartialEq, MallocSizeOf)]
pub enum SvgTag {
    Shape(Geometry),
    Container(ContainerTag),
    Text,
    Use,
    ClipMask,
    PaintServer(PaintServerTag),
    Defs,
    Unknown,
}

impl SvgTag {
    pub fn from_str(s: &str) -> Self {
        match s {
            "rect" => SvgTag::Shape(Geometry::Rect {
                x: None, y: None, width: None, height: None, rx: None, ry: None,
            }),
            "circle" => SvgTag::Shape(Geometry::Circle {
                cx: None, cy: None, r: None,
            }),
            "ellipse" => SvgTag::Shape(Geometry::Ellipse {
                cx: None, cy: None, rx: None, ry: None,
            }),
            "line" => SvgTag::Shape(Geometry::Line {
                x1: None, y1: None, x2: None, y2: None,
            }),
            "polyline" => SvgTag::Shape(Geometry::Polyline { points: Vec::new() }),
            "polygon" => SvgTag::Shape(Geometry::Polygon { points: Vec::new() }),
            "path" => SvgTag::Shape(Geometry::Path { path: BezPath::new() }),
            "text" => SvgTag::Text,
            "use" => SvgTag::Use,
            "clipPath" => SvgTag::ClipMask,
            "mask" => SvgTag::ClipMask,
            "linearGradient" => SvgTag::PaintServer(PaintServerTag::LinearGradient),
            "radialGradient" => SvgTag::PaintServer(PaintServerTag::RadialGradient),
            "svg" => SvgTag::Container(ContainerTag::Svg),
            "g" => SvgTag::Container(ContainerTag::G),
            "a" => SvgTag::Container(ContainerTag::A),
            "switch" => SvgTag::Container(ContainerTag::Switch),
            "foreignObject" => SvgTag::Container(ContainerTag::ForeignObject),
            "defs" => SvgTag::Defs,
            _ => SvgTag::Unknown,
        }
    }

    pub fn is_basic_shape(&self) -> bool {
        matches!(self, SvgTag::Shape(_))
    }

    pub fn is_container(&self) -> bool {
        matches!(self, SvgTag::Container(_) | SvgTag::Defs)
    }
}

// ── Geometry ──────────────────────────────────────────────────────

/// Shape-specific coordinate data. The renderer matches on the variant
/// to determine both the element type and its coordinates in a single dispatch.
#[derive(Debug, Clone, PartialEq, MallocSizeOf)]
pub enum Geometry {
    Rect {
        x: Option<SvgLength>,
        y: Option<SvgLength>,
        width: Option<SvgLength>,
        height: Option<SvgLength>,
        rx: Option<SvgLength>,
        ry: Option<SvgLength>,
    },
    Circle {
        cx: Option<SvgLength>,
        cy: Option<SvgLength>,
        r: Option<SvgLength>,
    },
    Ellipse {
        cx: Option<SvgLength>,
        cy: Option<SvgLength>,
        rx: Option<SvgLength>,
        ry: Option<SvgLength>,
    },
    Line {
        x1: Option<SvgLength>,
        y1: Option<SvgLength>,
        x2: Option<SvgLength>,
        y2: Option<SvgLength>,
    },
    Polyline {
        #[ignore_malloc_size_of = "Vec<KurboPoint> does not implement MallocSizeOf"]
        points: Vec<KurboPoint>,
    },
    Polygon {
        #[ignore_malloc_size_of = "Vec<KurboPoint> does not implement MallocSizeOf"]
        points: Vec<KurboPoint>,
    },
    Path {
        #[ignore_malloc_size_of = "BezPath does not implement MallocSizeOf"]
        path: BezPath,
    },
}

// ── Container tag ─────────────────────────────────────────────────

/// Grouping elements that hold child nodes and establish rendering context boundaries.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum ContainerTag {
    // Foundation
    G,
    Svg,
    // Enhancement
    A,
    Switch,
    ForeignObject,
}

// ── Paint server tag ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum PaintServerTag {
    LinearGradient,
    RadialGradient,
    Pattern,
}

// ── Line cap / join ───────────────────────────────────────────────

/// SVG line cap style.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgLineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgLineJoin {
    Miter,
    Round,
    Bevel,
}

// ── Fill rule ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

// ── Fill / Stroke params ──────────────────────────────────────────

/// Fill parameters — color, opacity, and fill rule.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct FillParams {
    pub color: Option<ColorF>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

impl Default for FillParams {
    fn default() -> Self {
        Self {
            color: Some(ColorF::BLACK),
            opacity: 1.0,
            fill_rule: FillRule::NonZero,
        }
    }
}

/// Stroke parameters — color, width, opacity, dash, linecap/join.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct StrokeParams {
    pub color: Option<ColorF>,
    pub width: f32,
    pub opacity: f32,
    pub dasharray: Option<Vec<f32>>,
    pub dashoffset: f32,
    pub linecap: SvgLineCap,
    pub linejoin: SvgLineJoin,
    pub miterlimit: f32,
}

impl Default for StrokeParams {
    fn default() -> Self {
        Self {
            color: None,
            width: 1.0,
            opacity: 1.0,
            dasharray: None,
            dashoffset: 0.0,
            linecap: SvgLineCap::Butt,
            linejoin: SvgLineJoin::Miter,
            miterlimit: 4.0,
        }
    }
}

// ── Render hints ──────────────────────────────────────────────────

/// Quality-versus-performance rendering hints.
#[derive(Debug, Clone, Copy, MallocSizeOf)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}

/// Rendering quality hints — Phase 2 will add ColorInterpolation,
/// ShapeRendering, TextRendering, ImageRendering.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct RenderHints {
    pub vector_effect: VectorEffect,
}

impl Default for RenderHints {
    fn default() -> Self {
        Self {
            vector_effect: VectorEffect::None,
        }
    }
}

// ── Visibility / Display / PaintOrder ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum Display {
    Inline,
    None,
}

impl Default for Display {
    fn default() -> Self {
        Self::Inline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum PaintOrder {
    FillStroke,
    StrokeFill,
}

impl Default for PaintOrder {
    fn default() -> Self {
        Self::FillStroke
    }
}

// ── NodeEffects ───────────────────────────────────────────────────

/// Per-element effect parameters — the source values on each node.
/// These are distinct from RenderState which accumulates them during tree walk.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct NodeEffects {
    pub transform: Option<Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub mask: Option<String>,
}

// ── Render node ───────────────────────────────────────────────────

/// A single element in the rendering tree. Nodes form a recursive tree
/// through the `children` field. Each node stores only data intrinsic to
/// its element; inherited state is tracked via RenderState during tree walk.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct SvgRenderNode {
    pub tag: SvgTag,
    pub effects: Option<Box<NodeEffects>>,
    pub fill: FillParams,
    pub stroke: StrokeParams,
    pub hints: RenderHints,
    pub opacity: f32,
    pub visibility: Visibility,
    pub display: Display,
    pub paint_order: PaintOrder,
    pub children: Vec<SvgRenderNode>,
}

impl SvgRenderNode {
    pub fn new(tag: SvgTag) -> Self {
        Self {
            tag,
            effects: None,
            fill: FillParams::default(),
            stroke: StrokeParams::default(),
            hints: RenderHints::default(),
            opacity: 1.0,
            visibility: Visibility::default(),
            display: Display::default(),
            paint_order: PaintOrder::default(),
            children: Vec::new(),
        }
    }
}

// ── Viewport info ─────────────────────────────────────────────────

/// Resolved viewport dimensions derived from SVG width, height, and viewBox.
#[derive(Debug, Clone, Copy, MallocSizeOf)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
}

// ── Render tree ───────────────────────────────────────────────────

/// Top-level container for a single SVG document fragment.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
}

// ── Legacy alias (used by layout integration during transition) ────

/// Flat input for direct rendering (Phase 1). Will be replaced by SvgRenderTree walk.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct SvgRenderInput {
    pub tag: SvgTag,
    pub geometry: Option<Box<Geometry>>,
    pub fill: FillParams,
    pub stroke: StrokeParams,
    pub effects: Option<Box<NodeEffects>>,
    pub opacity: f32,
}

impl SvgRenderInput {
    pub fn new(tag: SvgTag) -> Self {
        Self {
            tag,
            geometry: None,
            fill: FillParams::default(),
            stroke: StrokeParams::default(),
            effects: None,
            opacity: 1.0,
        }
    }

    pub fn extract_geometry(&self) -> Option<&Geometry> {
        self.geometry.as_deref()
    }
}
