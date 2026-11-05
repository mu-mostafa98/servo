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

use crate::model::document::{ClipPathDef, ClipPathUnits, Defs, MaskContentUnits};
use crate::model::element::SvgNode;
use crate::model::style::FillRule;
use crate::render::EffectLeaf;
use crate::render::effects::filter::node_local_bounds;
use crate::render::geometry::{
    ClipGeometry, ClipSpace, ComplexClip, basic_shape_clip_geometry, clip_geometry_to_complex_clip,
    clip_path_geometry, text_to_bez_path, union_clip_geometry,
};
use crate::render::renderer::clip_chain_option;

// ======================= Clip Path Resolution =======================

/// Resolve a clip/mask leaf (shape, text run, or image) to clip geometry.
///
/// Shapes go through their native [`ClipGeometry`] path; a text run contributes
/// the fill outline of its shaped glyphs; an image contributes its viewport
/// rectangle.
fn leaf_clip_geometry(
    leaf: EffectLeaf<'_>,
    space: ClipSpace,
    clip_rule: FillRule,
) -> Option<ClipGeometry> {
    match leaf {
        EffectLeaf::Shape(shape) => shape.clip_info(space, clip_rule),
        EffectLeaf::Text(span) => {
            let path = text_to_bez_path(span)?;
            Some(clip_path_geometry(&path, space, clip_rule))
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

/// Emit one resolved clip geometry as a WebRender clip chain item plus (for
/// polygon/path shapes that WebRender can only approximate) a [`ComplexClip`]
/// for the vello rasterization fallback.
fn define_clip_for_geometry(
    geometry: &ClipGeometry,
    spatial_id: SpatialId,
    current_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
    complex: &mut Vec<ComplexClip>,
) -> ClipChainId {
    let chain = match geometry {
        ClipGeometry::RoundedRect { bounds, radii } => {
            let clip_id = wr.define_clip_rounded_rect(
                spatial_id,
                ComplexClipRegion {
                    rect: *bounds,
                    radii: *radii,
                    mode: ClipMode::Clip,
                },
            );
            wr.define_clip_chain(clip_chain_option(current_chain), [clip_id])
        },
        ClipGeometry::Rect { bounds } => {
            let clip_id = wr.define_clip_rect(spatial_id, *bounds);
            wr.define_clip_chain(clip_chain_option(current_chain), [clip_id])
        },
        ClipGeometry::Path { bounds, .. } => {
            // WebRender 0.70's quad path panics on image-mask clips, so we
            // can't use `define_clip_image_mask` for arbitrary polygon/path
            // clips. Keep a bounding-rect fallback for native shapes that
            // can't be rasterized (e.g. pattern fills) and defer the real
            // clip to vello rasterization via `ComplexClip`.
            let clip_id = wr.define_clip_rect(spatial_id, *bounds);
            wr.define_clip_chain(clip_chain_option(current_chain), [clip_id])
        },
    };

    // Every clip geometry is also carried as a `ComplexClip` so CPU-rasterized
    // shapes (paths/polygons/polylines) get the clip too: their native clip
    // chain is only applied to native primitives, and `rasterize_bez` pushes
    // rasters with the *outer* SVG clip chain.
    if let Some(clip) = clip_geometry_to_complex_clip(geometry) {
        complex.push(clip);
    }
    chain
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
    // `url(#…)` clip-path takes precedence over an inline basic shape; a CSS
    // `clip-path` value is one or the other.
    if let Some(clip_def) = node
        .style
        .clip_path
        .as_ref()
        .and_then(|c| c.resolve(&defs.clip_paths))
    {
        // `objectBoundingBox` clip coordinates are fractions of the *clipped
        // element's* bounding box, so resolve that bbox once and scale every
        // clip shape against it (falling back to identity → an empty clip). The
        // bbox is the element that ultimately references the clip, so it is
        // shared across the whole nested `clip-path` chain (a `<clipPath>`
        // establishes no bbox of its own).
        let bbox = Some(node_local_bounds(node));
        let mut complex = Vec::new();
        let current_chain = resolve_clip_path_def(
            clip_def,
            svg_origin,
            spatial_id,
            bbox,
            parent_clip_chain,
            wr,
            defs,
            &mut complex,
            &mut Vec::new(),
        );
        return (current_chain, complex);
    }

    // Inline CSS basic-shape clip (`circle()`, `inset()`, `polygon()`, `path()`).
    if let Some(basic) = &node.style.basic_shape_clip {
        let bbox = Some(node_local_bounds(node));
        let Some(geometry) = basic_shape_clip_geometry(basic, *svg_origin, bbox) else {
            return (parent_clip_chain, Vec::new());
        };
        let mut complex = Vec::new();
        let chain = define_clip_for_geometry(&geometry, spatial_id, parent_clip_chain, wr, &mut complex);
        return (chain, complex);
    }

    (parent_clip_chain, Vec::new())
}

/// Flatten one `<clipPath>` definition — and any `clip-path` it itself carries —
/// into a WebRender clip chain plus the vello [`ComplexClip`]s.
///
/// A `clip-path` on the `<clipPath>` element clips the definition's own
/// content, so its clip entries are emitted before the content's and the two
/// intersect (exactly the nested-clipping semantics). Recursion is bounded by
/// `visited` to keep cyclic references from overflowing the stack.
#[allow(clippy::too_many_arguments)]
fn resolve_clip_path_def(
    clip_def: &ClipPathDef,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    bbox: Option<LayoutRect>,
    mut current_chain: ClipChainId,
    wr: &mut DisplayListBuilder,
    defs: Defs<'_>,
    complex: &mut Vec<ComplexClip>,
    visited: &mut Vec<usize>,
) -> ClipChainId {
    let key = clip_def as *const ClipPathDef as usize;
    if visited.contains(&key) {
        return current_chain;
    }
    visited.push(key);

    // Nested `clip-path` first: it clips this definition's content.
    if let Some(nested) = clip_def
        .clip_path
        .as_ref()
        .and_then(|c| c.resolve(&defs.clip_paths))
    {
        current_chain = resolve_clip_path_def(
            nested,
            svg_origin,
            spatial_id,
            bbox,
            current_chain,
            wr,
            defs,
            complex,
            visited,
        );
    }

    let space = ClipSpace::new(*svg_origin, clip_def.clip_path_units, bbox);
    let mut geometries: Vec<ClipGeometry> = Vec::new();
    clip_def.root.for_each_effect_leaf(&mut |leaf, style| {
        // `clip-rule` is inherited: the `<clipPath>` element's rule is the
        // default, and a leaf's own `evenodd` overrides it. A leaf that
        // explicitly sets `nonzero` is indistinguishable from one that never
        // set it, so it also inherits — which only diverges from the spec in
        // the pathological "explicit nonzero inside an evenodd clipPath" case.
        let rule = if style.clip_rule == FillRule::EvenOdd {
            FillRule::EvenOdd
        } else {
            clip_def.clip_rule
        };
        if let Some(geometry) = leaf_clip_geometry(leaf, space, rule) {
            geometries.push(geometry);
        }
    });

    match geometries.len() {
        0 => {},
        1 => {
            current_chain =
                define_clip_for_geometry(&geometries[0], spatial_id, current_chain, wr, complex);
        },
        _ => {
            // Several shapes in one `<clipPath>` union together (SVG), but
            // WebRender clip chains intersect (AND) — merge the outlines into a
            // single non-zero-fill path that forces vello rasterization.
            if let Some(union) = union_clip_geometry(&geometries) {
                current_chain =
                    define_clip_for_geometry(&union, spatial_id, current_chain, wr, complex);
            }
        },
    }

    visited.pop();
    current_chain
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
        // Masks use luminance/alpha, not the shape's `clip-rule`, so always
        // resolve mask geometry with the default non-zero fill rule.
        let Some(geometry) = leaf_clip_geometry(leaf, space, FillRule::NonZero) else {
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
                    Some(ComplexClip {
                        path,
                        fill_rule,
                        native: false,
                    }),
                )
            },
        };

        masks.push(MaskClip { chain, complex });
    });

    if masks.is_empty() { None } else { Some(masks) }
}
