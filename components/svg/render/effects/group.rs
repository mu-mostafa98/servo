/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Group mask compositing.
//!
//! A masked container (`<g mask="…">`, `<use>`, `<symbol>`, …) must composite
//! its whole subtree source-over *first*, then multiply the single result by
//! the mask once. Masking each leaf independently and letting WebRender
//! source-over composite the results double-masks wherever children overlap: a
//! semi-transparent child drawn over another child lets the one underneath
//! bleed through, tinting/darkening the foreground color.
//!
//! This mirrors [`crate::render::effects::mask::rasterize_mask`]'s two-pass
//! composite, but composites *content* (solid/gradient paint at full RGBA)
//! rather than a grayscale mask, then applies the mask to the finished pixmap.

use euclid::Transform2D;
use kurbo::{BezPath, Rect, Shape as _};
use std::hash::{Hash, Hasher};
use vello_cpu::kurbo::Affine;
use vello_cpu::peniko::Fill;
use webrender_api::units::{LayoutPoint, LayoutRect};
use webrender_api::DisplayListBuilder;

use crate::model::document::Defs;
use crate::model::element::{Container, SvgNode, SvgTag};
use crate::model::style::paint::{FillParams, LineCap, LineJoin, StrokeParams};
use crate::model::style::paint_servers::PaintServer;
use crate::model::style::{ColorInterpolation, FillRule};
use crate::render::effects::mask::{translate_gradient, MaskRaster};
use crate::render::geometry::text_to_bez_path;
use crate::render::renderer::color_interpolation_of_style;
use crate::render::renderer::path::{
    apply_paint, line_cap_to_kurbo, line_join_to_kurbo, resolve_fill_paint, resolve_stroke_paint,
    scale_paint, transform_to_affine,
};
use crate::render::renderer::transform::compute_transform_matrix;
use crate::{RasterSink, RasterizedImage};

/// One content leaf of a flattened subtree, ready to composite into a shared
/// pixmap.
pub(crate) struct GroupLeaf {
    bez: BezPath,
    fill: Option<FillParams>,
    stroke: Option<StrokeParams>,
    node_opacity: f32,
    fill_rule: FillRule,
    bbox: Rect,
    color_interpolation: ColorInterpolation,
    /// The leaf's own accumulated transform (its own transform composed with
    /// the masked group's and every ancestor's), mapping leaf-local space into
    /// the same layout space the union bbox and mask raster live in.
    xform: Transform2D<f32, (), ()>,
}

/// The result of flattening a container's subtree into one offscreen composite.
enum GroupComposite {
    /// The subtree is too complex to flatten (nested transforms/viewports/
    /// effects); the caller falls back to per-leaf traversal.
    NotFlat,
    /// The subtree flattens but has nothing to draw.
    Empty,
    /// A finished premultiplied RGBA pixmap covering `x`/`y` (layout space,
    /// relative to the SVG's raster offset) at `w`×`h` device pixels.
    Ready {
        x: f32,
        y: f32,
        w: u16,
        h: u16,
        rgba: Vec<u8>,
    },
}

/// Collect a container's subtree, composite it source-over into one offscreen
/// premultiplied RGBA pixmap, and return the result.
///
/// `leaf_opacity` is threaded into each leaf's own paint (the accumulated
/// opacity of every *enclosing* group — the container's own opacity, if any,
/// is applied afterwards to the finished composite, so it must not be baked
/// into the leaves here). Children composite among themselves at full
/// strength: overlapping children do not over-composite.
#[allow(clippy::too_many_arguments)]
fn composite_group_content(
    group: &SvgNode,
    leaf_opacity: f32,
    raster_offset: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    clip_rect: Option<LayoutRect>,
    defs: Defs<'_>,
) -> GroupComposite {
    // Collect the group's content leaves, accumulating opacity and transform.
    // Nested transforms are composed into each leaf's own transform; nested
    // viewports/effects clear `flat`, so the caller can fall back.
    let mut flat = true;
    let mut leaves: Vec<GroupLeaf> = Vec::new();
    for child in &group.children {
        collect_leaves(child, node_xform, leaf_opacity, defs, &mut flat, &mut leaves);
    }
    if !flat {
        return GroupComposite::NotFlat;
    }
    if leaves.is_empty() {
        return GroupComposite::Empty;
    }

    // First pass: apply each leaf's transform and the viewBox scale to its path
    // and compute the union bounding box (in the same layout space
    // `raster_offset` uses). The per-leaf scale (from the leaf transform's
    // determinant) is used for the stroke inset that expands the layout-space
    // bounding box.
    let Some(mut union) = prepare_leaves(&mut leaves, viewbox_scale) else {
        return GroupComposite::Empty;
    };

    // Intersect with an ancestor sub-viewport clip (when present).
    if let Some(clip) = clip_rect {
        let clip = Rect::new(
            clip.min.x as f64,
            clip.min.y as f64,
            clip.max.x as f64,
            clip.max.y as f64,
        );
        union = Rect::new(
            union.x0.max(clip.x0),
            union.y0.max(clip.y0),
            union.x1.min(clip.x1),
            union.y1.min(clip.y1),
        );
        if union.width() <= 0.0 || union.height() <= 0.0 {
            return GroupComposite::Empty;
        }
    }

    let css_w = (union.width().ceil() as u16).max(1);
    let css_h = (union.height().ceil() as u16).max(1);
    let w = ((css_w as f32 * device_scale).ceil() as u16).max(1);
    let h = ((css_h as f32 * device_scale).ceil() as u16).max(1);

    let rgba = render_leaves(
        &leaves,
        union.x0,
        union.y0,
        w,
        h,
        device_scale,
        viewbox_scale,
        defs,
    );

    GroupComposite::Ready {
        x: raster_offset.x + union.x0 as f32,
        y: raster_offset.y + union.y0 as f32,
        w,
        h,
        rgba,
    }
}

