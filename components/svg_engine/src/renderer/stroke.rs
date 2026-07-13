use webrender_api::units::{LayoutRect, LayoutSideOffsets, LayoutSize};
use webrender_api::{BorderDetails, BorderRadius, BorderSide, BorderStyle, NormalBorder};

use crate::renderer::{RenderContext, make_common_props, to_colorf};

pub(crate) fn stroke_rect(
    bounds: LayoutRect,
    radii: Option<BorderRadius>,
    ctx: &mut RenderContext,
) {
    let Some(stroke) = &ctx.style.stroke else {
        return;
    };

    if let Some(svg_color) = stroke.color {
        if stroke.width <= 0.0 {
            return;
        }

        let mut color = to_colorf(&svg_color);
        color.a *= stroke.opacity * ctx.style.opacity;

        let widths = LayoutSideOffsets::new_all_same(stroke.width);
        let border_side = BorderSide {
            color,
            style: BorderStyle::Solid,
        };
        let details = BorderDetails::Normal(NormalBorder {
            left: border_side.clone(),
            right: border_side.clone(),
            top: border_side.clone(),
            bottom: border_side,
            radius: radii.unwrap_or(BorderRadius {
                top_left: LayoutSize::new(0.0, 0.0),
                top_right: LayoutSize::new(0.0, 0.0),
                bottom_left: LayoutSize::new(0.0, 0.0),
                bottom_right: LayoutSize::new(0.0, 0.0),
            }),
            do_aa: true,
        });

        let common = make_common_props(bounds, ctx.spatial_id, ctx.clip_chain_id);
        ctx.wr.push_border(&common, bounds, widths, details);
    }
}
