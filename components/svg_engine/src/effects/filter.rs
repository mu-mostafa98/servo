/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG filter resolution — converts `<filter>` primitive references
//! into WebRender [`FilterOp`] lists.
//!
//! **Single responsibility:** given a render node and its filter
//! definitions, produce the list of WebRender filter operations.
//! No tree walking, no display list management beyond filter op
//! construction.

use webrender_api::FilterOp;

use crate::render_tree::{FilterPrimitive, SvgRenderNode};
use crate::renderer::FilterProvider;

/// If the node references a filter, return the list of WebRender
/// [`FilterOp`]s.  Returns `None` when no filter is present, the
/// referenced filter definition is missing, or the filter resolves
/// to an empty op list.
pub(crate) fn get_filter_ops(
    node: &SvgRenderNode,
    filters: &dyn FilterProvider,
) -> Option<Vec<FilterOp>> {
    let effects = node.style.effects.as_ref()?;
    let filter_id = effects.filter.as_ref()?;
    let filter_def = match filters.filter(filter_id) {
        Some(d) => d,
        None => {
            log::warn!("filter \"{}\" not found in definitions", filter_id);
            return None;
        },
    };

    let mut ops = Vec::new();
    for prim in &filter_def.primitives {
        match prim {
            FilterPrimitive::GaussianBlur(sdx, sdy) => {
                ops.push(FilterOp::Blur(*sdx, *sdy));
            },
            FilterPrimitive::DropShadow(dx, dy, sd, r, g, b, a) => {
                ops.push(FilterOp::DropShadow(webrender_api::Shadow {
                    blur_radius: *sd,
                    offset: webrender_api::units::LayoutVector2D::new(*dx, *dy),
                    color: webrender_api::ColorF::new(*r, *g, *b, *a),
                }));
            },
            FilterPrimitive::ColorMatrix(matrix) => {
                ops.push(FilterOp::ColorMatrix(*matrix));
            },
        }
    }
    if ops.is_empty() { None } else { Some(ops) }
}
