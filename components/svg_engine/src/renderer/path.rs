/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::Transform2D;
use kurbo::{BezPath, PathEl, Point as KurboPoint, Shape};
use webrender_api::units::{LayoutPoint, LayoutRect};

use crate::renderer::providers::PaintResourceProvider;
use crate::renderer::{Render, RenderContext};
use crate::shapes::Path;
use crate::style::gradient::{GradientDef, GradientUnits, SpreadMethod};
use crate::style::{FillParams, FillRule, StrokeParams};
use crate::RasterizedImage;

use std::hash::{Hash, Hasher};

use vello_cpu::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::peniko::{ColorStop, ColorStops, Extend, Gradient};

/// Tolerance for flattening bezier curves into line segments.
/// Lower values = smoother curves, more segments.
/// 0.1 px is invisible to the user at any reasonable zoom level.
const FLATTEN_TOLERANCE: f64 = 0.1;

/// A resolved paint for rasterization — either a solid color or a gradient.
pub(crate) enum ResolvedPaint {
    Solid(AlphaColor<Srgb>),
    Gradient(Gradient),
}

/// Renders an SVG `<path>` via vello_cpu (solid or gradient, preserving curves).
impl Render for Path {
    fn render(&self, ctx: &mut RenderContext) {
        if ctx.native_rendering {
            // Pattern content: render natively so reference frames apply.
            // Stroke each subpath independently (no connecting segments);
            // fill only closed paths.
            let has_close = self
                .path
                .elements()
                .iter()
                .any(|e| matches!(e, PathEl::ClosePath));
            let fill_rule = ctx
                .style
                .fill
                .as_ref()
                .map(|f| f.fill_rule)
                .unwrap_or(FillRule::NonZero);
            let subpaths = flatten_subpaths(&self.path);
            let stroke_before_fill = crate::renderer::paint_order_stroke_before_fill(ctx);
            let has_stroke = ctx.style.stroke.is_some();
            let has_fill = has_close && ctx.style.fill.is_some();

            if stroke_before_fill {
                if has_stroke {
                    for subpath in &subpaths {
                        if subpath.len() >= 2 {
                            crate::renderer::polyline::render_native_stroke(subpath, ctx);
                        }
                    }
                }
                if has_fill {
                    let all: Vec<KurboPoint> = subpaths.iter().flatten().copied().collect();
                    if all.len() >= 3 {
                        crate::renderer::polyline::render_native_fill(&all, ctx, fill_rule);
                    }
                }
            } else {
                if has_fill {
                    let all: Vec<KurboPoint> = subpaths.iter().flatten().copied().collect();
                    if all.len() >= 3 {
                        crate::renderer::polyline::render_native_fill(&all, ctx, fill_rule);
                    }
                }
                if has_stroke {
                    for subpath in &subpaths {
                        if subpath.len() >= 2 {
                            crate::renderer::polyline::render_native_stroke(subpath, ctx);
                        }
                    }
                }
            }
            return;
        }

        // CPU-rasterized shapes bypass reference frames, so fold the nested
        // viewBox translation into the raster position explicitly.
        let raster_origin = LayoutPoint::new(
            ctx.svg_origin.x + ctx.raster_offset.x,
            ctx.svg_origin.y + ctx.raster_offset.y,
        );
        rasterize_bez(
            &self.path,
            ctx.style.fill.as_ref(),
            ctx.style.stroke.as_ref(),
            ctx.style.opacity,
            &raster_origin,
            ctx.viewbox_scale,
            ctx.device_scale,
            Transform2D::identity(),
            None,
            ctx.paints,
            ctx.rasters,
        );
    }
}

