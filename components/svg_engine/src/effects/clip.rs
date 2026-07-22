/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::LayoutPoint;
use webrender_api::{ClipChainId, ClipMode, ComplexClipRegion, DisplayListBuilder, SpatialId};

use crate::render_tree::{ClipPathUnits, SvgRenderNode};
use crate::renderer::{ClipMaskProvider, clip_chain_option};
use crate::shapes::ClipGeometry;

pub(crate) fn resolve_node_clip_path(
    node: &SvgRenderNode,
    clips: &dyn ClipMaskProvider,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> ClipChainId {
    let Some(ref effects) = node.style.effects else {
        return parent_clip_chain;
    };
    let Some(ref clip_path_id) = effects.clip_path else {
        return parent_clip_chain;
    };
    let Some(clip_def) = clips.clip_path(clip_path_id) else {
        log::warn!("clip-path \"{}\" not found in definitions", clip_path_id);
        return parent_clip_chain;
    };

    let mut current_chain = parent_clip_chain;
    for shape in &clip_def.shapes {
        let Some(geometry) = shape.clip_info(svg_origin, clip_def.clip_path_units) else {
            continue;
        };

        let clip_id = match geometry {
            ClipGeometry::RoundedRect { bounds, radii } => wr.define_clip_rounded_rect(
                spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii,
                    mode: ClipMode::Clip,
                },
            ),
            ClipGeometry::Polygon { bounds } => wr.define_clip_rect(spatial_id, bounds),
        };

        let parent = clip_chain_option(current_chain);
        current_chain = wr.define_clip_chain(parent, [clip_id]);
    }
    current_chain
}

pub(crate) fn build_mask_clips(
    node: &SvgRenderNode,
    clips: &dyn ClipMaskProvider,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> Option<Vec<ClipChainId>> {
    let effects = node.style.effects.as_ref()?;
    let mask_id = effects.mask.as_ref()?;
    let mask_def = match clips.mask(mask_id) {
        Some(d) => d,
        None => {
            log::warn!("mask \"{}\" not found in definitions", mask_id);
            return None;
        },
    };

    let mut clips = Vec::with_capacity(mask_def.shapes.len());
    for (shape, _style) in &mask_def.shapes {
        let Some(geometry) = shape.clip_info(svg_origin, ClipPathUnits::UserSpaceOnUse) else {
            continue;
        };

        let clip_id = match geometry {
            ClipGeometry::RoundedRect { bounds, radii } => wr.define_clip_rounded_rect(
                spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii,
                    mode: ClipMode::Clip,
                },
            ),
            ClipGeometry::Polygon { bounds } => wr.define_clip_rect(spatial_id, bounds),
        };

        let chain = wr.define_clip_chain(clip_chain_option(parent_clip_chain), [clip_id]);
        clips.push(chain);
    }

    if clips.is_empty() { None } else { Some(clips) }
}
