use webrender_api::units::LayoutRect;
use webrender_api::{ClipChainId, CommonItemProperties, SpaceAndClipInfo};

use crate::renderer::{RenderContext, to_colorf};

pub(crate) fn fill_rect(bounds: LayoutRect, clip: ClipChainId, ctx: &mut RenderContext) {
    let Some(fill) = &ctx.style.fill else {
        return;
    };

    if let Some(svg_color) = fill.color {
        let mut color = to_colorf(&svg_color);
        color.a *= fill.opacity * ctx.style.opacity;
        let common = CommonItemProperties::new(
            bounds,
            SpaceAndClipInfo {
                spatial_id: ctx.spatial_id,
                clip_chain_id: clip,
            },
        );
        ctx.wr.push_rect(&common, bounds, color);
    }
}
