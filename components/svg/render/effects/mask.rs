/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Mask rasterization — renders `<mask>` content into a grayscale luminance/
//! alpha pixmap that is multiplied into masked shapes during vello_cpu
//! rasterization.
//!
//! WebRender 0.70's quad path panics when a clip chain contains an image-mask
//! clip (`bug: image-masks not expected on rect/quads`), so `<mask>` cannot be
//! expressed as a native clip. Instead the mask content is rasterized here, and
//! the resulting alpha channel is multiplied into the masked content's alpha
//! when it is itself CPU-rasterized.

use euclid::Transform2D;
use kurbo::{BezPath, Rect, Shape as _};
use vello_cpu::kurbo::Affine;
use vello_cpu::peniko::{Fill, Gradient, GradientKind};
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::model::document::{Defs, MaskContentUnits, MaskDef, MaskType, MaskUnits, RegionValue};
use crate::model::element::shape::Shape;
use crate::model::style::{ColorInterpolation, FillRule};
use crate::model::style::paint::{FillParams, LineCap, LineJoin, StrokeParams};
use crate::model::style::paint_servers::PaintServer;
use crate::model::units::Opacity;
use crate::render::EffectLeaf;
use crate::render::renderer::color_interpolation_of_style;
use crate::render::renderer::path::{
    apply_paint, line_cap_to_kurbo, line_join_to_kurbo, resolve_fill_paint, resolve_stroke_paint,
    scale_paint, transform_to_affine,
};
use crate::render::geometry::text_to_bez_path;

/// A CPU-rasterized mask: grayscale pixels whose alpha channel holds the mask
/// value (luminance or alpha of the mask content). Positioned in the same
/// layout space as the content rasters it is multiplied against.
pub(crate) struct MaskRaster {
    /// Layout-space origin (before the document origin is added back).
    pub x: f32,
    pub y: f32,
    /// Size in device pixels.
    pub width: u32,
    pub height: u32,
    /// Device pixel ratio used to rasterize the mask.
    pub scale: f32,
    /// RGBA pixel data; the alpha channel is the mask value (0 = hidden,
    /// 255 = fully visible).
    pub data: Vec<u8>,
}

/// One fully-owned mask-content leaf, collected during the first pass and
/// composited in the second. It lives at module scope so the per-leaf nested
/// mask compositing helpers can reference it.
struct MaskShape {
    bez: BezPath,
    fill: Option<FillParams>,
    stroke: Option<StrokeParams>,
    node_opacity: f32,
    fill_rule: FillRule,
    bbox: Rect,
    color_interpolation: ColorInterpolation,
    /// A nested `<mask>` applied to this leaf (resolved recursively), if any.
    nested_mask: Option<MaskRaster>,
}

/// Maximum nested-mask recursion depth. A cyclic `<mask>` reference produces an
/// empty mask instead of overflowing the stack.
const MAX_MASK_DEPTH: u32 = 16;

