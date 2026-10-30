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

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ClipChainId, ClipMode, ComplexClipRegion, DisplayListBuilder, SpatialId};

use crate::model::document::{ClipPathUnits, Defs, MaskContentUnits};
use crate::model::element::SvgNode;
use crate::render::EffectLeaf;
use crate::render::effects::filter::node_local_bounds;
use crate::render::geometry::{
    ClipGeometry, ClipSpace, ComplexClip, clip_path_geometry, text_to_bez_path,
};
use crate::render::renderer::clip_chain_option;

// ======================= Clip Path Resolution =======================

/// Resolve a clip/mask leaf (shape, text run, or image) to clip geometry.
///
/// Shapes go through their native [`ClipGeometry`] path; a text run contributes
/// the fill outline of its shaped glyphs; an image contributes its viewport
/// rectangle.
fn leaf_clip_geometry(leaf: EffectLeaf<'_>, space: ClipSpace) -> Option<ClipGeometry> {
    match leaf {
        EffectLeaf::Shape(shape) => shape.clip_info(space),
        EffectLeaf::Text(span) => {
            let path = text_to_bez_path(span)?;
            Some(clip_path_geometry(&path, space))
        },
        EffectLeaf::Image(img) => {
            if img.width <= 0.0 || img.height <= 0.0 {
                return None;
            }
            let bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(
                    img.x * space.sx + space.ox,
                    img.y * space.sy + space.oy,
                ),
                LayoutSize::new(img.width * space.sx, img.height * space.sy),
            );
            Some(ClipGeometry::Rect { bounds })
        },
    }
}

/// A single mask shape resolved to either a WebRender clip chain (rect /
/// rounded-rect) or a [`ComplexClip`] applied during rasterization.
#[derive(Clone)]
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
    node: &SvgNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
    defs: Defs<'_>,
) -> (ClipChainId, Vec<ComplexClip>) {
    let Some(clip_def) = node
        .style
        .clip_path
        .as_ref()
        .and_then(|c| c.resolve(&defs.clip_paths))
    else {
        return (parent_clip_chain, Vec::new());
    };

    // `objectBoundingBox` clip coordinates are fractions of the *clipped
    // element's* bounding box, so resolve that bbox once and scale every clip
    // shape against it (falling back to identity → an empty clip).
    let space = ClipSpace::new(
        *svg_origin,
        clip_def.clip_path_units,
        Some(node_local_bounds(node)),
    );

    let mut current_chain = parent_clip_chain;
    let mut complex = Vec::new();
    clip_def.root.for_each_effect_leaf(&mut |leaf, _style| {
        let Some(geometry) = leaf_clip_geometry(leaf, space) else {
            return;
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
                current_chain = wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
            },
            ClipGeometry::Rect { bounds } => {
                let clip_id = wr.define_clip_rect(spatial_id, bounds);
                current_chain = wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
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
                current_chain = wr.define_clip_chain(clip_chain_option(current_chain), [clip_id]);
                complex.push(ComplexClip { path, fill_rule });
            },
        }
    });

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
    node: &SvgNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    parent_clip_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
    defs: Defs<'_>,
) -> Option<Vec<MaskClip>> {
    let mask_def = node
        .style
        .mask
        .as_ref()
        .and_then(|m| m.resolve(&defs.masks))?;

    // Honour `maskContentUnits` for the geometric-clip fallback (reached only
    // when mask content cannot be CPU-rasterized).
    let units = match mask_def.content_units {
        MaskContentUnits::ObjectBoundingBox => ClipPathUnits::ObjectBoundingBox,
        MaskContentUnits::UserSpaceOnUse => ClipPathUnits::UserSpaceOnUse,
    };
    let space = ClipSpace::new(*svg_origin, units, Some(node_local_bounds(node)));

    let mut masks = Vec::new();
    mask_def.root.for_each_effect_leaf(&mut |leaf, _style| {
        let Some(geometry) = leaf_clip_geometry(leaf, space) else {
            return;
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
    });

    if masks.is_empty() { None } else { Some(masks) }
}
