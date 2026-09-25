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
use webrender_api::units::LayoutPoint;

use kurbo::{BezPath, Rect, Shape as _};
use vello_cpu::kurbo::Affine;
use vello_cpu::peniko::{Fill, Gradient, GradientKind};

use crate::model::tree::{MaskContentUnits, MaskDef, MaskType};
use crate::render::renderer::path::{
    apply_paint, resolve_fill_paint, scale_paint, transform_to_affine,
};
use crate::model::style::fill::FillParams;
use crate::model::style::FillRule;

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
/// Only `maskContentUnits="userSpaceOnUse"` masks are rasterized here;
/// `objectBoundingBox` masks keep the legacy geometric-clip path (they need the
/// masked element's bounding box, which is resolved separately).
pub(crate) fn rasterize_mask(
    mask: &MaskDef,
    svg_origin: &LayoutPoint,
    node_xform: Transform2D<f32, (), ()>,
    viewbox_scale: (f32, f32),
    device_scale: f32,
) -> Option<MaskRaster> {
    if mask.content_units != MaskContentUnits::UserSpaceOnUse {
        return None;
    }

    // One fully-owned entry per mask shape (avoids borrowing the mask subtree
    // across the closure below).
    struct MaskShape {
        bez: BezPath,
        fill: FillParams,
        node_opacity: f32,
        fill_rule: FillRule,
        bbox: Rect,
    }

    // First pass: collect shape fills and the union bounding box, applying the
    // node transform and viewBox scale so every shape lands in the same
    // layout space the content rasters use.
    let mut shapes: Vec<MaskShape> = Vec::new();
    let mut union: Option<Rect> = None;
    mask.root.for_each_shape_leaf(&mut |shape, style| {
        if !style.is_visible() {
            return;
        }
        let Some(fill) = style.fill.clone() else {
            return;
        };
        let Some(bez) = shape.to_bez_path() else {
            return;
        };
        let mut bez_scaled = bez.clone();
        bez_scaled.apply_affine(transform_to_affine(&node_xform));
        bez_scaled.apply_affine(Affine::scale_non_uniform(
            viewbox_scale.0 as f64,
            viewbox_scale.1 as f64,
        ));
        let bbox = bez_scaled.bounding_box();
        if bbox.width() <= 0.0 || bbox.height() <= 0.0 {
            return;
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
        let fill_rule = fill.fill_rule;
        shapes.push(MaskShape {
            bez: bez_scaled,
            fill,
            node_opacity: style.opacity.get(),
            fill_rule,
            bbox,
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

    // Second pass: composite each shape's fill into the shared pixmap (in
    // document order, source-over over transparent black).
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

        let Some(mut paint) = resolve_fill_paint(
            &s.fill,
            s.bbox.width() as f32,
            s.bbox.height() as f32,
            viewbox_scale,
            &s.bbox,
            s.node_opacity,
        ) else {
            continue;
        };

        // Gradients are resolved in the shape's own bbox-local space; shift
        // them into union-local space before scaling to device resolution.
        if let crate::render::renderer::path::ResolvedPaint::Gradient(g) = &mut paint {
            translate_gradient(
                g,
                (s.bbox.x0 - union.x0) as f64,
                (s.bbox.y0 - union.y0) as f64,
            );
        }

        context.set_fill_rule(match s.fill_rule {
            FillRule::NonZero => Fill::NonZero,
            FillRule::EvenOdd => Fill::EvenOdd,
        });
        apply_paint(&mut context, scale_paint(paint, device_scale as f64));
        context.fill_path(&bez_local);
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
            MaskType::Luminance => {
                0.2126 * p.r as f32 + 0.7152 * p.g as f32 + 0.0722 * p.b as f32
            },
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
