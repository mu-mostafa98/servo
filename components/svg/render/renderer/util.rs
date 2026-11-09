/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared rendering helpers — color conversion, clip chain utilities,
//! stroke width adjustment, and rendering hint resolution.

use svgtypes::Color as SvgColor;
use webrender_api::units::LayoutRect;
use webrender_api::{ClipChainId, ColorF, CommonItemProperties, SpaceAndClipInfo, SpatialId};

use crate::model::style::{ColorInterpolation, FillRule, NodeStyle, ShapeRendering, VectorEffect};
use crate::render::renderer::render_trait::RenderContext;

/// Epsilon threshold for treating a vector length as zero.
/// Used to guard against division-by-zero in gradient projection
/// and zero-length line detection.
pub(crate) const ZERO_LENGTH_EPSILON: f32 = 0.001;

/// Construct a [`CommonItemProperties`] from an origin-space rect and clip info.
pub(crate) fn make_common_props(
    bounds: LayoutRect,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
) -> CommonItemProperties {
    CommonItemProperties::new(
        bounds,
        SpaceAndClipInfo {
            spatial_id,
            clip_chain_id,
        },
    )
}

/// Whether the current style requests `vector-effect: non-scaling-stroke`.
fn non_scaling_stroke(ctx: &RenderContext) -> bool {
    matches!(
        ctx.style
            .render_hints
            .as_ref()
            .and_then(|h| h.vector_effect),
        Some(VectorEffect::NonScalingStroke)
    )
}

/// The total user-space → screen scale per axis: the accumulated transform
/// scale times the viewBox → viewport scale, clamped away from zero.
fn total_scale_xy(ctx: &RenderContext) -> (f32, f32) {
    let (tx, ty) = ctx.accumulated_scale;
    let (vx, vy) = ctx.viewbox_scale;
    ((tx * vx).max(0.01), (ty * vy).max(0.01))
}

/// Return the effective stroke width, adjusted for `vector-effect: non-scaling-stroke`.
///
/// This is the scalar form, used by strokes that can only carry a single width
/// (line segments, polyline decomposition, crisp-edge rasters). It compensates
/// with the largest axis scale, which is correct for uniform scaling.
pub(crate) fn effective_stroke_width(ctx: &RenderContext, width: f32) -> f32 {
    if non_scaling_stroke(ctx) {
        let (sx, sy) = total_scale_xy(ctx);
        return width / sx.max(sy);
    }
    width
}

/// Return per-axis effective stroke widths `(x, y)` for `vector-effect:
/// non-scaling-stroke`, to be used by `push_border` on rectangles and rounded
/// rectangles / ellipses.
///
/// A non-uniform transform (e.g. `scale(2, 0.6)`) scales vertical edges by the
/// x factor and horizontal edges by the y factor, so the two directions must be
/// compensated independently for the stroke to stay constant in screen space.
/// Without `non-scaling-stroke`, both are just the raw width.
pub(crate) fn effective_stroke_widths(ctx: &RenderContext, width: f32) -> (f32, f32) {
    if non_scaling_stroke(ctx) {
        let (sx, sy) = total_scale_xy(ctx);
        (width / sx, width / sy)
    } else {
        (width, width)
    }
}

/// Convert an [`SvgColor`] to a [`ColorF`].
pub(crate) fn to_colorf(c: &SvgColor) -> ColorF {
    ColorF::new(
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
        c.alpha as f32 / 255.0,
    )
}

/// Convert a [`ClipChainId`] to an [`Option`], returning `None` for
/// [`ClipChainId::INVALID`] and `Some(id)` otherwise.
pub(crate) fn clip_chain_option(id: ClipChainId) -> Option<ClipChainId> {
    if id == ClipChainId::INVALID {
        None
    } else {
        Some(id)
    }
}

/// Whether stroke should be rendered before fill, based on `paint-order`.
pub(crate) fn paint_order_stroke_before_fill(ctx: &RenderContext) -> bool {
    ctx.style
        .render_hints
        .as_ref()
        .and_then(|h| h.paint_order)
        .map(|p| p.stroke_before_fill())
        .unwrap_or(false)
}

/// Map the shape-rendering hint to a numeric parameter.
pub(crate) fn shape_rendering_value(
    ctx: &RenderContext,
    precision: f32,
    speed: f32,
    default: f32,
) -> f32 {
    match ctx
        .style
        .render_hints
        .as_ref()
        .and_then(|h| h.shape_rendering)
    {
        Some(ShapeRendering::GeometricPrecision) => precision,
        Some(ShapeRendering::OptimizeSpeed) => speed,
        _ => default,
    }
}

/// Whether the current context requests `shape-rendering="crispEdges"`,
/// which asks the renderer to disable anti-aliasing in favour of sharp,
/// aliased edges.
pub(crate) fn crisp_edges(ctx: &RenderContext) -> bool {
    matches!(
        ctx.style
            .render_hints
            .as_ref()
            .and_then(|h| h.shape_rendering),
        Some(ShapeRendering::CrispEdges)
    )
}

/// The `color-interpolation` hint for the current context, defaulting to sRGB.
pub(crate) fn color_interpolation(ctx: &RenderContext) -> ColorInterpolation {
    color_interpolation_of_style(ctx.style)
}

/// The `color-interpolation` hint for a given style, defaulting to sRGB.
pub(crate) fn color_interpolation_of_style(style: &NodeStyle) -> ColorInterpolation {
    style
        .render_hints
        .as_ref()
        .and_then(|h| h.color_interpolation)
        .unwrap_or(ColorInterpolation::Srgb)
}

/// The effective fill rule for the current style, defaulting to non-zero.
pub(crate) fn fill_rule(ctx: &RenderContext) -> FillRule {
    ctx.style
        .fill
        .as_ref()
        .map(|f| f.fill_rule)
        .unwrap_or(FillRule::NonZero)
}
