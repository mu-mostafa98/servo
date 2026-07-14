/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::LayoutRect;
use crate::renderer::{RenderContext, make_common_props, to_colorf};

pub(crate) fn fill_rect(bounds: LayoutRect, ctx: &mut RenderContext) {
    let Some(fill) = &ctx.style.fill else { return };
    let opacity = fill.opacity * ctx.style.opacity;
    if let Some(svg_color) = fill.color {
        let mut color = to_colorf(&svg_color);
        color.a *= opacity;
        let common = make_common_props(bounds, ctx.spatial_id, ctx.clip_chain_id);
        ctx.wr.push_rect(&common, bounds, color);
    }
}
