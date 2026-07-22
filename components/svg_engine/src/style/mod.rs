/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub mod color;
pub(crate) mod fill;
pub mod gradient;
pub(crate) mod hints;
pub(crate) mod node_effects;
pub(crate) mod stroke;
pub mod transform_ops;
pub(crate) mod visibility;

pub use self::fill::{FillParams, FillRule};
pub use self::hints::{
    ColorInterpolation, ColorRendering, PaintOrder, RenderHints, ShapeRendering, VectorEffect,
};
pub use self::node_effects::NodeEffects;
pub use self::stroke::{LineCap, LineJoin, StrokeParams};
pub use self::visibility::{Display, Visibility};

#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub visibility: Visibility,
    pub display: Display,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
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
            stroke: None,
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
