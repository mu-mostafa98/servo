use euclid::Transform2D;
use malloc_size_of_derive::MallocSizeOf;
use webrender_api::ColorF;

// ── Fill rule ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum FillRule {
    NonZero,
    EvenOdd,
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

// ── Fill / Stroke params ──────────────────────────────────────────

/// Fill parameters — color, opacity, fill rule, and paint server.
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

/// Rendering quality hints.
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

// ── NodeStyles ────────────────────────────────────────────────────

/// Bundled set of all style properties carried by a render node.
/// This is the single source of truth for per-node rendering parameters,
/// including effects (transform, clip, mask) which are accumulated into
/// RenderState during tree walk.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct NodeStyles {
    pub fill: FillParams,
    pub stroke: StrokeParams,
    pub hints: RenderHints,
    pub opacity: f32,
    pub visibility: Visibility,
    pub display: Display,
    pub paint_order: PaintOrder,
    pub effects: Option<Box<NodeEffects>>,
}

impl Default for NodeStyles {
    fn default() -> Self {
        Self {
            fill: FillParams::default(),
            stroke: StrokeParams::default(),
            hints: RenderHints::default(),
            opacity: 1.0,
            visibility: Visibility::default(),
            display: Display::default(),
            paint_order: PaintOrder::default(),
            effects: None,
        }
    }
}
