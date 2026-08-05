/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape emitters — convert usvg types into backend-agnostic paint commands.
//!
//! Each shape implements the [`Emit`] trait, producing [`PaintCommand`]s
//! that are later consumed by a [`crate::renderer::Renderer`].

pub mod image;
pub mod path;
pub mod simple;

use webrender_api::units::LayoutPoint;

/// Backend-agnostic paint command produced by emitters.
#[derive(Debug, Clone)]
pub(crate) enum PaintCommand {
    FillRect {
        bounds: FillRectBounds,
        color: PaintColor,
        clip: Option<RoundedClip>,
    },
    StrokeRect {
        bounds: FillRectBounds,
        color: PaintColor,
        width: f32,
        radii: Option<RoundedRadii>,
    },
    DrawImage {
        x: f32,
        y: f32,
        w: u32,
        h: u32,
        data: Vec<u8>,
        fallback_color: PaintColor,
    },
    StrokeLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: PaintColor,
        width: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FillRectBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaintColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundedClip {
    pub rx: f32,
    pub ry: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundedRadii {
    pub rx: f32,
    pub ry: f32,
}

/// Bundled context passed to every [`Emit::emit`] call.
pub(crate) struct EmitContext {
    pub svg_origin: LayoutPoint,
}

/// Convert an SVG shape into backend-agnostic paint commands.
pub(crate) trait Emit {
    /// Produce paint commands for this shape.
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>);
}

// ======================= Helpers =======================

pub(crate) fn color_from_usvg(c: &usvg::Color, opacity: f32) -> PaintColor {
    PaintColor {
        r: c.red as f32 / 255.0,
        g: c.green as f32 / 255.0,
        b: c.blue as f32 / 255.0,
        a: opacity,
    }
}

// ======================= Gradient Conversion =======================

use vello_cpu::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::kurbo::Point;
use vello_cpu::peniko::{ColorStop, ColorStops, Extend, Gradient};
use vello_cpu::RenderContext;

/// Convert a usvg linear gradient to a peniko [`Gradient`] with coordinates
/// relative to the shape's pixmap (top-left origin at 0,0).
pub(crate) fn convert_linear_gradient(
    gradient: &usvg::LinearGradient,
    bbox: usvg::Rect,
) -> Gradient {
    let (x1, y1) = (gradient.x1() as f64, gradient.y1() as f64);
    let (x2, y2) = (gradient.x2() as f64, gradient.y2() as f64);

    // Compute user-space coordinates.
    let (start, end) = resolve_user_space_coords(
        gradient.units(), gradient.transform(),
        x1, y1, x2, y2, bbox,
    );

    // Convert to pixmap-local space (subtract bbox origin).
    let bx = bbox.x() as f64;
    let by = bbox.y() as f64;
    let local_start = Point::new(start.x as f64 - bx, start.y as f64 - by);
    let local_end = Point::new(end.x as f64 - bx, end.y as f64 - by);

    let stops = convert_stops(gradient.stops());
    let mut g = Gradient::new_linear(local_start, local_end);
    g.extend = convert_spread(gradient.spread_method());
    g.stops = stops;
    g
}

/// Convert a usvg radial gradient to a peniko [`Gradient`] with coordinates
/// relative to the shape's pixmap (top-left origin at 0,0).
///
/// SVG radial gradients on non-square bounding boxes are elliptical.
/// Since peniko only supports circular radial gradients, the caller must
/// apply a [`PaintTransform`] (via `RenderContext::set_paint_transform`) to
/// stretch the circular gradient into an ellipse matching the bbox aspect ratio.
pub(crate) fn convert_radial_gradient(
    gradient: &usvg::RadialGradient,
    bbox: usvg::Rect,
) -> (Gradient, Option<PaintTransform>) {
    let fx = gradient.fx() as f64;
    let fy = gradient.fy() as f64;
    let fr = gradient.fr().get() as f64;
    let cx = gradient.cx() as f64;
    let cy = gradient.cy() as f64;
    let r = gradient.r().get() as f64;

    let bw = bbox.width() as f64;
    let bh = bbox.height() as f64;
    let bx = bbox.x() as f64;
    let by = bbox.y() as f64;

    // Compute pixmap-local center and focal (using actual bbox dimensions).
    let (pixmap_focal, pixmap_center) = resolve_user_space_coords(
        gradient.units(), gradient.transform(),
        fx, fy, cx, cy, bbox,
    );
    let pixmap_center_x = pixmap_center.x as f64 - bx;
    let pixmap_center_y = pixmap_center.y as f64 - by;

    // Determine whether we need aspect-ratio correction.
    let needs_aspect = gradient.units() == usvg::Units::ObjectBoundingBox
        && (bw - bh).abs() > 0.01 && bw > 0.0 && bh > 0.0;

    // Build the paint transform that stretches the circular gradient into an
    // ellipse matching the bbox aspect ratio. The transform scales Y around the
    // gradient center so the gradient touches all four edges of the bbox.
    let paint_transform = if needs_aspect {
        let scale_y = bw / bh;
        Some(PaintTransform {
            center_x: pixmap_center_x,
            center_y: pixmap_center_y,
            scale_y,
        })
    } else {
        None
    };

    // The peniko gradient's center and focal must be in GRADIENT space
    // (after the paint transform is applied). The paint transform T maps:
    //   geometry_point → gradient_point = T(geometry_point)
    // For the center: T(pixmap_center) = pixmap_center (center maps to itself).
    // For the focal: we must apply T to get the gradient-space position.
    let gradient_focal_x = pixmap_focal.x as f64 - bx;
    let gradient_focal_y = if let Some(ref pt) = paint_transform {
        pt.center_y + (pixmap_focal.y as f64 - by - pt.center_y) * pt.scale_y
    } else {
        pixmap_focal.y as f64 - by
    };

    // Radii: for ObjectBoundingBox, use the larger dimension as reference
    // so the circular gradient extends to the farthest edge. The paint
    // transform will stretch it to hit the nearer edge.
    let (fr_gradient, r_gradient) = if gradient.units() == usvg::Units::ObjectBoundingBox {
        let ref_dim = bw.max(bh);
        (fr * ref_dim, r * ref_dim)
    } else {
        (fr, r)
    };

    let stops = convert_stops(gradient.stops());
    let mut g = Gradient::new_two_point_radial(
        Point::new(gradient_focal_x, gradient_focal_y),
        fr_gradient as f32,
        Point::new(pixmap_center_x, pixmap_center_y),
        r_gradient as f32,
    );
    g.extend = convert_spread(gradient.spread_method());
    g.stops = stops;

    (g, paint_transform)
}

/// Paint transform for radial gradients on non-square bounding boxes.
/// Apply via `RenderContext::set_paint_transform` before filling.
pub(crate) struct PaintTransform {
    pub center_x: f64,
    pub center_y: f64,
    pub scale_y: f64,
}

impl PaintTransform {
    /// Apply this paint transform to a [`RenderContext`].
    pub fn apply(&self, context: &mut RenderContext) {
        use vello_cpu::kurbo::Affine;
        let t = Affine::translate((self.center_x, self.center_y))
            * Affine::scale_non_uniform(1.0, self.scale_y)
            * Affine::translate((-self.center_x, -self.center_y));
        context.set_paint_transform(t);
        // Note: caller must call reset_paint_transform() after rendering.
    }
}

/// Compute user-space coordinates for two points, handling
/// ObjectBoundingBox → UserSpaceOnUse resolution and gradient transform.
fn resolve_user_space_coords(
    units: usvg::Units,
    transform: usvg::Transform,
    px1: f64, py1: f64,
    px2: f64, py2: f64,
    bbox: usvg::Rect,
) -> (usvg::tiny_skia_path::Point, usvg::tiny_skia_path::Point) {
    let (ux1, uy1, ux2, uy2) = if units == usvg::Units::ObjectBoundingBox {
        // Map from normalized (0..1) coords to user space via bounding box.
        let bx = bbox.x() as f64;
        let by = bbox.y() as f64;
        let bw = bbox.width() as f64;
        let bh = bbox.height() as f64;
        (bx + px1 * bw, by + py1 * bh, bx + px2 * bw, by + py2 * bh)
    } else {
        (px1, py1, px2, py2)
    };

    // Apply the gradient's own transform.
    let mut p1 = usvg::tiny_skia_path::Point { x: ux1 as f32, y: uy1 as f32 };
    let mut p2 = usvg::tiny_skia_path::Point { x: ux2 as f32, y: uy2 as f32 };
    transform.map_point(&mut p1);
    transform.map_point(&mut p2);

    (p1, p2)
}

/// Convert usvg stops to peniko [`ColorStops`].
fn convert_stops(stops: &[usvg::Stop]) -> ColorStops {
    let items: Vec<ColorStop> = stops.iter().map(|stop| {
        let c = stop.color();
        let a = stop.opacity().get();
        ColorStop {
            offset: stop.offset().get(),
            color: DynamicColor::from_alpha_color(
                AlphaColor::<Srgb>::from_rgba8(c.red, c.green, c.blue, (a * 255.0) as u8),
            ),
        }
    }).collect();
    ColorStops(items.into())
}

/// Convert usvg spread method to peniko [`Extend`].
fn convert_spread(method: usvg::SpreadMethod) -> Extend {
    match method {
        usvg::SpreadMethod::Pad => Extend::Pad,
        usvg::SpreadMethod::Reflect => Extend::Reflect,
        usvg::SpreadMethod::Repeat => Extend::Repeat,
    }
}

/// Returns the first color from a gradient's stops as a fallback [`PaintColor`].
pub(crate) fn gradient_fallback_color(stops: &[usvg::Stop]) -> PaintColor {
    stops.first().map(|s| {
        let c = s.color();
        PaintColor {
            r: c.red as f32 / 255.0,
            g: c.green as f32 / 255.0,
            b: c.blue as f32 / 255.0,
            a: s.opacity().get(),
        }
    }).unwrap_or(PaintColor { r: 0.5, g: 0.5, b: 0.5, a: 1.0 })
}

/// Check if a [`usvg::Paint`] is any gradient variant.
pub(crate) fn is_gradient_paint(paint: &usvg::Paint) -> bool {
    matches!(paint, usvg::Paint::LinearGradient(_) | usvg::Paint::RadialGradient(_))
}

/// Scale RGBA image data in the Y dimension (non-uniform scale).
/// Used for radial gradients on non-square bounding boxes where the
/// circular peniko gradient needs to become elliptical.
pub(crate) fn scale_rgba_y(
    rgba: &[u8], src_w: u32, src_h: u32, target_h: u32,
) -> Vec<u8> {
    if target_h == src_h {
        return rgba.to_vec();
    }
    let src_h = src_h as usize;
    let target_h = target_h as usize;
    let src_w = src_w as usize;
    let mut scaled = Vec::with_capacity(src_w * target_h * 4);
    for y in 0..target_h {
        // Map target Y back to source Y (linear interpolation).
        let src_y = (y as f64 * (src_h - 1) as f64 / (target_h.max(1) - 1) as f64) as usize;
        let src_row_start = (src_y.min(src_h - 1)) * src_w * 4;
        scaled.extend_from_slice(&rgba[src_row_start..src_row_start + src_w * 4]);
    }
    scaled
}
