pub mod color;
pub(crate) mod fill;
pub(crate) mod hints;
pub(crate) mod node_effects;
pub(crate) mod visibility;

pub use self::fill::{FillParams, FillRule};
pub use self::hints::{
    ColorInterpolation, ColorRendering, ImageRendering, PaintOrder, RenderHints, ShapeRendering,
    TextRendering, VectorEffect,
};
pub use self::node_effects::NodeEffects;
pub use self::visibility::{Display, Visibility};

#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub visibility: Visibility,
    pub display: Display,
    pub fill: Option<FillParams>,
    pub render_hints: Option<RenderHints>,
    pub effects: Option<NodeEffects>,
    pub opacity: f32,
}

impl Default for NodeStyle {
    fn default() -> Self {
        NodeStyle {
            visibility: Visibility::Visible,
            display: Display::Inline,
            fill: None,
            render_hints: None,
            effects: None,
            opacity: 1.0,
        }
    }
}

impl NodeStyle {
    pub fn is_visible(&self) -> bool {
        matches!(self.visibility, Visibility::Visible)
    }

    pub fn is_displayed(&self) -> bool {
        !matches!(self.display, Display::None)
    }
}