/// Upload and emit a finished group composite as a single image display item.
fn emit_group_composite(
    composite: GroupComposite,
    device_scale: f32,
    wr: &mut DisplayListBuilder,
    sink: &RasterSink,
) {
    let GroupComposite::Ready { x, y, w, h, rgba } = composite else {
        return;
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rgba.hash(&mut hasher);
    let hash = hasher.finish();

    sink.emit(
        wr,
        RasterizedImage {
            x,
            y,
            width: w as u32,
            height: h as u32,
            scale: device_scale,
            data: rgba,
            content_hash: hash,
        },
    );
}

/// Multiply a premultiplied RGBA pixmap by a single opacity factor, scaling RGB
/// by the same factor as alpha so the unpremultiplied color is unchanged.
fn apply_composite_opacity(mut rgba: Vec<u8>, opacity: f32) -> Vec<u8> {
    if opacity >= 1.0 {
        return rgba;
    }
    for px in rgba.chunks_exact_mut(4) {
        for channel in px.iter_mut() {
            *channel = (*channel as f32 * opacity).round() as u8;
        }
    }
    rgba
}

/// Multiply a premultiplied RGBA pixmap by a mask raster, sampling the mask at
/// each composite pixel's layout-space position. Pixels outside the mask region
/// are fully hidden (alpha 0).
fn apply_composite_mask(
    mut rgba: Vec<u8>,
    mask: &MaskRaster,
    composite_x: f32,
    composite_y: f32,
    w: u16,
    h: u16,
    device_scale: f32,
) -> Vec<u8> {
    let mask_w = mask.width as usize;
    for row in 0..h as usize {
        for col in 0..w as usize {
            let idx = (row * w as usize + col) * 4;
            let lx = composite_x + (col as f32 + 0.5) / device_scale;
            let ly = composite_y + (row as f32 + 0.5) / device_scale;
            let mx = (lx - mask.x) * mask.scale;
            let my = (ly - mask.y) * mask.scale;
            let mask_alpha: f32 = if mx < 0.0
                || my < 0.0
                || mx >= mask.width as f32
                || my >= mask.height as f32
            {
                0.0
            } else {
                let mxi = mx as usize;
                let myi = my as usize;
                mask.data[((myi * mask_w + mxi) * 4) + 3] as f32 / 255.0
            };
            // `rgba` is premultiplied, so scaling the mask into the alpha must
            // scale RGB by the same factor to keep the unpremultiplied color
            // unchanged (same as `rasterize_bez`).
            rgba[idx + 0] = (rgba[idx + 0] as f32 * mask_alpha).round() as u8;
            rgba[idx + 1] = (rgba[idx + 1] as f32 * mask_alpha).round() as u8;
            rgba[idx + 2] = (rgba[idx + 2] as f32 * mask_alpha).round() as u8;
            rgba[idx + 3] = (rgba[idx + 3] as f32 * mask_alpha).round() as u8;
        }
    }
    rgba
}

/// Rasterize an opacity-composited container's subtree into one offscreen
/// pixmap, apply the container's own opacity to the finished composite once,
/// and emit it.
///
/// Group opacity is a compositing operation: the children are painted among
/// themselves at full strength (their own opacities only) and the finished
/// group image is then faded as a whole. Folding the group's opacity into each
/// leaf's paint instead lets overlapping children over-composite, tinting the
/// overlap darker than the correct single fade.
///
/// `ancestor_opacity` is the accumulated opacity of every enclosing group
/// (threaded into the leaves); `own_opacity` is this container's own `opacity`,
/// applied to the composite.
///
/// Returns `true` when the group was fully handled (including the empty-group
/// case). Returns `false` when the subtree is too complex to flatten (nested
/// transforms/viewports/effects), signalling the caller to fall back to the
/// per-leaf path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_opacity_group(
    group: &SvgNode,
    ancestor_opacity: f32,
    own_opacity: f32,
    raster_offset: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    clip_rect: Option<LayoutRect>,
    defs: Defs<'_>,
    wr: &mut DisplayListBuilder,
    sink: &RasterSink,
) -> bool {
    let composite = composite_group_content(
        group,
        ancestor_opacity,
        raster_offset,
        node_xform,
        viewbox_scale,
        device_scale,
        clip_rect,
        defs,
    );
    let (x, y, w, h, rgba) = match composite {
        GroupComposite::NotFlat => return false,
        GroupComposite::Empty => return true,
        GroupComposite::Ready { x, y, w, h, rgba } => (x, y, w, h, rgba),
    };
    // Fade the finished composite once, as a whole.
    let rgba = apply_composite_opacity(rgba, own_opacity);
    emit_group_composite(
        GroupComposite::Ready { x, y, w, h, rgba },
        device_scale,
        wr,
        sink,
    );
    true
}

