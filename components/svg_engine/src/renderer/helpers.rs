/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared rendering helpers — color conversion, clip chain utilities,
//! stroke width adjustment, and rendering hint resolution.

use svgtypes::Color as SvgColor;
use webrender_api::units::LayoutRect;
use webrender_api::{ClipChainId, ColorF, CommonItemProperties, SpaceAndClipInfo, SpatialId};

use crate::renderer::render_trait::RenderContext;
use crate::style::hints::{ShapeRendering, VectorEffect};

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
    CommonItemProperties::new(bounds, SpaceAndClipInfo { spatial_id, clip_chain_id })
}

/// Return the effective stroke width, adjusted for `vector-effect: non-scaling-stroke`.
pub(crate) fn effective_stroke_width(ctx: &RenderContext, width: f32) -> f32 {
    if let Some(hints) = &ctx.style.render_hints &&
        let Some(VectorEffect::NonScalingStroke) = hints.vector_effect
    {
        let scale = ctx.accumulated_scale.max(0.01);
        return width / scale;
    }
    width
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
    if id == ClipChainId::INVALID { None } else { Some(id) }
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
    match ctx.style.render_hints.as_ref().and_then(|h| h.shape_rendering) {
        Some(ShapeRendering::GeometricPrecision) => precision,
        Some(ShapeRendering::OptimizeSpeed) => speed,
        _ => default,
    }
}
