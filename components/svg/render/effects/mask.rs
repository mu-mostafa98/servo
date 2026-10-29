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
use webrender_api::units::{LayoutPoint, LayoutRect};

use crate::model::document::{Defs, MaskContentUnits, MaskDef, MaskType};
use crate::model::style::{ColorInterpolation, FillRule};
use crate::model::style::paint::{FillParams, LineCap, LineJoin, StrokeParams};
use crate::render::renderer::color_interpolation_of_style;
use crate::render::renderer::path::{
    apply_paint, line_cap_to_kurbo, line_join_to_kurbo, resolve_fill_paint, resolve_stroke_paint,
    scale_paint, transform_to_affine,
};

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

    // One fully-owned entry per mask shape (avoids borrowing the mask subtree
    // across the closure below).
    struct MaskShape {
        bez: BezPath,
        fill: Option<FillParams>,
        stroke: Option<StrokeParams>,
        node_opacity: f32,
        fill_rule: FillRule,
        bbox: Rect,
        color_interpolation: ColorInterpolation,
    }

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
    mask.root.for_each_shape_leaf(&mut |shape, style| {
        if !style.is_visible() {
            return;
        }
        let fill = style.fill.clone();
        let stroke = style.stroke.clone();
        if fill.is_none() && stroke.is_none() {
            return;
        }
        let Some(bez) = shape.to_bez_path() else {
            return;
        };
        let mut bez_scaled = bez.clone();
        // objectBoundingBox content: scale/offset into the element's local
        // space first, then apply the node transform and viewBox scale.
        bez_scaled.apply_affine(Affine::scale_non_uniform(bb_sx as f64, bb_sy as f64));
        bez_scaled.apply_affine(Affine::translate((bb_ox as f64, bb_oy as f64)));
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
        shapes.push(MaskShape {
            bez: bez_scaled,
            fill,
            stroke,
            node_opacity: style.opacity.get(),
            fill_rule,
            bbox,
            color_interpolation: color_interpolation_of_style(style),
        });
    });

    let union = union?;

    let css_w = (union.width().ceil() as u16).max(1);
    let css_h = (union.height().ceil() as u16).max(1);
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

    // Second pass: composite each shape's fill and stroke into the shared
    // pixmap (in document order, source-over over transparent black).
    let stroke_total_scale = stroke_css_scale * device_scale;
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

    // Convert the composited premultiplied RGBA to a grayscale alpha map.
    // For luminance masks the mask value is the luminance of the premultiplied
    // color (== alpha × luminance of the unpremultiplied color); for alpha
    // masks it is the composited alpha directly.
    let mut data = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for p in target.data().iter() {
        let value = match mask.mask_type {
            MaskType::Luminance => 0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32,
            MaskType::Alpha => p.a as f32,
        };
        let v = value.round().clamp(0.0, 255.0) as u8;
        data.extend_from_slice(&[255, 255, 255, v]);
    }

    Some(MaskRaster {
        x: svg_origin.x + union.x0 as f32,
        y: svg_origin.y + union.y0 as f32,
        width: w as u32,
        height: h as u32,
        scale: device_scale,
        data,
    })
}

/// Shift a peniko gradient's geometry by `(dx, dy)`.
fn translate_gradient(g: &mut Gradient, dx: f64, dy: f64) {
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