/// Rasterize a masked container's subtree into one offscreen pixmap, mask it
/// once, and emit it.
///
/// Returns `true` when the group was fully handled (including the empty-group
/// case). Returns `false` when the subtree is too complex to flatten (nested
/// transforms/viewports/effects), signalling the caller to fall back to the
/// per-leaf masking path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_masked_group(
    group: &SvgNode,
    mask: &MaskRaster,
    svg_origin: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    effective_opacity: f32,
    clip_rect: Option<LayoutRect>,
    defs: Defs<'_>,
    wr: &mut DisplayListBuilder,
    sink: &RasterSink,
) -> bool {
    let composite = composite_group_content(
        group,
        effective_opacity,
        svg_origin,
        node_xform,
        viewbox_scale,
        device_scale,
        clip_rect,
        defs,
    );
    let (x, y, w, h, rgba) = match composite {
        GroupComposite::NotFlat => return false,
        GroupComposite::Empty => return true,
        GroupComposite::Ready { x, y, w, h, rgba } => (x, y, w, h, rgba),
    };
    // Multiply the whole composite by the mask once. The mask raster lives in
    // the same layout space as `svg_origin`, so each composite pixel is mapped
    // through the shared layout coordinate system into the mask's pixmap.
    let rgba = apply_composite_mask(rgba, mask, x, y, w, h, device_scale);
    emit_group_composite(
        GroupComposite::Ready { x, y, w, h, rgba },
        device_scale,
        wr,
        sink,
    );
    true
}

/// First pass over flattened leaves: bake each leaf's transform and the viewBox
/// scale into its path and compute the union bounding box. Returns `None` when
/// no leaf has an area.
pub(crate) fn prepare_leaves(leaves: &mut [GroupLeaf], viewbox_scale: (f32, f32)) -> Option<Rect> {
    let mut union: Option<Rect> = None;
    for leaf in leaves.iter_mut() {
        let node_scale = (leaf.xform.m11 * leaf.xform.m22 - leaf.xform.m12 * leaf.xform.m21)
            .abs()
            .sqrt()
            .max(1e-4);
        let css_scale = viewbox_scale.0 * node_scale;
        let mut bez_scaled = leaf.bez.clone();
        bez_scaled.apply_affine(transform_to_affine(&leaf.xform));
        bez_scaled.apply_affine(Affine::scale_non_uniform(
            viewbox_scale.0 as f64,
            viewbox_scale.1 as f64,
        ));
        let mut bbox = bez_scaled.bounding_box();
        // Expand for a centered stroke (half the scaled width, plus miter/square
        // overhang), matching `rasterize_bez`.
        if let Some(s) = &leaf.stroke {
            let half_width = s.width.get() as f64 * css_scale as f64 / 2.0;
            let join_inset = match s.line_join {
                LineJoin::Miter | LineJoin::MiterClip => {
                    half_width * (s.miter_limit.max(1.0) as f64)
                },
                _ => half_width,
            };
            let cap_inset = match s.line_cap {
                LineCap::Square => half_width * std::f64::consts::SQRT_2,
                _ => half_width,
            };
            let inset = join_inset.max(cap_inset);
            bbox = Rect::new(
                bbox.x0 - inset,
                bbox.y0 - inset,
                bbox.x1 + inset,
                bbox.y1 + inset,
            );
        }
        if bbox.width() <= 0.0 || bbox.height() <= 0.0 {
            continue;
        }
        leaf.bez = bez_scaled;
        leaf.bbox = bbox;
        union = Some(match union {
            Some(u) => Rect::new(
                u.x0.min(bbox.x0),
                u.y0.min(bbox.y0),
                u.x1.max(bbox.x1),
                u.y1.max(bbox.y1),
            ),
            None => bbox,
        });
    }
    union
}

