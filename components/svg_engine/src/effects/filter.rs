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
            FilterPrimitive::Saturate(s) => {
                let s = s.clamp(0.0, 10.0);
                let lum_r = 0.213;
                let lum_g = 0.715;
                let lum_b = 0.072;
                ops.push(FilterOp::ColorMatrix([
                    lum_r + (1.0 - lum_r) * s, lum_g * (1.0 - s), lum_b * (1.0 - s), 0.0, 0.0,
                    lum_r * (1.0 - s), lum_g + (1.0 - lum_g) * s, lum_b * (1.0 - s), 0.0, 0.0,
                    lum_r * (1.0 - s), lum_g * (1.0 - s), lum_b + (1.0 - lum_b) * s, 0.0, 0.0,
                    0.0, 0.0, 0.0, 1.0, 0.0,
                ]));
            },
            FilterPrimitive::LuminanceToAlpha => {
                ops.push(FilterOp::ColorMatrix([
                    0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0,
                    0.213, 0.715, 0.072, 0.0, 0.0,
                ]));
            },
            FilterPrimitive::Offset(dx, dy) => {
                // feOffset shifts the input. Without the full SVG filter graph,
                // we approximate by translating the content via a drop-shadow
                // with zero blur and transparent color (which shifts the content).
                // Reference: https://www.w3.org/TR/filter-effects-1/#feOffsetElement
                ops.push(FilterOp::DropShadow(webrender_api::Shadow {
                    blur_radius: 0.0,
                    offset: webrender_api::units::LayoutVector2D::new(*dx, *dy),
                    color: webrender_api::ColorF::new(0.0, 0.0, 0.0, 0.0),
                }));
            },
            FilterPrimitive::Flood(r, g, b, a) => {
                ops.push(FilterOp::Flood(webrender_api::ColorF::new(*r, *g, *b, *a)));
            },
            FilterPrimitive::Composite(composite_kind) => {
                // Note: feComposite without the full SVG filter graph is
                // represented as an identity op. The proper implementation
                // requires building a FilterOpGraphNode with multiple inputs.
                // For now, we keep the filter recognized and push a
                // placeholder.
                ops.push(FilterOp::Identity);
                // Log the unsupported composite kind for debugging.
                log::debug!("feComposite ({:?}) not yet fully supported in SVG engine", composite_kind);
            },
            FilterPrimitive::Tile => {
                // feTile repeats the input to fill the filter region.
                // Proper support requires the SVG filter graph.
                ops.push(FilterOp::Identity);
                log::debug!("feTile not yet fully supported in SVG engine");
            },
            FilterPrimitive::Image(img_kind) => {
                // feImage loads an external image or renders a referenced element.
                // Proper support requires the SVG filter graph and image loading.
                // For now, keep the filter recognized with a placeholder.
                let img_id = match img_kind {
                    crate::render_tree::FeImageKind::FragmentRef(id) => format!("#{}", id),
                    crate::render_tree::FeImageKind::ExternalUrl(url) => url.clone(),
                };
                log::debug!("feImage ({}) not yet fully supported in SVG engine", img_id);
                ops.push(FilterOp::Identity);
            },
        }
    }
    if ops.is_empty() { None } else { Some(ops) }
}
