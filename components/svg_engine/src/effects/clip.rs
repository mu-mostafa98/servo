/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Clip path and mask resolution — converts SVG `clip-path` and `mask`
//! references into WebRender [`ClipChain`] IDs and vello-rasterized clip
//! geometry.
//!
//! **Single responsibility:** given a render node and its effect definitions,
//! produce the clip chains and [`ComplexClip`] geometry needed for rendering.
//! No tree walking, no display list management beyond clip definition.

use webrender_api::units::LayoutPoint;
use webrender_api::{
    ClipChainId, ClipMode, ComplexClipRegion, DisplayListBuilder, SpatialId,
};

use crate::render_tree::{ClipPathUnits, SvgRenderNode};
use crate::renderer::{ClipMaskProvider, clip_chain_option};
use crate::shapes::{ClipGeometry, ComplexClip};

// ======================= Clip Path Resolution =======================

/// A single mask shape resolved to either a WebRender clip chain (rect /
/// rounded-rect) or a [`ComplexClip`] applied during rasterization.
pub(crate) struct MaskClip {
    /// WebRender clip chain for rect/rounded-rect mask shapes (native shapes).
    pub chain: ClipChainId,
    /// Complex (polygon/path) mask geometry, applied during rasterization.
    pub complex: Option<ComplexClip>,
}

/// Resolve a node's `clip-path` reference into a WebRender clip chain plus any
/// complex (polygon/path) clips that must be applied during rasterization.
///
/// Returns `(clip_chain, complex_clips)`. When no clip-path is present, both
/// are returned unchanged (`parent_clip_chain` and an empty list).
pub(crate) fn resolve_node_clip_path(
    node: &SvgRenderNode,
    clips: &dyn ClipMaskProvider,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> (ClipChainId, Vec<ComplexClip>) {
    let Some(ref effects) = node.style.effects else {
        return (parent_clip_chain, Vec::new());
    };
    let Some(ref clip_path_id) = effects.clip_path else {
        return (parent_clip_chain, Vec::new());
    };
    let Some(clip_def) = clips.clip_path(clip_path_id) else {
        log::warn!("clip-path \"{}\" not found in definitions", clip_path_id);
        return (parent_clip_chain, Vec::new());
    };

    let mut current_chain = parent_clip_chain;
    let mut complex = Vec::new();
    for shape in &clip_def.shapes {
        let Some(geometry) = shape.clip_info(svg_origin, clip_def.clip_path_units) else {
            continue;
        };

        match geometry {
            ClipGeometry::RoundedRect { bounds, radii } => {
                let clip_id = wr.define_clip_rounded_rect(
                    spatial_id,
                    ComplexClipRegion {
                        rect: bounds,
                        radii,
                        mode: ClipMode::Clip,
                    },
                );
                current_chain =
                    wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
            },
            ClipGeometry::Rect { bounds } => {
                let clip_id = wr.define_clip_rect(spatial_id, bounds);
                current_chain =
                    wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
            },
            ClipGeometry::Path {
                bounds,
                path,
                fill_rule,
            } => {
                // WebRender 0.70's quad path panics on image-mask clips, so we
                // can't use `define_clip_image_mask` for arbitrary polygon/path
                // clips. Keep a bounding-rect fallback for native shapes that
                // can't be rasterized (e.g. pattern fills) and defer the real
                // clip to vello rasterization via `ComplexClip`.
                let clip_id = wr.define_clip_rect(spatial_id, bounds);
                current_chain =
                    wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
                complex.push(ComplexClip { path, fill_rule });
            },
        }
    }

    (current_chain, complex)
}

// ======================= Mask Resolution =======================

/// Build individual clip chains for each mask shape, one per shape.
///
/// Returns `None` when no mask is present.
/// Returns `Some(vec![...])` with one [`MaskClip`] per mask shape.
///
/// Each clip chain combines the parent clip AND one mask shape. Rendering the
/// shape once per mask clip achieves union (OR) behavior.
pub(crate) fn build_mask_clips(
    node: &SvgRenderNode,
    clips: &dyn ClipMaskProvider,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
) -> Option<Vec<MaskClip>> {
    let effects = node.style.effects.as_ref()?;
    let mask_id = effects.mask.as_ref()?;
    let mask_def = match clips.mask(mask_id) {
        Some(d) => d,
        None => {
            log::warn!("mask \"{}\" not found in definitions", mask_id);
            return None;
        },
    };

    let mut masks = Vec::with_capacity(mask_def.shapes.len());
    for (shape, _style) in &mask_def.shapes {
        let Some(geometry) = shape.clip_info(svg_origin, ClipPathUnits::UserSpaceOnUse) else {
            continue;
        };

        let (chain, complex) = match geometry {
            ClipGeometry::RoundedRect { bounds, radii } => {
                let clip_id = wr.define_clip_rounded_rect(
                    spatial_id,
                    ComplexClipRegion {
                        rect: bounds,
                        radii,
                        mode: ClipMode::Clip,
                    },
                );
                (
                    wr.define_clip_chain(clip_chain_option(parent_clip_chain), [clip_id]),
                    None,
                )
            },
            ClipGeometry::Rect { bounds } => {
                let clip_id = wr.define_clip_rect(spatial_id, bounds);
                (
                    wr.define_clip_chain(clip_chain_option(parent_clip_chain), [clip_id]),
                    None,
                )
            },
            ClipGeometry::Path {
                bounds,
                path,
                fill_rule,
            } => {
                // Same fallback as clip-path: bounding-rect clip for native
                // shapes, real clip deferred to rasterization.
                let clip_id = wr.define_clip_rect(spatial_id, bounds);
                (
                    wr.define_clip_chain(clip_chain_option(parent_clip_chain), [clip_id]),
                    Some(ComplexClip { path, fill_rule }),
                )
            },
        };

        masks.push(MaskClip { chain, complex });
    }

    if masks.is_empty() { None } else { Some(masks) }
}