/// Second pass over prepared leaves: composite every fill and stroke into one
/// premultiplied RGBA pixmap of `w`x`h` device pixels. `origin` is the
/// layout-space position of the pixmap's top-left corner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_leaves(
    leaves: &[GroupLeaf],
    origin_x: f64,
    origin_y: f64,
    w: u16,
    h: u16,
    device_scale: f32,
    viewbox_scale: (f32, f32),
    defs: Defs<'_>,
) -> Vec<u8> {
    let mut context = vello_cpu::RenderContext::new_with(
        w,
        h,
        vello_cpu::RenderSettings {
            num_threads: 0,
            ..Default::default()
        },
    );
    let mut resources = vello_cpu::Resources::new();
    let mut target = vello_cpu::Pixmap::new(w, h);

    // Composite each leaf's fill and stroke into the shared pixmap (in document
    // order, source-over over transparent black).
    for leaf in leaves {
        if leaf.bbox.width() <= 0.0 || leaf.bbox.height() <= 0.0 {
            continue;
        }
        let node_scale = (leaf.xform.m11 * leaf.xform.m22 - leaf.xform.m12 * leaf.xform.m21)
            .abs()
            .sqrt()
            .max(1e-4);
        let stroke_total_scale = viewbox_scale.0 * node_scale * device_scale;
        let mut bez_local = leaf.bez.clone();
        bez_local.apply_affine(Affine::scale_non_uniform(
            device_scale as f64,
            device_scale as f64,
        ));
        bez_local.apply_affine(Affine::translate((
            -origin_x * device_scale as f64,
            -origin_y * device_scale as f64,
        )));

        // Shift a resolved gradient from the leaf's own bbox-local space into
        // pixmap-local space before scaling to device resolution.
        let shift_gradient = |paint: &mut crate::render::renderer::path::ResolvedPaint| {
            if let crate::render::renderer::path::ResolvedPaint::Gradient(g, transform) = paint {
                let dx = leaf.bbox.x0 - origin_x;
                let dy = leaf.bbox.y0 - origin_y;
                translate_gradient(g, dx, dy);
                // A radial paint transform is a scale-about-the-center, so shift
                // its center by conjugating with a translation.
                if let Some(t) = transform {
                    let shift = Affine::translate((dx, dy));
                    *t = shift * *t * Affine::translate((-dx, -dy));
                }
            }
        };

        if let Some(f) = &leaf.fill {
            if let Some(mut paint) = resolve_fill_paint(
                f,
                leaf.bbox.width() as f32,
                leaf.bbox.height() as f32,
                viewbox_scale,
                &leaf.bbox,
                leaf.node_opacity,
                defs,
                leaf.color_interpolation,
            ) {
                shift_gradient(&mut paint);
                context.set_fill_rule(match leaf.fill_rule {
                    FillRule::NonZero => Fill::NonZero,
                    FillRule::EvenOdd => Fill::EvenOdd,
                });
                apply_paint(&mut context, scale_paint(paint, device_scale as f64));
                context.fill_path(&bez_local);
            }
        }

        if let Some(st) = &leaf.stroke {
            if let Some(paint) = resolve_stroke_paint(
                st,
                leaf.bbox.width() as f32,
                leaf.bbox.height() as f32,
                viewbox_scale,
                &leaf.bbox,
                leaf.node_opacity,
                defs,
                leaf.color_interpolation,
            ) {
                apply_paint(&mut context, scale_paint(paint, device_scale as f64));
                let mut vello_stroke = vello_cpu::kurbo::Stroke::new(
                    st.width.get() as f64 * stroke_total_scale as f64,
                )
                .with_caps(line_cap_to_kurbo(st.line_cap))
                .with_join(line_join_to_kurbo(st.line_join))
                .with_miter_limit(st.miter_limit.max(1.0) as f64);
                if let Some(dashes) = &st.dash_array {
                    let pattern: Vec<f64> = dashes
                        .iter()
                        .map(|d| *d as f64 * stroke_total_scale as f64)
                        .collect();
                    vello_stroke = vello_stroke
                        .with_dashes(st.dash_offset as f64 * stroke_total_scale as f64, &pattern);
                }
                context.set_stroke(vello_stroke);
                context.stroke_path(&bez_local);
            }
        }
    }

    context.flush();
    context.render(&mut target, &mut resources);

    target
        .data()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect()
}