/// Convert an euclid `Transform2D` to a kurbo `Affine`.
fn transform_to_affine(xform: &Transform2D<f32, (), ()>) -> vello_cpu::kurbo::Affine {
    // euclid (column-vector):  x' = m11·x + m21·y + m31, y' = m12·x + m22·y + m32
    // kurbo Affine (augmented): | a c e |   x' = a·x + c·y + e
    //                           | b d f |   y' = b·x + d·y + f
    vello_cpu::kurbo::Affine::new([
        xform.m11 as f64,
        xform.m12 as f64,
        xform.m21 as f64,
        xform.m22 as f64,
        xform.m31 as f64,
        xform.m32 as f64,
    ])
}

/// Rasterize a `BezPath` (solid fill/stroke or gradient) via vello_cpu into a
/// [`RasterizedImage`], pushed onto `rasters`.
pub(crate) fn rasterize_bez(
    bez: &BezPath,
    fill: Option<&FillParams>,
    stroke: Option<&StrokeParams>,
    node_opacity: f32,
    svg_origin: &LayoutPoint,
    viewbox_scale: (f32, f32),
    scale: f32,
    node_xform: Transform2D<f32, (), ()>,
    clip_rect: Option<LayoutRect>,
    paints: &dyn PaintResourceProvider,
    rasters: &mut Vec<RasterizedImage>,
) {
    // Approximate scalar scale of the accumulated node transform, used to keep
    // stroke widths/dashes proportional. `sqrt(|det|)` is exact for uniform
    // scale and a reasonable approximation for skew/rotation.
    let node_scale = (node_xform.m11 * node_xform.m22 - node_xform.m12 * node_xform.m21)
        .abs()
        .sqrt()
        .max(1e-4);
    // CSS-space scale (viewBox × node transform), used for the stroke inset
    // that expands the layout-space bounding box.
    let css_scale = viewbox_scale.0 * node_scale;
    // Device-space scale (CSS scale × device pixel ratio), used for the stroke
    // width/dash lengths at raster time.
    let total_scale = css_scale * scale;

    // Apply the node transform (full affine: translate/scale/rotate/skew), then
    // the viewBox scale so the pixmap is rasterized at viewport resolution.
    let mut bez_scaled = bez.clone();
    bez_scaled.apply_affine(transform_to_affine(&node_xform));
    bez_scaled.apply_affine(vello_cpu::kurbo::Affine::scale_non_uniform(
        viewbox_scale.0 as f64,
        viewbox_scale.1 as f64,
    ));

    let mut bbox = bez_scaled.bounding_box();
    // Expand the pixmap to include the stroke, which is centered on the path
    // outline and would otherwise be clipped at the fill's bounding box.
    if let Some(s) = stroke {
        let inset = s.width as f64 * css_scale as f64 / 2.0;
        bbox = kurbo::Rect::new(
            bbox.x0 - inset,
            bbox.y0 - inset,
            bbox.x1 + inset,
            bbox.y1 + inset,
        );
    }
    if bbox.width() <= 0.0 || bbox.height() <= 0.0 {
        return;
    }
    // Layout-space pixmap size (1 CSS pixel = 1 pixel), then scaled up to the
    // device resolution so the compositor downsamples instead of upsampling.
    let css_w = (bbox.width().ceil() as u16).max(1);
    let css_h = (bbox.height().ceil() as u16).max(1);
    let w = ((css_w as f32 * scale).ceil() as u16).max(1);
    let h = ((css_h as f32 * scale).ceil() as u16).max(1);

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

    // Scale to device resolution, then offset the path so its (device-space)
    // bounding box sits at (0,0) within the device-resolution pixmap.
    let mut bez_local = bez_scaled;
    bez_local.apply_affine(vello_cpu::kurbo::Affine::scale_non_uniform(
        scale as f64,
        scale as f64,
    ));
    bez_local.apply_affine(vello_cpu::kurbo::Affine::translate((
        -bbox.x0 * scale as f64,
        -bbox.y0 * scale as f64,
    )));

    if let Some(f) = fill {
        if let Some(paint) = resolve_fill_paint(f, css_w as f32, css_h as f32, viewbox_scale, &bbox, node_opacity, paints) {
            context.set_fill_rule(match f.fill_rule {
                FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
            });
            apply_paint(&mut context, scale_paint(paint, scale as f64));
            context.fill_path(&bez_local);
        }
    }

    if let Some(s) = stroke {
        if let Some(paint) = resolve_stroke_paint(s, css_w as f32, css_h as f32, viewbox_scale, &bbox, node_opacity, paints) {
            apply_paint(&mut context, scale_paint(paint, scale as f64));
            let mut vello_stroke =
                vello_cpu::kurbo::Stroke::new(s.width as f64 * total_scale as f64);
            // Dash lengths/offset are in user units, so scale them by the
            // same factor as the path (viewBox scale × node-transform scale
            // × device scale) before handing them to kurbo, which implements
            // SVG's odd-length-doubling rule.
            if let Some(dashes) = &s.dash_array {
                let dash_scale = total_scale as f64;
                let pattern: Vec<f64> = dashes.iter().map(|d| *d as f64 * dash_scale).collect();
                vello_stroke =
                    vello_stroke.with_dashes(s.dash_offset as f64 * dash_scale, &pattern);
            }
            context.set_stroke(vello_stroke);
            context.stroke_path(&bez_local);
        }
    }

    context.flush();
    context.render(&mut target, &mut resources);

    let mut rgba: Vec<u8> = target
        .data()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();

    // Position and size of the raster in the target space (the space of
    // `svg_origin`), before any sub-viewport clip is applied. `x`/`y` are in
    // layout space; `width`/`height` are device pixels.
    let mut raster_x = svg_origin.x + bbox.x0 as f32;
    let mut raster_y = svg_origin.y + bbox.y0 as f32;
    let mut raster_w = w as u32;
    let mut raster_h = h as u32;

    // Clip the raster to the sub-viewport rect (when present), cropping the
    // pixmap data and repositioning it so only the visible part remains. The
    // clip rect lives in the same layout space as `svg_origin`, so pixel
    // offsets are scaled by the device factor.
    if let Some(clip) = clip_rect {
        let css_w = raster_w as f32 / scale;
        let css_h = raster_h as f32 / scale;
        let cx0 = raster_x.max(clip.min.x);
        let cy0 = raster_y.max(clip.min.y);
        let cx1 = (raster_x + css_w).min(clip.max.x);
        let cy1 = (raster_y + css_h).min(clip.max.y);
        if cx0 >= cx1 || cy0 >= cy1 {
            return; // fully outside the clip — nothing to draw
        }
        let px0 = ((cx0 - raster_x) * scale).floor() as u32;
        let py0 = ((cy0 - raster_y) * scale).floor() as u32;
        let px1 = (((cx1 - raster_x) * scale).ceil() as u32).min(raster_w);
        let py1 = (((cy1 - raster_y) * scale).ceil() as u32).min(raster_h);
        let cw = px1 - px0;
        let ch = py1 - py0;

        let mut cropped = Vec::with_capacity((cw * ch * 4) as usize);
        for row in py0..py1 {
            let start = (row * raster_w + px0) as usize * 4;
            let end = (row * raster_w + px1) as usize * 4;
            cropped.extend_from_slice(&rgba[start..end]);
        }
        rgba = cropped;
        raster_x += px0 as f32 / scale;
        raster_y += py0 as f32 / scale;
        raster_w = cw;
        raster_h = ch;
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rgba.hash(&mut hasher);
    let hash = hasher.finish();

    rasters.push(RasterizedImage {
        x: raster_x,
        y: raster_y,
        width: raster_w,
        height: raster_h,
        scale,
        data: rgba,
        content_hash: hash,
    });
}

/// Set the resolved paint on the render context.
fn apply_paint(context: &mut vello_cpu::RenderContext, paint: ResolvedPaint) {
    match paint {
        ResolvedPaint::Solid(c) => context.set_paint(c),
        ResolvedPaint::Gradient(g) => context.set_paint(g),
    }
}

/// Scale a resolved paint's gradient geometry into device space.
fn scale_paint(mut paint: ResolvedPaint, scale: f64) -> ResolvedPaint {
    if let ResolvedPaint::Gradient(g) = &mut paint {
        scale_gradient(g, scale);
    }
    paint
}

/// Scale a gradient's endpoints/radius by `scale` (device pixel ratio).
fn scale_gradient(gradient: &mut Gradient, scale: f64) {
    use vello_cpu::peniko::GradientKind;
    match &mut gradient.kind {
        GradientKind::Linear(pos) => {
            pos.start.x *= scale;
            pos.start.y *= scale;
            pos.end.x *= scale;
            pos.end.y *= scale;
        },
        GradientKind::Radial(pos) => {
            pos.start_center.x *= scale;
            pos.start_center.y *= scale;
            pos.start_radius = (pos.start_radius as f64 * scale) as f32;
            pos.end_center.x *= scale;
            pos.end_center.y *= scale;
            pos.end_radius = (pos.end_radius as f64 * scale) as f32;
        },
        GradientKind::Sweep(_) => {},
    }
}

/// Resolve a fill to a concrete paint (solid color or gradient).
fn resolve_fill_paint(
    fill: &FillParams,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    node_opacity: f32,
    paints: &dyn PaintResourceProvider,
) -> Option<ResolvedPaint> {
    if let Some(crate::style::gradient::PaintServer::Gradient(id)) = &fill.paint_server {
        if let Some(def) = paints.gradient(id) {
            return Some(ResolvedPaint::Gradient(gradient_def_to_peniko(def, w, h, viewbox_scale, bbox)));
        }
    }
    if let Some(color) = &fill.color {
        return Some(ResolvedPaint::Solid(vello_color(color, fill.opacity * node_opacity)));
    }
    None
}

/// Resolve a stroke to a concrete paint (solid color or gradient).
fn resolve_stroke_paint(
    stroke: &StrokeParams,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    node_opacity: f32,
    paints: &dyn PaintResourceProvider,
) -> Option<ResolvedPaint> {
    if let Some(crate::style::gradient::PaintServer::Gradient(id)) = &stroke.paint_server {
        if let Some(def) = paints.gradient(id) {
            return Some(ResolvedPaint::Gradient(gradient_def_to_peniko(def, w, h, viewbox_scale, bbox)));
        }
    }
    if let Some(color) = &stroke.color {
        return Some(ResolvedPaint::Solid(vello_color(color, stroke.opacity * node_opacity)));
    }
    None
}

/// Convert a [`GradientDef`] to a [`Gradient`] in pixmap-local coordinates.
fn gradient_def_to_peniko(
    def: &GradientDef,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
) -> Gradient {
    match def {
        GradientDef::Linear(lg) => linear_to_peniko(lg, w, h, viewbox_scale, bbox),
        GradientDef::Radial(rg) => radial_to_peniko(rg, w, h, viewbox_scale, bbox),
    }
}

/// Convert a linear gradient to a [`Gradient`] in pixmap-local coordinates.
fn linear_to_peniko(
    lg: &crate::style::gradient::LinearGradient,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
) -> Gradient {
    let (x1, y1, x2, y2) = match lg.units {
        GradientUnits::ObjectBoundingBox => (
            lg.x1.to_object_bbox() * w,
            lg.y1.to_object_bbox() * h,
            lg.x2.to_object_bbox() * w,
            lg.y2.to_object_bbox() * h,
        ),
        GradientUnits::UserSpaceOnUse => (
            lg.x1.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
            lg.y1.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
            lg.x2.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
            lg.y2.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
        ),
    };
    let mut g = Gradient::new_linear(
        vello_cpu::kurbo::Point::new(x1 as f64, y1 as f64),
        vello_cpu::kurbo::Point::new(x2 as f64, y2 as f64),
    );
    g.extend = spread_to_extend(lg.spread_method);
    g.stops = stops_to_colorstops(&lg.stops);
    g
}

/// Convert a radial gradient to a [`Gradient`] in pixmap-local coordinates.
fn radial_to_peniko(
    rg: &crate::style::gradient::RadialGradient,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
) -> Gradient {
    let scale = w.max(h);
    let (cx, cy, r, fx, fy) = match rg.units {
        GradientUnits::ObjectBoundingBox => (
            rg.cx.to_object_bbox() * w,
            rg.cy.to_object_bbox() * h,
            rg.r.to_object_bbox() * scale,
            rg.fx.to_object_bbox() * w,
            rg.fy.to_object_bbox() * h,
        ),
        GradientUnits::UserSpaceOnUse => (
            rg.cx.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
            rg.cy.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
            rg.r.to_user_space(scale) * viewbox_scale.0,
            rg.fx.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
            rg.fy.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
        ),
    };
    let mut g = Gradient::new_two_point_radial(
        vello_cpu::kurbo::Point::new(fx as f64, fy as f64),
        0.0, // focal radius (fr) — not modelled by svg-text's gradient type
        vello_cpu::kurbo::Point::new(cx as f64, cy as f64),
        r,
    );
    g.extend = spread_to_extend(rg.spread_method);
    g.stops = stops_to_colorstops(&rg.stops);
    g
}

/// Convert svg-text gradient stops to peniko [`ColorStops`].
fn stops_to_colorstops(stops: &[crate::style::gradient::GradientStop]) -> ColorStops {
    let items: Vec<ColorStop> = stops
        .iter()
        .map(|s| ColorStop {
            offset: s.offset,
            color: DynamicColor::from_alpha_color(
                AlphaColor::<Srgb>::from_rgba8(s.color.red, s.color.green, s.color.blue, s.color.alpha),
            ),
        })
        .collect();
    ColorStops(items.into())
}

/// Convert an svg-text spread method to peniko [`Extend`].
fn spread_to_extend(spread: SpreadMethod) -> Extend {
    match spread {
        SpreadMethod::Pad => Extend::Pad,
        SpreadMethod::Reflect => Extend::Reflect,
        SpreadMethod::Repeat => Extend::Repeat,
    }
}

/// Convert an `svgtypes::Color` to a vello_cpu solid paint, applying opacity.
fn vello_color(c: &svgtypes::Color, opacity: f32) -> AlphaColor<Srgb> {
    let a = (c.alpha as f32 * opacity).round().clamp(0.0, 255.0) as u8;
    AlphaColor::from_rgba8(c.red, c.green, c.blue, a)
}

/// Flatten a `BezPath` into a list of subpaths (each a sequence of points),
/// preserving the `MoveTo` subpath boundaries so the native renderer can
/// stroke each subpath independently (no connecting segments).
fn flatten_subpaths(path: &BezPath) -> Vec<Vec<KurboPoint>> {
    let mut subpaths: Vec<Vec<KurboPoint>> = Vec::new();
    let mut current: Vec<KurboPoint> = Vec::new();
    let mut subpath_start: Option<KurboPoint> = None;

    kurbo::flatten(
        path.elements().iter().copied(),
        FLATTEN_TOLERANCE,
        |el| match el {
            PathEl::MoveTo(p) => {
                if !current.is_empty() {
                    subpaths.push(std::mem::take(&mut current));
                }
                current.push(p);
                subpath_start = Some(p);
            },
            PathEl::LineTo(p) => {
                current.push(p);
            },
            PathEl::ClosePath => {
                if let Some(start) = subpath_start {
                    current.push(start);
                }
                subpath_start = None;
            },
            _ => {},
        },
    );

    if !current.is_empty() {
        subpaths.push(current);
    }

    subpaths
}