/// Rasterize a `<mask>`'s content into a grayscale alpha map.
///
/// Supports both `maskContentUnits`: `userSpaceOnUse` content is used as-is,
/// while `objectBoundingBox` content is scaled/offset by the masked element's
/// bounding box (`element_bbox`, resolved by the caller). A degenerate
/// `objectBoundingBox` bbox yields an empty mask.
pub(crate) fn rasterize_mask(
    mask: &MaskDef,
    svg_origin: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    element_bbox: LayoutRect,
    defs: Defs<'_>,
) -> Option<MaskRaster> {
    rasterize_mask_inner(
        mask,
        svg_origin,
        node_xform,
        viewbox_scale,
        device_scale,
        element_bbox,
        defs,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn rasterize_mask_inner(
    mask: &MaskDef,
    svg_origin: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    element_bbox: LayoutRect,
    defs: Defs<'_>,
    depth: u32,
) -> Option<MaskRaster> {
    if depth > MAX_MASK_DEPTH {
        return None;
    }
    // objectBoundingBox mask coordinates are fractions of the masked element's
    // bbox, mapped into that element's local space before the node transform.
    let (bb_sx, bb_sy, bb_ox, bb_oy) = match mask.content_units {
        MaskContentUnits::ObjectBoundingBox => {
            if element_bbox.size().width <= 0.0 || element_bbox.size().height <= 0.0 {
                return None;
            }
            (
                element_bbox.size().width,
                element_bbox.size().height,
                element_bbox.min.x,
                element_bbox.min.y,
            )
        },
        MaskContentUnits::UserSpaceOnUse => (1.0, 1.0, 0.0, 0.0),
    };

    // Stroke widths live in the mask content's coordinate system, so they are
    // scaled by the objectBoundingBox diagonal (when applicable), the node
    // transform, and the viewBox → viewport scale — the same chain applied to
    // the path geometry, using a uniform scale for the stroke width.
    let bb_uniform = if mask.content_units == MaskContentUnits::ObjectBoundingBox {
        ((bb_sx * bb_sx + bb_sy * bb_sy) / 2.0).sqrt()
    } else {
        1.0
    };
    let node_scale = (node_xform.m11 * node_xform.m22 - node_xform.m12 * node_xform.m21)
        .abs()
        .sqrt()
        .max(1e-4);
    let stroke_css_scale = bb_uniform * viewbox_scale.0 * node_scale;

    // First pass: collect shape fills/strokes and the union bounding box,
    // applying the node transform and viewBox scale so every shape lands in the
    // same layout space the content rasters use.
    let mut shapes: Vec<MaskShape> = Vec::new();
    let mut union: Option<Rect> = None;
    mask.root.for_each_effect_leaf(&mut |leaf, style| {
        if !style.is_visible() {
            return;
        }
        // Resolve the leaf's fill path and paints. Shapes and text use their own
        // fill/stroke; an image contributes its viewport rectangle as opaque
        // white (fully visible), approximating a fully opaque image's mask
        // contribution (raw image pixels are not available on the CPU path).
        let (bez, fill, stroke) = match leaf {
            EffectLeaf::Shape(shape) => {
                let fill = style.fill.clone();
                let stroke = style.stroke.clone();
                if fill.is_none() && stroke.is_none() {
                    return;
                }
                let Some(bez) = shape.to_bez_path() else {
                    return;
                };
                (bez, fill, stroke)
            },
            EffectLeaf::Text(span) => {
                let fill = style.fill.clone();
                let stroke = style.stroke.clone();
                if fill.is_none() && stroke.is_none() {
                    return;
                }
                let Some(bez) = text_to_bez_path(span) else {
                    return;
                };
                (bez, fill, stroke)
            },
            EffectLeaf::Image(img) => {
                if img.width <= 0.0 || img.height <= 0.0 {
                    return;
                }
                let mut bez = kurbo::BezPath::new();
                bez.move_to((img.x as f64, img.y as f64));
                bez.line_to(((img.x + img.width) as f64, img.y as f64));
                bez.line_to(((img.x + img.width) as f64, (img.y + img.height) as f64));
                bez.line_to((img.x as f64, (img.y + img.height) as f64));
                bez.close_path();
                let fill = Some(FillParams {
                    paint_server: Some(PaintServer::Solid(svgtypes::Color::new_rgb(
                        255, 255, 255,
                    ))),
                    opacity: Opacity::new(1.0),
                    fill_rule: FillRule::NonZero,
                });
                (bez, fill, None)
            },
        };
        let mut bez_scaled = bez.clone();
        // objectBoundingBox content: scale/offset into the element's local
        // space first, then apply the node transform and viewBox scale. Every
        // geometry scales per-axis except a `<circle>`: its radius resolves
        // against the bbox diagonal (`bb_uniform`, per the SVG 1.1 note), so
        // rebuild the circle at the correct size rather than letting the
        // per-axis content scale distort it into an ellipse.
        if mask.content_units == MaskContentUnits::ObjectBoundingBox {
            if let EffectLeaf::Shape(Shape::Circle(c)) = leaf {
                bez_scaled = kurbo::Circle::new(
                    (
                        c.cx.get() as f64 * bb_sx as f64 + bb_ox as f64,
                        c.cy.get() as f64 * bb_sy as f64 + bb_oy as f64,
                    ),
                    c.r.get() as f64 * bb_uniform as f64,
                )
                .to_path(0.1);
            } else {
                bez_scaled.apply_affine(Affine::scale_non_uniform(bb_sx as f64, bb_sy as f64));
                bez_scaled.apply_affine(Affine::translate((bb_ox as f64, bb_oy as f64)));
            }
        }
        bez_scaled.apply_affine(transform_to_affine(&node_xform));
        bez_scaled.apply_affine(Affine::scale_non_uniform(
            viewbox_scale.0 as f64,
            viewbox_scale.1 as f64,
        ));
        let mut bbox = bez_scaled.bounding_box();
        if bbox.width() <= 0.0 || bbox.height() <= 0.0 {
            return;
        }
        // Expand the mask bbox for a centered stroke (half the scaled width,
        // plus miter/square overhang), matching `rasterize_bez`.
        if let Some(s) = &stroke {
            let half_width = s.width.get() as f64 * stroke_css_scale as f64 / 2.0;
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
        union = Some(match union {
            Some(u) => Rect::new(
                u.x0.min(bbox.x0),
                u.y0.min(bbox.y0),
                u.x1.max(bbox.x1),
                u.y1.max(bbox.y1),
            ),
            None => bbox,
        });
        let fill_rule = fill
            .as_ref()
            .map(|f| f.fill_rule)
            .unwrap_or(FillRule::NonZero);
        // A nested `<mask>` on this leaf (mask content that is itself masked) is
        // resolved recursively so its contribution is masked before compositing.
        // The leaf's local bbox supplies the nested mask's `objectBoundingBox`
        // reference; the shared node transform/viewBox scale place the nested
        // mask in the same layout space as the leaf geometry.
        let nested_mask = style
            .mask
            .as_ref()
            .and_then(|m| m.resolve(&defs.masks))
            .and_then(|nested| {
                rasterize_mask_inner(
                    nested,
                    svg_origin,
                    node_xform,
                    viewbox_scale,
                    device_scale,
                    effect_leaf_bounds(leaf),
                    defs,
                    depth + 1,
                )
            });
        shapes.push(MaskShape {
            bez: bez_scaled,
            fill,
            stroke,
            node_opacity: style.opacity.get(),
            fill_rule,
            bbox,
            color_interpolation: color_interpolation_of_style(style),
            nested_mask,
        });
    });

    let union = union?;

    // Clip the mask to its region (`x`/`y`/`width`/`height` + `maskUnits`).
    // The region is resolved into the element's local space, transformed into
    // the same layout space as the content union, and intersected with it so
    // mask content outside the region is fully hidden (SVG: the mask region
    // clips the mask content).
    let region_layout = mask_region_rect(mask, element_bbox, &node_xform, viewbox_scale);
    let union = Rect::new(
        union.x0.max(region_layout.x0),
        union.y0.max(region_layout.y0),
        union.x1.min(region_layout.x1),
        union.y1.min(region_layout.y1),
    );
    if union.width() <= 0.0 || union.height() <= 0.0 {
        return None;
    }

    let css_w = (union.width().ceil() as u16).max(1);
    let css_h = (union.height().ceil() as u16).max(1);
    let w = ((css_w as f32 * device_scale).ceil() as u16).max(1);
    let h = ((css_h as f32 * device_scale).ceil() as u16).max(1);

    let stroke_total_scale = stroke_css_scale * device_scale;

    // Composite the mask content into a premultiplied RGBA buffer.
    //
    // Fast path (no nested masks): draw every shape into one vello context so
    // vello source-over's them in document order, then rasterize once.
    //
    // Nested-mask path: each shape is rasterized into its own pixmap, its nested
    // mask is multiplied in per-pixel, and the result is source-over'd into the
    // union buffer. A nested mask is a per-pixel alpha map, which vello cannot
    // apply as a clip, so it must be baked in after rasterization.
    let has_nested = shapes.iter().any(|s| s.nested_mask.is_some());

    let rgba: Vec<u8> = if !has_nested {
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

        for s in &shapes {
            let mut bez_local = s.bez.clone();
            bez_local.apply_affine(Affine::scale_non_uniform(
                device_scale as f64,
                device_scale as f64,
            ));
            bez_local.apply_affine(Affine::translate((
                -union.x0 * device_scale as f64,
                -union.y0 * device_scale as f64,
            )));

            // Shift a resolved gradient from the shape's own bbox-local space into
            // union-local space before scaling to device resolution.
            let shift_gradient = |paint: &mut crate::render::renderer::path::ResolvedPaint| {
                if let crate::render::renderer::path::ResolvedPaint::Gradient(g, transform) = paint {
                    let dx = (s.bbox.x0 - union.x0) as f64;
                    let dy = (s.bbox.y0 - union.y0) as f64;
                    translate_gradient(g, dx, dy);
                    // A radial paint transform is a scale-about-the-center, so shift
                    // its center by conjugating with a translation.
                    if let Some(t) = transform {
                        let shift = vello_cpu::kurbo::Affine::translate((dx, dy));
                        *t = shift * *t * vello_cpu::kurbo::Affine::translate((-dx, -dy));
                    }
                }
            };

            if let Some(f) = &s.fill {
                if let Some(mut paint) = resolve_fill_paint(
                    f,
                    s.bbox.width() as f32,
                    s.bbox.height() as f32,
                    viewbox_scale,
                    &s.bbox,
                    s.node_opacity,
                    defs,
                    s.color_interpolation,
                ) {
                    shift_gradient(&mut paint);
                    context.set_fill_rule(match s.fill_rule {
                        FillRule::NonZero => Fill::NonZero,
                        FillRule::EvenOdd => Fill::EvenOdd,
                    });
                    apply_paint(&mut context, scale_paint(paint, device_scale as f64));
                    context.fill_path(&bez_local);
                }
            }

            if let Some(st) = &s.stroke {
                if let Some(paint) = resolve_stroke_paint(
                    st,
                    s.bbox.width() as f32,
                    s.bbox.height() as f32,
                    viewbox_scale,
                    &s.bbox,
                    s.node_opacity,
                    defs,
                    s.color_interpolation,
                ) {
                    apply_paint(&mut context, scale_paint(paint, device_scale as f64));
                    let mut vello_stroke =
                        vello_cpu::kurbo::Stroke::new(st.width.get() as f64 * stroke_total_scale as f64)
                            .with_caps(line_cap_to_kurbo(st.line_cap))
                            .with_join(line_join_to_kurbo(st.line_join))
                            .with_miter_limit(st.miter_limit.max(1.0) as f64);
                    // Dash lengths/offset are in mask-content units, scaled by the
                    // same factor as the stroke width.
                    if let Some(dashes) = &st.dash_array {
                        let pattern: Vec<f64> = dashes
                            .iter()
                            .map(|d| *d as f64 * stroke_total_scale as f64)
                            .collect();
                        vello_stroke =
                            vello_stroke.with_dashes(st.dash_offset as f64 * stroke_total_scale as f64, &pattern);
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
    } else {
        let mut union_data = vec![0u8; (w as usize) * (h as usize) * 4];
        for s in &shapes {
            let (leaf_data, lw, lh) = rasterize_mask_shape(
                s,
                stroke_total_scale,
                viewbox_scale,
                device_scale,
                defs,
            );

            let leaf_data = if let Some(nested) = &s.nested_mask {
                apply_nested_mask(&leaf_data, lw, lh, s, nested, svg_origin, device_scale)
            } else {
                leaf_data
            };

            let dst_x = ((s.bbox.x0 - union.x0) * device_scale as f64).round() as i32;
            let dst_y = ((s.bbox.y0 - union.y0) * device_scale as f64).round() as i32;
            composite_source_over(
                &mut union_data,
                w as u32,
                h as u32,
                dst_x,
                dst_y,
                &leaf_data,
                lw as u32,
                lh as u32,
            );
        }
        union_data
    };

    // Convert the composited premultiplied RGBA to a grayscale alpha map.
    //
    // A luminance mask uses the Rec. 709 *luma* of the mask content's color,
    // applied to the gamma-encoded sRGB channels (NOT linearized). This is what
    // Chrome/Skia does (`SkLumaColorFilter`: `{0,0,0, a * luma(r,g,b)}` with
    // luma as a dot product of gamma-encoded channels), and it matches
    // feColorMatrix `luminanceToAlpha`. Because the stored channels are
    // premultiplied, `0.2126*r + 0.7152*g + 0.0722*b` already equals
    // `alpha × luma(unpremultiplied color)`, so the content's own transparency
    // is honored without an explicit unpremultiply. An alpha mask uses the
    // composited alpha directly.
    let data = grayscale_data(&rgba, mask.mask_type);

    Some(MaskRaster {
        x: svg_origin.x + union.x0 as f32,
        y: svg_origin.y + union.y0 as f32,
        width: w as u32,
        height: h as u32,
        scale: device_scale,
        data,
    })
}

/// The local bounding box of a mask-content leaf, mirroring
/// [`crate::render::effects::filter::node_local_bounds`] for an [`EffectLeaf`].
/// Used as the `objectBoundingBox` reference when resolving a nested mask.
fn effect_leaf_bounds(leaf: EffectLeaf<'_>) -> LayoutRect {
    match leaf {
        EffectLeaf::Shape(shape) => shape.local_bounds(),
        EffectLeaf::Text(span) => {
            let fs = if span.font_size > 0.0 {
                span.font_size
            } else {
                16.0
            };
            LayoutRect::from_origin_and_size(
                LayoutPoint::new(span.origin_x(), span.origin_y() - fs),
                LayoutSize::new(span.total_advance().max(1.0), fs * 1.25),
            )
        },
        EffectLeaf::Image(img) => LayoutRect::from_origin_and_size(
            LayoutPoint::new(img.x, img.y),
            LayoutSize::new(img.width, img.height),
        ),
    }
}

/// Rasterize a single mask-content leaf into its own premultiplied-RGBA pixmap,
/// returning `(rgba_bytes, width, height)`.
///
/// The path is translated into the leaf's own bbox-local space, so the resolved
/// gradient (already bbox-local) needs no further shift — only the device scale.
fn rasterize_mask_shape(
    s: &MaskShape,
    stroke_total_scale: f32,
    viewbox_scale: (f32, f32),
    device_scale: f32,
    defs: Defs<'_>,
) -> (Vec<u8>, u16, u16) {
    let css_w = (s.bbox.width().ceil() as u16).max(1);
    let css_h = (s.bbox.height().ceil() as u16).max(1);
    let w = ((css_w as f32 * device_scale).ceil() as u16).max(1);
    let h = ((css_h as f32 * device_scale).ceil() as u16).max(1);

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

    let mut bez_local = s.bez.clone();
    bez_local.apply_affine(Affine::scale_non_uniform(
        device_scale as f64,
        device_scale as f64,
    ));
    bez_local.apply_affine(Affine::translate((
        -s.bbox.x0 * device_scale as f64,
        -s.bbox.y0 * device_scale as f64,
    )));

    if let Some(f) = &s.fill {
        if let Some(paint) = resolve_fill_paint(
            f,
            s.bbox.width() as f32,
            s.bbox.height() as f32,
            viewbox_scale,
            &s.bbox,
            s.node_opacity,
            defs,
            s.color_interpolation,
        ) {
            context.set_fill_rule(match s.fill_rule {
                FillRule::NonZero => Fill::NonZero,
                FillRule::EvenOdd => Fill::EvenOdd,
            });
            apply_paint(&mut context, scale_paint(paint, device_scale as f64));
            context.fill_path(&bez_local);
        }
    }

    if let Some(st) = &s.stroke {
        if let Some(paint) = resolve_stroke_paint(
            st,
            s.bbox.width() as f32,
            s.bbox.height() as f32,
            viewbox_scale,
            &s.bbox,
            s.node_opacity,
            defs,
            s.color_interpolation,
        ) {
            apply_paint(&mut context, scale_paint(paint, device_scale as f64));
            let mut vello_stroke =
                vello_cpu::kurbo::Stroke::new(st.width.get() as f64 * stroke_total_scale as f64)
                    .with_caps(line_cap_to_kurbo(st.line_cap))
                    .with_join(line_join_to_kurbo(st.line_join))
                    .with_miter_limit(st.miter_limit.max(1.0) as f64);
            if let Some(dashes) = &st.dash_array {
                let pattern: Vec<f64> = dashes
                    .iter()
                    .map(|d| *d as f64 * stroke_total_scale as f64)
                    .collect();
                vello_stroke =
                    vello_stroke.with_dashes(st.dash_offset as f64 * stroke_total_scale as f64, &pattern);
            }
            context.set_stroke(vello_stroke);
            context.stroke_path(&bez_local);
        }
    }

    context.flush();
    context.render(&mut target, &mut resources);

    let rgba = target
        .data()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();
    (rgba, w, h)
}

/// Multiply a leaf's premultiplied RGBA by a nested mask, sampled in the same
/// layout space the mask raster lives in. Pixels outside the nested mask region
/// are fully hidden.
fn apply_nested_mask(
    leaf: &[u8],
    lw: u16,
    lh: u16,
    s: &MaskShape,
    nested: &MaskRaster,
    svg_origin: &LayoutPoint,
    device_scale: f32,
) -> Vec<u8> {
    let mut out = leaf.to_vec();
    let nested_w = nested.width as usize;
    for row in 0..lh as usize {
        for col in 0..lw as usize {
            let idx = (row * lw as usize + col) * 4;
            let lx = svg_origin.x + s.bbox.x0 as f32 + (col as f32 + 0.5) / device_scale;
            let ly = svg_origin.y + s.bbox.y0 as f32 + (row as f32 + 0.5) / device_scale;
            let mx = (lx - nested.x) * nested.scale;
            let my = (ly - nested.y) * nested.scale;
            let alpha: f32 = if mx < 0.0
                || my < 0.0
                || mx >= nested.width as f32
                || my >= nested.height as f32
            {
                0.0
            } else {
                let mxi = mx as usize;
                let myi = my as usize;
                nested.data[((myi * nested_w + mxi) * 4) + 3] as f32 / 255.0
            };
            // Premultiplied RGBA: scale RGB by the same factor as alpha so the
            // unpremultiplied color stays unchanged.
            out[idx + 0] = (out[idx + 0] as f32 * alpha).round() as u8;
            out[idx + 1] = (out[idx + 1] as f32 * alpha).round() as u8;
            out[idx + 2] = (out[idx + 2] as f32 * alpha).round() as u8;
            out[idx + 3] = (out[idx + 3] as f32 * alpha).round() as u8;
        }
    }
    out
}

/// Source-over composite a premultiplied RGBA pixmap into a destination buffer
/// at `(dst_x, dst_y)`, clipping to the destination bounds.
fn composite_source_over(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    dst_x: i32,
    dst_y: i32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
) {
    for sy in 0..src_h as i32 {
        let dy = dst_y + sy;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for sx in 0..src_w as i32 {
            let dx = dst_x + sx;
            if dx < 0 || dx >= dst_w as i32 {
                continue;
            }
            let si = ((sy as u32 * src_w + sx as u32) * 4) as usize;
            let di = ((dy as u32 * dst_w + dx as u32) * 4) as usize;
            let sa = src[si + 3] as f32 / 255.0;
            let da = dst[di + 3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                dst[di..di + 4].copy_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            for c in 0..3 {
                let s = src[si + c] as f32;
                let d = dst[di + c] as f32;
                dst[di + c] = (s + d * (1.0 - sa)).round().clamp(0.0, 255.0) as u8;
            }
            dst[di + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Convert a premultiplied RGBA buffer into a grayscale alpha map (each output
/// pixel is `[255, 255, 255, v]`, where `v` is the mask value).
fn grayscale_data(rgba: &[u8], mask_type: MaskType) -> Vec<u8> {
    let mut data = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        let value = match mask_type {
            MaskType::Luminance => {
                0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32
            },
            MaskType::Alpha => px[3] as f32,
        };
        let v = value.round().clamp(0.0, 255.0) as u8;
        data.extend_from_slice(&[255, 255, 255, v]);
    }
    data
}

/// Resolve the `<mask>` region (`x`/`y`/`width`/`height`) into a layout-space
/// rect, mirroring [`crate::render::effects::filter::filter_region_rect`].
///
/// A `<percentage>` is always a fraction of the referencing element's bounding
/// box. A bare `<number>` consults `maskUnits`: a bounding-box fraction under
/// `objectBoundingBox` (the default), or a user-unit coordinate under
/// `userSpaceOnUse`. The resolved local-space region is then transformed by the
/// node transform and viewBox scale into the same layout space the mask content
/// is rasterized in (an axis-aligned bounding box of the transformed corners,
/// so rotation/skew degrade gracefully).
fn mask_region_rect(
    mask: &MaskDef,
    element_bbox: LayoutRect,
    node_xform: &Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
) -> Rect {
    let start = |value: RegionValue, bbox_min: f32, bbox_size: f32| -> f32 {
        match value {
            RegionValue::Percent(p) => bbox_min + p * bbox_size,
            RegionValue::Number(n) => match mask.mask_units {
                MaskUnits::UserSpaceOnUse => n,
                MaskUnits::ObjectBoundingBox => bbox_min + n * bbox_size,
            },
        }
    };
    let size = |value: RegionValue, bbox_size: f32| -> f32 {
        match value {
            RegionValue::Percent(p) => p * bbox_size,
            RegionValue::Number(n) => match mask.mask_units {
                MaskUnits::UserSpaceOnUse => n,
                MaskUnits::ObjectBoundingBox => n * bbox_size,
            },
        }
    };
    let rx = start(mask.x, element_bbox.min.x, element_bbox.size().width);
    let ry = start(mask.y, element_bbox.min.y, element_bbox.size().height);
    let rw = size(mask.width, element_bbox.size().width);
    let rh = size(mask.height, element_bbox.size().height);

    let xform = transform_to_affine(node_xform);
    let vb = Affine::scale_non_uniform(viewbox_scale.0 as f64, viewbox_scale.1 as f64);
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (cx, cy) in [(rx, ry), (rx + rw, ry), (rx + rw, ry + rh), (rx, ry + rh)] {
        let p = vb * (xform * kurbo::Point::new(cx as f64, cy as f64));
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Rect::new(min_x, min_y, max_x, max_y)
}

/// Shift a peniko gradient's geometry by `(dx, dy)`.
pub(crate) fn translate_gradient(g: &mut Gradient, dx: f64, dy: f64) {
    match &mut g.kind {
        GradientKind::Linear(pos) => {
            pos.start.x += dx;
            pos.start.y += dy;
            pos.end.x += dx;
            pos.end.y += dy;
        },
        GradientKind::Radial(pos) => {
            pos.start_center.x += dx;
            pos.start_center.y += dy;
            pos.end_center.x += dx;
            pos.end_center.y += dy;
        },
        GradientKind::Sweep(_) => {},
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::element::{Container, SvgNode, SvgTag};
    use crate::model::style::NodeStyle;

    /// Build a minimal [`MaskDef`] with an empty container root, for testing
    /// region resolution (which reads only the region fields, not the content).
    fn empty_mask(
        mask_units: MaskUnits,
        x: RegionValue,
        y: RegionValue,
        width: RegionValue,
        height: RegionValue,
    ) -> MaskDef {
        MaskDef {
            root: SvgNode {
                id: None,
                tag: SvgTag::Container(Container::Group),
                style: NodeStyle::default(),
                transforms: vec![],
                viewport: None,
                children: vec![],
                source_node: None,
            },
            mask_type: MaskType::Luminance,
            content_units: MaskContentUnits::UserSpaceOnUse,
            x,
            y,
            width,
            height,
            mask_units,
        }
    }

    fn assert_rect_approx(got: Rect, want: (f64, f64, f64, f64)) {
        assert!(
            (got.x0 - want.0).abs() < 1e-3
                && (got.y0 - want.1).abs() < 1e-3
                && (got.x1 - want.2).abs() < 1e-3
                && (got.y1 - want.3).abs() < 1e-3,
            "rect {:?} != ({}, {}, {}, {})",
            got,
            want.0,
            want.1,
            want.2,
            want.3,
        );
    }

    #[test]
    fn grayscale_luminance_white_visible_black_hidden() {
        // Opaque white mask content → fully visible (255).
        assert_eq!(
            grayscale_data(&[255, 255, 255, 255], MaskType::Luminance),
            vec![255, 255, 255, 255]
        );
        // Opaque black mask content → fully hidden (0).
        assert_eq!(
            grayscale_data(&[0, 0, 0, 255], MaskType::Luminance),
            vec![255, 255, 255, 0]
        );
    }

    #[test]
    fn grayscale_alpha_uses_alpha_channel_only() {
        // An alpha mask ignores the color channels entirely.
        assert_eq!(
            grayscale_data(&[10, 20, 30, 128], MaskType::Alpha),
            vec![255, 255, 255, 128]
        );
    }

    #[test]
    fn grayscale_luminance_premultiplied_channel() {
        // Red at 50% opacity premultiplies to [128, 0, 0, 128]; the luminance of
        // the premultiplied channels equals alpha × luma(unpremultiplied red).
        assert_eq!(
            grayscale_data(&[128, 0, 0, 128], MaskType::Luminance),
            vec![255, 255, 255, 27]
        );
    }

    #[test]
    fn composite_source_over_opaque_over_transparent() {
        let mut dst = [0u8; 4];
        composite_source_over(&mut dst, 1, 1, 0, 0, &[255, 0, 0, 255], 1, 1);
        assert_eq!(dst, [255, 0, 0, 255]);
    }

    #[test]
    fn composite_source_over_semitransparent_over_opaque() {
        let mut dst = [0u8, 0, 255, 255]; // opaque blue
        composite_source_over(&mut dst, 1, 1, 0, 0, &[128, 0, 0, 128], 1, 1);
        assert_eq!(dst, [128, 0, 127, 255]);
    }

    #[test]
    fn composite_source_over_clips_out_of_bounds() {
        let mut dst = [0u8; 4];
        composite_source_over(&mut dst, 1, 1, 5, 5, &[255, 255, 255, 255], 1, 1);
        assert_eq!(dst, [0, 0, 0, 0]);
    }

    #[test]
    fn mask_region_user_space_on_use_absolute_numbers() {
        // Under `userSpaceOnUse`, a bare `<number>` region is absolute user
        // units, independent of the referencing element's bbox.
        let mask = empty_mask(
            MaskUnits::UserSpaceOnUse,
            RegionValue::Number(0.0),
            RegionValue::Number(0.0),
            RegionValue::Number(100.0),
            RegionValue::Number(100.0),
        );
        let bbox = LayoutRect::from_origin_and_size(
            LayoutPoint::new(10.0, 20.0),
            LayoutSize::new(50.0, 60.0),
        );
        let got = mask_region_rect(&mask, bbox, &Transform2D::identity(), (1.0, 1.0));
        assert_rect_approx(got, (0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn mask_region_object_bounding_box_default_region() {
        // The default mask region (-10%, -10%, 120%, 120%) is a fraction of the
        // referencing element's bbox.
        let mask = empty_mask(
            MaskUnits::ObjectBoundingBox,
            RegionValue::Percent(-0.1),
            RegionValue::Percent(-0.1),
            RegionValue::Percent(1.2),
            RegionValue::Percent(1.2),
        );
        let bbox = LayoutRect::from_origin_and_size(
            LayoutPoint::new(10.0, 20.0),
            LayoutSize::new(100.0, 100.0),
        );
        let got = mask_region_rect(&mask, bbox, &Transform2D::identity(), (1.0, 1.0));
        assert_rect_approx(got, (0.0, 10.0, 120.0, 130.0));
    }

    #[test]
    fn mask_region_object_bounding_box_number_is_fraction() {
        // Under `objectBoundingBox`, a bare `<number>` is also a fraction of the
        // bbox (e.g. 0.5 → half the width/height).
        let mask = empty_mask(
            MaskUnits::ObjectBoundingBox,
            RegionValue::Number(0.0),
            RegionValue::Number(0.0),
            RegionValue::Number(0.5),
            RegionValue::Number(0.5),
        );
        let bbox = LayoutRect::from_origin_and_size(
            LayoutPoint::new(10.0, 20.0),
            LayoutSize::new(100.0, 100.0),
        );
        let got = mask_region_rect(&mask, bbox, &Transform2D::identity(), (1.0, 1.0));
        assert_rect_approx(got, (10.0, 20.0, 60.0, 70.0));
    }
}