/// Recursively collect a masked group's content leaves, accumulating opacity
/// and composing each node's own affine transform into the leaf's transform.
///
/// A node whose viewport, clip-path, mask, filter, markers, or non-rasterizable
/// paint makes it unsafe to flatten clears `flat`, so the caller falls back to
/// the per-leaf traversal.
pub(crate) fn collect_leaves(
    node: &SvgNode,
    accumulated: Transform2D<f32, (), ()>,
    inherited_opacity: f32,
    defs: Defs<'_>,
    flat: &mut bool,
    leaves: &mut Vec<GroupLeaf>,
) {
    if !node.style.is_displayed() || !node.style.is_visible() {
        return;
    }
    if node.viewport.is_some()
        || node.style.clip_path.is_some()
        || node.style.basic_shape_clip.is_some()
        || node.style.mask.is_some()
        || node.style.filter.is_some()
        || node.style.markers.is_some()
    {
        *flat = false;
        return;
    }
    // Compose this node's own transform (left-to-right) onto the accumulated
    // parent transform, matching `render_node`'s `node_xform` accumulation.
    let accumulated = compute_transform_matrix(&node.transforms).then(&accumulated);
    let opacity = inherited_opacity * node.style.opacity.get();
    match &node.tag {
        SvgTag::Shape(shape) => {
            let fill = node.style.fill.clone();
            let stroke = node.style.stroke.clone();
            if fill.is_none() && stroke.is_none() {
                return;
            }
            if !paint_rasterizable(fill.as_ref().and_then(|f| f.paint_server.as_ref()), defs)
                || !paint_rasterizable(stroke.as_ref().and_then(|s| s.paint_server.as_ref()), defs)
            {
                *flat = false;
                return;
            }
            let Some(bez) = shape.to_bez_path() else {
                return;
            };
            let fill_rule = fill
                .as_ref()
                .map(|f| f.fill_rule)
                .unwrap_or(FillRule::NonZero);
            leaves.push(GroupLeaf {
                bez,
                fill,
                stroke,
                node_opacity: opacity,
                fill_rule,
                bbox: Rect::default(),
                color_interpolation: color_interpolation_of_style(&node.style),
                xform: accumulated,
            });
        },
        SvgTag::Text(span) => {
            let fill = node.style.fill.clone();
            let stroke = node.style.stroke.clone();
            if fill.is_none() && stroke.is_none() {
                return;
            }
            if !paint_rasterizable(fill.as_ref().and_then(|f| f.paint_server.as_ref()), defs)
                || !paint_rasterizable(stroke.as_ref().and_then(|s| s.paint_server.as_ref()), defs)
            {
                *flat = false;
                return;
            }
            let Some(bez) = text_to_bez_path(span) else {
                return;
            };
            let fill_rule = fill
                .as_ref()
                .map(|f| f.fill_rule)
                .unwrap_or(FillRule::NonZero);
            leaves.push(GroupLeaf {
                bez,
                fill,
                stroke,
                node_opacity: opacity,
                fill_rule,
                bbox: Rect::default(),
                color_interpolation: color_interpolation_of_style(&node.style),
                xform: accumulated,
            });
        },
        SvgTag::Image(_) => {
            // Images have no CPU-side pixels, so a raster mask cannot be
            // multiplied in here. Fall back to the per-leaf path.
            *flat = false;
        },
        SvgTag::Container(Container::Defs) => {},
        SvgTag::Container(_) => {
            for child in &node.children {
                collect_leaves(child, accumulated, opacity, defs, flat, leaves);
            }
        },
    }
}

/// Whether a paint server resolves to something the vello_cpu path can rasterize
/// (solid or gradient). Patterns, broken references, and `context-fill`/
/// `context-stroke` rasterize as no paint here, so they force the caller to
/// fall back.
fn paint_rasterizable(paint_server: Option<&PaintServer>, defs: Defs<'_>) -> bool {
    match paint_server.and_then(|p| defs.resolve_paint_server(p)) {
        None | Some(PaintServer::Solid(_)) | Some(PaintServer::Gradient(_)) => true,
        Some(PaintServer::Pattern(_))
        | Some(PaintServer::Ref { .. })
        | Some(PaintServer::ContextFill)
        | Some(PaintServer::ContextStroke) => false,
    }
}
