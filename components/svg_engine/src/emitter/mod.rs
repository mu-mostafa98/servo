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
pub mod text;

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
    /// Render text glyphs directly via the backend's native text API.
    /// Used for simple text (solid fill, no textPath/rotate/dx/dy).
    Text {
        x: f32,
        y: f32,
        glyphs: Vec<TextGlyph>,
        /// Opaque handle into the `FontKeyRegistry` — the WebRender backend
        /// resolves this to a `FontInstanceKey` for `push_text`.
        font_handle: usize,
        font_size: f32,
        color: PaintColor,
    },
}

/// A positioned glyph for [`PaintCommand::Text`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextGlyph {
    /// Glyph ID within the font.
    pub glyph_id: u32,
    /// X position relative to the text origin.
    pub x: f32,
    /// Y position relative to the text origin.
    pub y: f32,
    /// Horizontal advance width to the next glyph.
    pub advance: f32,
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
pub(crate) struct EmitContext<'a> {
    pub svg_origin: LayoutPoint,
    /// Pre-shaped glyphs keyed by the same handle stored on usvg text spans.
    /// Read by the text emitter for simple text.
    pub glyphs: &'a crate::GlyphStore,
    /// Handle → WebRender `FontInstanceKey` map, read by the WebRender
    /// backend's `draw_text`. Read here only to pass the handle through
    /// `PaintCommand::Text`; the backend does the actual lookup.
    pub font_keys: &'a crate::FontKeyRegistry,
    /// Accumulated group opacity (1.0 = fully opaque).
    /// Multiplied by parent group opacities when descending into child groups.
    pub group_opacity: f32,
}

/// Convert an SVG shape into backend-agnostic paint commands.
pub(crate) trait Emit {
    /// Produce paint commands for this shape.
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>);
}

// ======================= Helpers =======================

pub(crate) fn color_from_usvg(c: &usvg::Color, opacity: f32, group_opacity: f32) -> PaintColor {
    PaintColor {
        r: c.red as f32 / 255.0,
        g: c.green as f32 / 255.0,
        b: c.blue as f32 / 255.0,
        a: opacity * group_opacity,
    }
}

// ======================= Gradient Conversion =======================

use vello_cpu::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::kurbo::Point;
use vello_cpu::peniko::{ColorStop, ColorStops, Extend, Gradient};

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
pub(crate) fn convert_radial_gradient(
    gradient: &usvg::RadialGradient,
    bbox: usvg::Rect,
) -> Gradient {
    let fx = gradient.fx() as f64;
    let fy = gradient.fy() as f64;
    let fr = gradient.fr().get() as f64;
    let cx = gradient.cx() as f64;
    let cy = gradient.cy() as f64;
    let r = gradient.r().get() as f64;

    // Compute user-space coordinates for focal point and circle center.
    let (focal, center) = resolve_user_space_coords(
        gradient.units(), gradient.transform(),
        fx, fy, cx, cy, bbox,
    );

    // Convert to pixmap-local space.
    let bx = bbox.x() as f64;
    let by = bbox.y() as f64;
    let local_focal = Point::new(focal.x as f64 - bx, focal.y as f64 - by);
    let local_center = Point::new(center.x as f64 - bx, center.y as f64 - by);

    // Scale radii: for ObjectBoundingBox, multiply by bbox diagonal factor.
    // For UserSpaceOnUse, radii are in user units (no scaling needed).
    let (fr_local, r_local) = if gradient.units() == usvg::Units::ObjectBoundingBox {
        // Radii in ObjectBoundingBox are relative to the bbox diagonal (sqrt(w²+h²)/√2).
        let factor = ((bbox.width() as f64).powi(2) + (bbox.height() as f64).powi(2)).sqrt() / 1.41421356;
        (fr * factor, r * factor)
    } else {
        (fr, r)
    };

    let stops = convert_stops(gradient.stops());
    let mut g = Gradient::new_two_point_radial(
        local_focal, fr_local as f32,
        local_center, r_local as f32,
    );
    g.extend = convert_spread(gradient.spread_method());
    g.stops = stops;
    g
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
