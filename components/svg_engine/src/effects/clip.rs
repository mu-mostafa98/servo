/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Clip path and mask resolution — converts SVG `clip-path` and `mask`
//! references into WebRender [`ClipChain`] IDs.
//!
//! **Single responsibility:** given a render node and its effect
//! definitions, produce the clip chains needed for rendering.  No
//! tree walking, no display list management beyond clip definition.

use webrender_api::{
    ClipChainId, ClipMode, ComplexClipRegion, DisplayListBuilder, SpatialId,
    units::LayoutPoint,
};

use crate::render_tree::{ClipPathUnits, SvgRenderNode};
use crate::renderer::ClipMaskProvider;
use crate::renderer::clip_chain_option;
use crate::shapes::ClipGeometry;

// ======================= Clip Path Resolution =======================

/// If the node has a `clip-path` reference, resolve it and build a
/// WebRender clip chain.  Otherwise returns `parent_clip_chain`
/// unchanged.
pub(crate) fn resolve_node_clip_path(
    node: &SvgRenderNode,
    clips: &impl ClipMaskProvider,
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
            ClipGeometry::RoundedRect { bounds, radii } => {
                wr.define_clip_rounded_rect(
                    spatial_id,
                    ComplexClipRegion { rect: bounds, radii, mode: ClipMode::Clip },
                )
            },
            ClipGeometry::Polygon { bounds } => {
                // WebRender 0.69 does not support arbitrary polygon clip paths
                // natively.  Fall back to bounding-rect clip, which is safe and
                // at least restricts the painted area to the shape's bounding box.
                wr.define_clip_rect(spatial_id, bounds)
            },
        };

        let parent = clip_chain_option(current_chain);
        current_chain = wr.define_clip_chain(parent, [clip_id]);
    }
    current_chain
}

// ======================= Mask Resolution =======================

/// Build individual clip chains for each mask shape, one per shape.
///
/// Returns `None` when no mask is present.
/// Returns `Some(vec![...])` with one clip chain per mask shape.
///
/// Each clip chain combines the parent clip AND one mask shape.
/// Rendering the shape once per mask clip achieves union (OR)
/// behavior.
pub(crate) fn build_mask_clips(
    node: &SvgRenderNode,
    clips: &impl ClipMaskProvider,
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
            ClipGeometry::RoundedRect { bounds, radii } => {
                wr.define_clip_rounded_rect(
                    spatial_id,
                    ComplexClipRegion { rect: bounds, radii, mode: ClipMode::Clip },
                )
            },
            ClipGeometry::Polygon { bounds } => {
                // Same bounding-rect fallback as clip-path above.
                wr.define_clip_rect(spatial_id, bounds)
            },
        };

        let chain = wr.define_clip_chain(
            clip_chain_option(parent_clip_chain),
            [clip_id],
        );
        clips.push(chain);
    }

    if clips.is_empty() { None } else { Some(clips) }
}
