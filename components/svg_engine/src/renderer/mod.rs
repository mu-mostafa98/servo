/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape renderers — convert SVG shapes into WebRender display list commands.
//!
//! Each shape in [`crate::shapes`] implements the [`Render`] trait, which
//! produces the corresponding [`webrender_api::DisplayListBuilder`] commands.
//! The [`crate::traversal`] module calls [`Render::render`] during SVG tree
//! traversal — there is no central dispatch match to maintain.

pub(crate) mod rect;
pub(crate) mod ellipse;
pub(crate) mod circle;
pub(crate) mod line;
pub(crate) mod polyline;
pub(crate) mod polygon;
pub(crate) mod path;
pub(crate) mod transform;
pub(crate) mod gradient;
pub(crate) mod fill;
pub(crate) mod stroke;
pub(crate) mod pattern;

use svgtypes::Color as SvgColor;
use webrender_api::{
    ClipChainId, ColorF, CommonItemProperties, DisplayListBuilder, SpaceAndClipInfo, SpatialId,
    units::LayoutPoint, units::LayoutRect,
};

use crate::shapes::*;
use crate::style::NodeStyle;
use crate::style::hints::{VectorEffect, ShapeRendering};
use crate::render_tree::{ClipPathDef, MaskDef, PatternDef};
use crate::style::gradient::GradientDef;

// ======================= Resource Provider Traits =======================

/// Provider for paint-server resources (gradients and patterns).
pub(crate) trait PaintResourceProvider {
    fn gradient(&self, id: &str) -> Option<&GradientDef>;
    fn pattern(&self, id: &str) -> Option<&PatternDef>;
    fn has_pattern(&self, id: &str) -> bool {
        self.pattern(id).is_some()
    }
}

/// Provider for clip-path and mask resources.
pub(crate) trait ClipMaskProvider {
    fn clip_path(&self, id: &str) -> Option<&ClipPathDef>;
    fn mask(&self, id: &str) -> Option<&MaskDef>;
}

/// Provider for filter-effect resources.
pub(crate) trait FilterProvider {
    fn filter(&self, id: &str) -> Option<&crate::render_tree::FilterDef>;
}

// ----------------------- Render Context -----------------------

/// Bundled rendering parameters passed to every [`Render::render`] call.
pub(crate) struct RenderContext<'a> {
    pub style: &'a NodeStyle,
    pub svg_origin: LayoutPoint,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub wr: &'a mut DisplayListBuilder,
    /// Paint resource provider, used internally by fill/stroke helpers.
    /// Shape `Render` impls should NOT access this field directly.
    /// Instead, call `fill::fill_rect(…)` or `stroke::stroke_rect(…)`
    /// which internally use this field to look up paint servers.
    pub paints: &'a dyn PaintResourceProvider,
    /// Accumulated transform scale from all ancestor transforms.
    /// Used by `vector-effect: non-scaling-stroke` to compensate stroke width.
    pub accumulated_scale: f32,
}

// ----------------------- Render Trait -----------------------

/// Convert an SVG shape into WebRender display list commands.
///
/// Every SVG shape type implements this trait so that traversal
/// code can call `shape.render(...)` without a central match.
///
/// # Preconditions
///
/// * `ctx.style` must be a valid [`NodeStyle`] (may have `None` fill
///   or stroke — those are silently skipped).
/// * `ctx.svg_origin` must be in absolute WebRender layout space.
/// * `ctx.spatial_id` and `ctx.clip_chain_id` must be valid (they may
///   be `ClipChainId::INVALID`, which WebRender handles as "no clip").
/// * `ctx.wr` must be a mutable [`DisplayListBuilder`] in a valid state.
///
/// # Postconditions
///
/// * Zero or more display list commands are pushed onto `ctx.wr`.
/// * `ctx.wr` is left in a valid state (all reference frames popped).
/// * `ctx.spatial_id` and `ctx.clip_chain_id` are not modified.
/// * `ctx.svg_origin` is not modified (shape coords are relative to it).
///
/// # Invariants
///
/// * Per the SVG 2 specification:
///   - `<line>` elements have no fill geometry and MUST NOT emit fill
///     commands even when `ctx.style.fill` is `Some`.
///   - `<circle>`, `<ellipse>`, `<rect>`, `<polygon>`, `<polyline>`,
///     and `<path>` MAY emit fill commands when `ctx.style.fill` is `Some`.
///   - All shape types MAY emit stroke commands when `ctx.style.stroke`
///     is `Some` and the stroke width is positive.
/// * Delegation chains (Circle → Ellipse → Rectangle, Polygon → Polyline,
///   Path → Polyline) preserve all invariants of the ultimate callee.
/// * [`RenderContext::paints`] is only accessed by `fill::` and `stroke::`
///   helper functions — shape `Render` impls should not read it directly.
///
/// # Liskov Substitution
///
/// Any shape's [`Render`] impl can be substituted for any other without
/// violating the invariants above.  Shape-specific behaviors (e.g. [`Line`]
/// having no fill) are mandated by the SVG specification, not by this
/// trait contract.
pub(crate) trait Render {
    /// Emit WebRender display list commands for this shape.
    fn render(&self, ctx: &mut RenderContext);
}

