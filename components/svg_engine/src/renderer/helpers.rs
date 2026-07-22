/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::Color as SvgColor;
use webrender_api::units::LayoutRect;
use webrender_api::{ClipChainId, ColorF, CommonItemProperties, SpaceAndClipInfo, SpatialId};

use crate::renderer::render_trait::RenderContext;
use crate::style::hints::{ShapeRendering, VectorEffect};

pub(crate) const ZERO_LENGTH_EPSILON: f32 = 0.001;

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

pub(crate) fn effective_stroke_width(ctx: &RenderContext, width: f32) -> f32 {
    if let Some(hints) = &ctx.style.render_hints &&
        let Some(VectorEffect::NonScalingStroke) = hints.vector_effect
    {
        let scale = ctx.accumulated_scale.max(0.01);
        return width / scale;
    }
    width
}

pub(crate) fn to_colorf(c: &SvgColor) -> ColorF {
    ColorF::new(
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
        c.alpha as f32 / 255.0,
    )
}

pub(crate) fn clip_chain_option(id: ClipChainId) -> Option<ClipChainId> {
    if id == ClipChainId::INVALID {
        None
    } else {
        Some(id)
    }
}

pub(crate) fn paint_order_stroke_before_fill(ctx: &RenderContext) -> bool {
    ctx.style
        .render_hints
        .as_ref()
        .and_then(|h| h.paint_order)
        .map(|p| p.stroke_before_fill())
        .unwrap_or(false)
}

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