// ----------------------- Delegation -----------------------

impl Render for Shape {
    fn render(&self, ctx: &mut RenderContext) {
        match self {
            Shape::Rect(r) => r.render(ctx),
            Shape::Circle(c) => c.render(ctx),
            Shape::Ellipse(e) => e.render(ctx),
            Shape::Line(l) => l.render(ctx),
            Shape::Polyline(p) => p.render(ctx),
            Shape::Polygon(p) => p.render(ctx),
            Shape::Path(p) => p.render(ctx),
        }
    }
}

// ----------------------- Shared Helpers -----------------------

/// Construct a [`CommonItemProperties`] from an origin-space rect and clip info.
///
/// This is a thin convenience wrapper used by multiple renderers to
/// avoid repeating the same `SpaceAndClipInfo` construction.
pub(crate) fn make_common_props(
    bounds: LayoutRect,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) -> CommonItemProperties {
    CommonItemProperties::new(bounds, SpaceAndClipInfo { spatial_id, clip_chain_id })
}

/// Return the effective stroke width, adjusted for `vector-effect: non-scaling-stroke`.
///
/// When `non-scaling-stroke` is active, divides the width by the accumulated
/// transform scale so that the stroke appears the same visual width regardless
/// of any ancestor or element-level scale transforms.
pub(crate) fn effective_stroke_width(ctx: &RenderContext, width: f32) -> f32 {
    if let Some(hints) = &ctx.style.render_hints
        && let Some(VectorEffect::NonScalingStroke) = hints.vector_effect {
            let scale = ctx.accumulated_scale.max(0.01);
            return width / scale;
        }
    width
}

/// Convert an [`svgtypes::Color`] to a [`webrender_api::ColorF`].
///
/// This is the single conversion point from the engine's canonical color type
/// to WebRender's color type.  All renderer code should use this helper
/// rather than inlining the conversion.
pub(crate) fn to_colorf(c: &SvgColor) -> ColorF {
    ColorF::new(
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
        c.alpha as f32 / 255.0,
    )
}

/// Epsilon threshold for treating a vector length as zero.
/// Used to guard against division-by-zero in gradient projection
/// and zero-length line detection.
pub(crate) const ZERO_LENGTH_EPSILON: f32 = 0.001;

/// Convert a [`ClipChainId`] to an [`Option`], returning `None` for
/// [`ClipChainId::INVALID`] and `Some(id)` otherwise.
///
/// This is the single source for the common pattern
/// `if id == ClipChainId::INVALID { None } else { Some(id) }`
/// that appears throughout the crate.
pub(crate) fn clip_chain_option(id: ClipChainId) -> Option<ClipChainId> {
    if id == ClipChainId::INVALID { None } else { Some(id) }
}

/// Map the shape-rendering hint to a numeric parameter.
///
/// Returns `precision` when [`ShapeRendering::GeometricPrecision`],
/// `speed` when [`ShapeRendering::OptimizeSpeed`],
/// and `default` otherwise.
pub(crate) fn shape_rendering_value(
    ctx: &RenderContext,
    precision: f32,
    speed: f32,
    default: f32,
) -> f32 {
    match ctx.style.render_hints.as_ref()
        .and_then(|h| h.shape_rendering)
    {
        Some(ShapeRendering::GeometricPrecision) => precision,
        Some(ShapeRendering::OptimizeSpeed) => speed,
        _ => default,
    }
}
