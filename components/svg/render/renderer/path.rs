/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering for SVG `<path>` (vello_cpu rasterization or native primitives).

use std::hash::{Hash, Hasher};

use euclid::Transform2D;
use kurbo::{BezPath, PathEl, Point as KurboPoint, Shape};
use vello_cpu::color::{AlphaColor, ColorSpaceTag, DynamicColor, Srgb};
use vello_cpu::peniko::{ColorStop, ColorStops, Extend, Gradient, InterpolationAlphaSpace};
use webrender_api::DisplayListBuilder;
use webrender_api::units::{LayoutPoint, LayoutRect};

use crate::model::document::{Defs, GradientDef, GradientUnits, SpreadMethod};
use crate::model::element::shape::Path;
use crate::model::style::paint_servers::PaintServer;
use crate::model::style::{ColorInterpolation, FillParams, FillRule, LineCap, LineJoin, StrokeParams};
use crate::render::geometry::{ComplexClip, path_data_to_bez};
use crate::render::renderer::{
    Render, RenderContext, color_interpolation, fill_rule, non_scaling_stroke, transform,
};
use crate::{RasterSink, RasterizedImage};

/// Tolerance for flattening bezier curves into line segments.
/// Lower values = smoother curves, more segments.
/// 0.1 px is invisible to the user at any reasonable zoom level.
const FLATTEN_TOLERANCE: f64 = 0.1;

/// A resolved paint for rasterization — either a solid color or a gradient.
pub(crate) enum ResolvedPaint {
    Solid(AlphaColor<Srgb>),
    /// A gradient plus an optional paint transform.  Radial gradients in
    /// `objectBoundingBox` units become elliptical under a non-uniform bounding
    /// box (§13.2.2), but peniko's `Gradient` models only circular radii — the
    /// ellipse is encoded as a circle plus a non-uniform paint transform (see
    /// [`radial_to_peniko`]).  A radial gradient with a `gradientTransform` is
    /// likewise a circle in its own coordinate space plus the paint transform
    /// that places it (see [`transformed_radial_to_peniko`]).
    Gradient(Gradient, Option<vello_cpu::kurbo::Affine>),
}

/// Renders an SVG `<path>` via vello_cpu (solid or gradient, preserving curves).
impl Render for Path {
    fn render(&self, ctx: &mut RenderContext) {
        // Round-trip the model path back into kurbo space for the native/bez
        // pipeline.
        let bez = path_data_to_bez(&self.path);

        if ctx.native_rendering {
            // Pattern content: render natively so reference frames apply.
            // Stroke each subpath independently (no connecting segments);
            // fill only closed paths.
            let has_close = bez
                .elements()
                .iter()
                .any(|e| matches!(e, PathEl::ClosePath));
            let fill_rule = fill_rule(ctx);
            let subpaths = flatten_subpaths(&bez);
            let stroke_before_fill = crate::render::renderer::paint_order_stroke_before_fill(ctx);
            let has_stroke = ctx.style.stroke.is_some();
            let has_fill = has_close && ctx.style.fill.is_some();

            if stroke_before_fill {
                if has_stroke {
                    for subpath in &subpaths {
                        if subpath.len() >= 2 {
                            crate::render::renderer::polyline::render_native_stroke(subpath, ctx);
                        }
                    }
                }
                if has_fill {
                    let all: Vec<KurboPoint> = subpaths.iter().flatten().copied().collect();
                    if all.len() >= 3 {
                        crate::render::renderer::polyline::render_native_fill(&all, ctx, fill_rule);
                    }
                }
            } else {
                if has_fill {
                    let all: Vec<KurboPoint> = subpaths.iter().flatten().copied().collect();
                    if all.len() >= 3 {
                        crate::render::renderer::polyline::render_native_fill(&all, ctx, fill_rule);
                    }
                }
                if has_stroke {
                    for subpath in &subpaths {
                        if subpath.len() >= 2 {
                            crate::render::renderer::polyline::render_native_stroke(subpath, ctx);
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
            &bez,
            ctx.style.fill.as_ref(),
            ctx.style.stroke.as_ref(),
            ctx.style.opacity.get(),
            &raster_origin,
            ctx.viewbox_scale,
            ctx.device_scale,
            Transform2D::identity(),
            non_scaling_stroke(ctx),
            None,
            &[],
            ctx.wr,
            ctx.sink,
            None,
            self.path_length,
            ctx.defs,
            color_interpolation(ctx),
        );
    }
}

/// Map an SVG [`LineCap`] to kurbo's [`Cap`].
pub(crate) fn line_cap_to_kurbo(cap: LineCap) -> vello_cpu::kurbo::Cap {
    match cap {
        LineCap::Butt => vello_cpu::kurbo::Cap::Butt,
        LineCap::Round => vello_cpu::kurbo::Cap::Round,
        LineCap::Square => vello_cpu::kurbo::Cap::Square,
    }
}

/// Map an SVG [`LineJoin`] to kurbo's [`Join`]. `miter-clip` and `arcs` have no
/// direct kurbo equivalent; approximate them as miter and round respectively.
pub(crate) fn line_join_to_kurbo(join: LineJoin) -> vello_cpu::kurbo::Join {
    match join {
        LineJoin::Miter | LineJoin::MiterClip => vello_cpu::kurbo::Join::Miter,
        LineJoin::Round | LineJoin::Arcs => vello_cpu::kurbo::Join::Round,
        LineJoin::Bevel => vello_cpu::kurbo::Join::Bevel,
    }
}

/// Convert an euclid `Transform2D` to a kurbo `Affine`.
pub(crate) fn transform_to_affine(xform: &Transform2D<f32, (), ()>) -> vello_cpu::kurbo::Affine {
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
/// [`RasterizedImage`], uploaded and pushed inline through `sink`.
///
/// `shape_ci` is the referencing shape's effective `color-interpolation`
/// (resolved to `Srgb`/`LinearRGB`); a gradient's own authored value still
/// takes precedence over it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_bez(
    bez: &BezPath,
    fill: Option<&FillParams>,
    stroke: Option<&StrokeParams>,
    node_opacity: f32,
    svg_origin: &LayoutPoint,
    viewbox_scale: (f32, f32),
    scale: f32,
    node_xform: Transform2D<f32, (), ()>,
    non_scaling_stroke: bool,
    clip_rect: Option<LayoutRect>,
    complex_clips: &[ComplexClip],
    wr: &mut DisplayListBuilder,
    sink: &RasterSink,
    alpha_mask: Option<&crate::render::effects::mask::MaskRaster>,
    path_length: Option<f32>,
    defs: Defs<'_>,
    shape_ci: ColorInterpolation,
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
    // `vector-effect: non-scaling-stroke` keeps the stroke constant in screen
    // space, ignoring the element's transforms (and the viewBox scale). The
    // geometry is still transformed, so the stroke is rasterized at its
    // authored width in screen CSS pixels — only the device scale applies.
    let stroke_scale = if non_scaling_stroke {
        scale
    } else {
        total_scale
    };
    // The stroke's width in the CSS space `bez_scaled` lives in: used to expand
    // the pixmap bounds to include the (centered) stroke overhang.
    let stroke_css_scale = stroke_scale / scale;

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
        // A centered stroke extends half its width beyond the path outline.
        // Sharp miter joins extend further — up to (width/2) × miter-limit —
        // and would be clipped at the pixmap edge if we only reserved the
        // half-width (a clipped miter tip renders like a bevel). Square caps
        // likewise reach (width/2) × √2 at their corners.
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
        bbox = kurbo::Rect::new(
            bbox.x0 - inset,
            bbox.y0 - inset,
            bbox.x1 + inset,
            bbox.y1 + inset,
        );
    }
    // Clamp the raster bounds to the visible sub-viewport so off-screen
    // geometry (huge coordinates / stroke widths) can't size an enormous
    // pixmap. `bbox` lives in the rasterized path's local space while
    // `clip_rect` lives in the same space as `svg_origin` (see the crop
    // below), so translate `bbox` by `svg_origin` to intersect, then back.
    if let Some(clip) = clip_rect {
        let l = kurbo::Rect::new(
            svg_origin.x as f64 + bbox.x0,
            svg_origin.y as f64 + bbox.y0,
            svg_origin.x as f64 + bbox.x1,
            svg_origin.y as f64 + bbox.y1,
        );
        let c = kurbo::Rect::new(
            clip.min.x as f64,
            clip.min.y as f64,
            clip.max.x as f64,
            clip.max.y as f64,
        );
        let inter = l.intersect(c);
        if inter.width() <= 0.0 || inter.height() <= 0.0 {
            return; // fully outside the viewport — nothing to draw
        }
        bbox = kurbo::Rect::new(
            inter.x0 - svg_origin.x as f64,
            inter.y0 - svg_origin.y as f64,
            inter.x1 - svg_origin.x as f64,
            inter.y1 - svg_origin.y as f64,
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

    // Apply polygon/path clip-paths (and complex mask shapes) as vello clip
    // paths. Their geometry is already translated by `cur_origin` (the node's
    // translation-only origin), which matches `node_xform`'s translation for
    // translate-only transforms. They still need the viewBox scale, the device
    // scale, and the bbox translation to land in pixmap space — the same
    // transforms applied to `bez_local` after `node_xform`.
    for clip in complex_clips {
        let mut clip_local = clip.path.clone();
        clip_local.apply_affine(vello_cpu::kurbo::Affine::scale_non_uniform(
            viewbox_scale.0 as f64,
            viewbox_scale.1 as f64,
        ));
        clip_local.apply_affine(vello_cpu::kurbo::Affine::scale_non_uniform(
            scale as f64,
            scale as f64,
        ));
        clip_local.apply_affine(vello_cpu::kurbo::Affine::translate((
            -bbox.x0 * scale as f64,
            -bbox.y0 * scale as f64,
        )));
        context.set_fill_rule(match clip.fill_rule {
            FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
            FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
        });
        context.push_clip_path(&clip_local);
    }

    if let Some(f) = fill {
        if let Some(paint) = resolve_fill_paint(
            f,
            css_w as f32,
            css_h as f32,
            viewbox_scale,
            &bbox,
            node_opacity,
            defs,
            shape_ci,
        ) {
            context.set_fill_rule(match f.fill_rule {
                FillRule::NonZero => vello_cpu::peniko::Fill::NonZero,
                FillRule::EvenOdd => vello_cpu::peniko::Fill::EvenOdd,
            });
            apply_paint(&mut context, scale_paint(paint, scale as f64));
            context.fill_path(&bez_local);
        }
    }

    if let Some(s) = stroke {
        if let Some(paint) = resolve_stroke_paint(
            s,
            css_w as f32,
            css_h as f32,
            viewbox_scale,
            &bbox,
            node_opacity,
            defs,
            shape_ci,
        ) {
            apply_paint(&mut context, scale_paint(paint, scale as f64));
            let mut vello_stroke =
                vello_cpu::kurbo::Stroke::new(s.width.get() as f64 * stroke_scale as f64)
                    .with_caps(line_cap_to_kurbo(s.line_cap))
                    .with_join(line_join_to_kurbo(s.line_join))
                    .with_miter_limit(s.miter_limit.max(1.0) as f64);
            // Dash lengths/offset are in user units, so scale them by the
            // same factor as the path (viewBox scale × node-transform scale
            // × device scale) before handing them to kurbo, which implements
            // SVG's odd-length-doubling rule.
            if let Some(dashes) = &s.dash_array {
                // SVG 2 `pathLength`: calibrate dash lengths/offset by the ratio
                // of the actual path length to the author-specified length, so
                // the dash pattern spans the path as the author intended
                // regardless of the real geometry.
                let calibration = match path_length {
                    Some(pl) if pl > 0.0 && pl.is_finite() => {
                        (bez.perimeter(FLATTEN_TOLERANCE) / pl as f64).max(1e-6)
                    },
                    _ => 1.0,
                };
                let dash_scale = stroke_scale as f64 * calibration;
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

    // Multiply the content's alpha by a rasterized luminance/alpha mask. The
    // mask raster lives in the same layout space as `svg_origin`, so each
    // content pixel is mapped through the shared layout coordinate system into
    // the mask's device-resolution pixmap. Pixels outside the mask's bounding
    // region are fully hidden (alpha 0).
    if let Some(mask) = alpha_mask {
        let mask_w = mask.width as usize;
        for row in 0..raster_h {
            for col in 0..raster_w {
                let idx = ((row * raster_w + col) * 4) as usize;
                let lx = raster_x + (col as f32 + 0.5) / scale;
                let ly = raster_y + (row as f32 + 0.5) / scale;
                let mx = (lx - mask.x) * mask.scale;
                let my = (ly - mask.y) * mask.scale;
                let mask_alpha: f32 = if mx < 0.0 ||
                    my < 0.0 ||
                    mx >= mask.width as f32 ||
                    my >= mask.height as f32
                {
                    0.0
                } else {
                    let mxi = mx as usize;
                    let myi = my as usize;
                    mask.data[((myi * mask_w + mxi) * 4) + 3] as f32 / 255.0
                };
                // `rgba` is premultiplied (vello_cpu produces premultiplied
                // pixels, and the raster is uploaded as PremultipliedAlpha), so
                // scaling the mask into the alpha must scale RGB by the same
                // factor to keep the unpremultiplied color unchanged.
                rgba[idx + 0] = (rgba[idx + 0] as f32 * mask_alpha).round() as u8;
                rgba[idx + 1] = (rgba[idx + 1] as f32 * mask_alpha).round() as u8;
                rgba[idx + 2] = (rgba[idx + 2] as f32 * mask_alpha).round() as u8;
                rgba[idx + 3] = (rgba[idx + 3] as f32 * mask_alpha).round() as u8;
            }
        }
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rgba.hash(&mut hasher);
    let hash = hasher.finish();

    sink.emit(
        wr,
        RasterizedImage {
            x: raster_x,
            y: raster_y,
            width: raster_w,
            height: raster_h,
            scale,
            data: rgba,
            content_hash: hash,
        },
    );
}

/// Set the resolved paint on the render context.
pub(crate) fn apply_paint(context: &mut vello_cpu::RenderContext, paint: ResolvedPaint) {
    match paint {
        ResolvedPaint::Solid(c) => {
            context.reset_paint_transform();
            context.set_paint(c);
        },
        ResolvedPaint::Gradient(g, transform) => {
            context.set_paint(g);
            match transform {
                Some(t) => context.set_paint_transform(t),
                None => context.reset_paint_transform(),
            }
        },
    }
}

/// Scale a resolved paint's gradient geometry into device space.
pub(crate) fn scale_paint(mut paint: ResolvedPaint, scale: f64) -> ResolvedPaint {
    if let ResolvedPaint::Gradient(g, transform) = &mut paint {
        scale_gradient(g, scale);
        // The paint transform lives in the same space as the gradient geometry,
        // so it must scale to device space too.  Conjugating a scale-around-a-
        // center transform by a uniform scale scales its center while leaving the
        // (non-uniform) scale factor unchanged.
        if let Some(t) = transform {
            let uniform = vello_cpu::kurbo::Affine::scale(scale);
            *t = uniform * *t * vello_cpu::kurbo::Affine::scale(1.0 / scale);
        }
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
pub(crate) fn resolve_fill_paint(
    fill: &FillParams,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    node_opacity: f32,
    defs: Defs<'_>,
    shape_ci: ColorInterpolation,
) -> Option<ResolvedPaint> {
    resolve_paint(
        &fill.paint_server,
        fill.opacity.get(),
        node_opacity,
        w,
        h,
        viewbox_scale,
        bbox,
        defs,
        shape_ci,
    )
}

/// Resolve a stroke to a concrete paint (solid color or gradient).
pub(crate) fn resolve_stroke_paint(
    stroke: &StrokeParams,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    node_opacity: f32,
    defs: Defs<'_>,
    shape_ci: ColorInterpolation,
) -> Option<ResolvedPaint> {
    resolve_paint(
        &stroke.paint_server,
        stroke.opacity.get(),
        node_opacity,
        w,
        h,
        viewbox_scale,
        bbox,
        defs,
        shape_ci,
    )
}

/// Shared paint-server resolution for fill and stroke, which differ only in
/// their per-paint opacity.
fn resolve_paint(
    paint_server: &Option<PaintServer>,
    paint_opacity: f32,
    node_opacity: f32,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    defs: Defs<'_>,
    shape_ci: ColorInterpolation,
) -> Option<ResolvedPaint> {
    // Bind any `url(#id)` reference to a definition now, at render time.
    let resolved = paint_server.as_ref().and_then(|p| defs.resolve_paint_server(p));
    match &resolved {
        Some(PaintServer::Gradient(def)) => {
            let (mut gradient, transform) =
                gradient_def_to_peniko(def.as_ref(), w, h, viewbox_scale, bbox, shape_ci)?;
            // Gradients carry their alpha in the stop colors, so `fill-opacity`/
            // `stroke-opacity`/`opacity` must be folded in by scaling every
            // stop's premultiplied components (mirrors the solid-paint branch).
            scale_gradient_opacity(&mut gradient, paint_opacity * node_opacity);
            Some(ResolvedPaint::Gradient(gradient, transform))
        },
        Some(PaintServer::Solid(color)) => Some(ResolvedPaint::Solid(vello_color(
            color,
            paint_opacity * node_opacity,
        ))),
        // Pattern content is tiled by the native renderer; `context-fill`/
        // `context-stroke` and broken references rasterize as no paint.
        Some(PaintServer::Pattern(_)) |
        Some(PaintServer::Ref { .. }) |
        Some(PaintServer::ContextFill) |
        Some(PaintServer::ContextStroke) |
        None => None,
    }
}

/// Convert a [`GradientDef`] to a [`Gradient`] for rasterization.  `None`
/// means the gradient paints nothing (non-invertible `gradientTransform`).
fn gradient_def_to_peniko(
    def: &GradientDef,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    shape_ci: ColorInterpolation,
) -> Option<(Gradient, Option<vello_cpu::kurbo::Affine>)> {
    match def {
        GradientDef::Linear(lg) => Some((
            linear_to_peniko(lg, w, h, viewbox_scale, bbox, shape_ci),
            None,
        )),
        GradientDef::Radial(rg) => radial_to_peniko(rg, w, h, viewbox_scale, bbox, shape_ci),
    }
}

/// Convert a linear gradient to a [`Gradient`] in pixmap-local coordinates.
fn linear_to_peniko(
    lg: &crate::model::document::LinearGradient,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    shape_ci: ColorInterpolation,
) -> Gradient {
    let (x1, y1, x2, y2) = match lg.units {
        GradientUnits::ObjectBoundingBox => {
            // Project in the normalized object-bounding-box space (Chrome's
            // behavior): non-uniform bbox scaling does not preserve perpendicular
            // projection, so projecting in the raw scaled box skews diagonals.
            let (x1, y1, x2, y2) =
                crate::render::renderer::gradient::object_bbox_linear_fractions(lg);
            crate::render::renderer::gradient::object_bbox_linear_endpoints(
                x1, y1, x2, y2, 0.0, 0.0, w, h,
            )
        },
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
    g.interpolation_cs = gradient_ci_to_tag(resolve_gradient_ci(lg.color_interpolation, shape_ci));
    // SVG gradients interpolate stop colors in *unpremultiplied* (straight)
    // alpha space, unlike CSS `linear-gradient()` which premultiplies. With
    // premultiplied interpolation a `white@1 → black@0` ramp fades a constant
    // white, but SVG (and Chrome/Skia) fades through gray, so the stop color
    // itself darkens as the alpha falls. This is the "transparent == transparent
    // black" behavior documented for SVG `stop-color`.
    g.interpolation_alpha_space = InterpolationAlphaSpace::Unpremultiplied;
    g
}

/// Convert a radial gradient to a [`Gradient`]: in pixmap-local coordinates,
/// or — when it carries a `gradientTransform` — in its own coordinate space
/// plus the paint transform that places it (see [`transformed_radial_to_peniko`]).
///
/// Returns `None` when the gradient paints nothing (a non-invertible
/// `gradientTransform`).
fn radial_to_peniko(
    rg: &crate::model::document::RadialGradient,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    shape_ci: ColorInterpolation,
) -> Option<(Gradient, Option<vello_cpu::kurbo::Affine>)> {
    // `gradientTransform` is a general affine: under it the gradient circle can
    // become a rotated ellipse, which the circle-plus-axis-aligned-scale
    // encoding below cannot express.  Transformed gradients therefore stay in
    // their own coordinate space instead.
    if !rg.transform.is_empty() {
        return transformed_radial_to_peniko(rg, w, h, viewbox_scale, bbox, shape_ci);
    }

    // Resolve the center, focal point, outer radii, and focal radius into the
    // pixmap-local (bbox-relative) space.  For `objectBoundingBox` units the
    // outer radius resolves against *both* bbox dimensions (§13.2.2): a
    // non-square bounding box turns the conceptually-circular gradient into an
    // ellipse.  `userSpaceOnUse` is uniform, so its gradient is a circle.
    let (cx, cy, rx, ry, fx, fy, fr) = match rg.units {
        GradientUnits::ObjectBoundingBox => (
            rg.cx.to_object_bbox() * w,
            rg.cy.to_object_bbox() * h,
            rg.r.to_object_bbox() * w,
            rg.r.to_object_bbox() * h,
            rg.fx.to_object_bbox() * w,
            rg.fy.to_object_bbox() * h,
            rg.fr.to_object_bbox() * w,
        ),
        GradientUnits::UserSpaceOnUse => {
            let nd = crate::render::renderer::gradient::normalized_diagonal(w, h);
            let r = rg.r.to_user_space(nd) * viewbox_scale.0;
            (
                rg.cx.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
                rg.cy.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
                r,
                r,
                rg.fx.to_user_space(w) * viewbox_scale.0 - bbox.x0 as f32,
                rg.fy.to_user_space(h) * viewbox_scale.1 - bbox.y0 as f32,
                rg.fr.to_user_space(nd) * viewbox_scale.0,
            )
        },
    };

    // peniko's `Gradient` models only circular radii, so an ellipse (rx != ry) is
    // expressed as a circle of radius `rx` plus a paint transform that scales `y`
    // by `ry / rx` about the center.  The gradient — including its focal point —
    // is evaluated in that circle space, so the focal point is pre-transformed
    // into it as well.
    let is_ellipse = rx > 0.0 && ry > 0.0 && (rx - ry).abs() > f32::EPSILON;
    let paint_transform = if is_ellipse {
        let k = (ry / rx) as f64;
        Some(
            vello_cpu::kurbo::Affine::translate((cx as f64, cy as f64)) *
                vello_cpu::kurbo::Affine::scale_non_uniform(1.0, k) *
                vello_cpu::kurbo::Affine::translate((-cx as f64, -cy as f64)),
        )
    } else {
        None
    };
    let (focal_x, focal_y) = if is_ellipse {
        (fx, cy + (fy - cy) * (rx / ry))
    } else {
        (fx, fy)
    };

    Some(radial_gradient(
        rg,
        vello_cpu::kurbo::Point::new(focal_x as f64, focal_y as f64),
        fr,
        vello_cpu::kurbo::Point::new(cx as f64, cy as f64),
        rx,
        shape_ci,
        paint_transform,
    ))
}

/// [`radial_to_peniko`] for a gradient carrying a `gradientTransform`.
///
/// The geometry stays in the gradient's own coordinate space — the unit
/// bounding-box square for `objectBoundingBox`, user space for
/// `userSpaceOnUse` — where it is still a circle, and the paint transform
/// carries the whole placement: `gradientTransform` first, then the
/// bounding-box scale, or the viewBox scale and the bbox origin.
fn transformed_radial_to_peniko(
    rg: &crate::model::document::RadialGradient,
    w: f32,
    h: f32,
    viewbox_scale: (f32, f32),
    bbox: &kurbo::Rect,
    shape_ci: ColorInterpolation,
) -> Option<(Gradient, Option<vello_cpu::kurbo::Affine>)> {
    let gradient_transform =
        transform_to_affine(&transform::compute_transform_matrix(&rg.transform));
    let ((cx, cy, r), (fx, fy, fr), placement) = match rg.units {
        GradientUnits::ObjectBoundingBox => (
            (
                rg.cx.to_object_bbox(),
                rg.cy.to_object_bbox(),
                rg.r.to_object_bbox(),
            ),
            (
                rg.fx.to_object_bbox(),
                rg.fy.to_object_bbox(),
                rg.fr.to_object_bbox(),
            ),
            vello_cpu::kurbo::Affine::scale_non_uniform(w as f64, h as f64) * gradient_transform,
        ),
        GradientUnits::UserSpaceOnUse => {
            let nd = crate::render::renderer::gradient::normalized_diagonal(w, h);
            (
                (
                    rg.cx.to_user_space(w),
                    rg.cy.to_user_space(h),
                    rg.r.to_user_space(nd),
                ),
                (
                    rg.fx.to_user_space(w),
                    rg.fy.to_user_space(h),
                    rg.fr.to_user_space(nd),
                ),
                vello_cpu::kurbo::Affine::translate((-bbox.x0, -bbox.y0))
                    * vello_cpu::kurbo::Affine::scale_non_uniform(
                        viewbox_scale.0 as f64,
                        viewbox_scale.1 as f64,
                    )
                    * gradient_transform,
            )
        },
    };
    // A non-invertible placement leaves no gradient space to sample from; as in
    // Chrome, such a gradient paints nothing.
    if !placement.is_finite() || placement.determinant() == 0.0 {
        return None;
    }
    Some(radial_gradient(
        rg,
        vello_cpu::kurbo::Point::new(fx as f64, fy as f64),
        fr,
        vello_cpu::kurbo::Point::new(cx as f64, cy as f64),
        r,
        shape_ci,
        Some(placement),
    ))
}

/// Build the peniko [`Gradient`] whose radial geometry — focal point, focal
/// radius, center and outer radius — is given in the space the gradient is
/// evaluated in, with `paint_transform` mapping that space to the pixmap.
fn radial_gradient(
    rg: &crate::model::document::RadialGradient,
    focal: vello_cpu::kurbo::Point,
    fr: f32,
    center: vello_cpu::kurbo::Point,
    r: f32,
    shape_ci: ColorInterpolation,
    paint_transform: Option<vello_cpu::kurbo::Affine>,
) -> (Gradient, Option<vello_cpu::kurbo::Affine>) {
    let mut g = Gradient::new_two_point_radial(
        focal,
        fr.max(0.0), // focal radius (fr); clamp negative to 0 per SVG 2
        center,
        r,
    );
    g.extend = spread_to_extend(rg.spread_method);
    g.stops = stops_to_colorstops(&rg.stops);
    g.interpolation_cs = gradient_ci_to_tag(resolve_gradient_ci(rg.color_interpolation, shape_ci));
    // Same as `linear_to_peniko`: SVG radial gradients interpolate stop colors
    // in unpremultiplied (straight) alpha space, matching Chrome/Skia.
    g.interpolation_alpha_space = InterpolationAlphaSpace::Unpremultiplied;
    (g, paint_transform)
}

/// The effective color-interpolation of a gradient: the gradient's own authored
/// value wins; `auto` defers to the referencing shape's value (`shape_ci`).
fn resolve_gradient_ci(gradient_ci: ColorInterpolation, shape_ci: ColorInterpolation) -> ColorInterpolation {
    match gradient_ci {
        ColorInterpolation::Auto => shape_ci,
        other => other,
    }
}

/// Map an SVG [`ColorInterpolation`] to peniko's interpolation color space.
///
/// `Auto` and `Srgb` both interpolate in sRGB; `LinearRGB` uses linear-light
/// interpolation (`ColorSpaceTag::LinearSrgb`).
fn gradient_ci_to_tag(ci: ColorInterpolation) -> ColorSpaceTag {
    match ci {
        ColorInterpolation::LinearRGB => ColorSpaceTag::LinearSrgb,
        ColorInterpolation::Srgb | ColorInterpolation::Auto => ColorSpaceTag::Srgb,
    }
}

/// Convert svg-text gradient stops to peniko [`ColorStops`].
fn stops_to_colorstops(stops: &[crate::model::document::GradientStop]) -> ColorStops {
    let items: Vec<ColorStop> = stops
        .iter()
        .map(|s| ColorStop {
            offset: s.offset,
            color: DynamicColor::from_alpha_color(AlphaColor::<Srgb>::from_rgba8(
                s.color.red,
                s.color.green,
                s.color.blue,
                s.color.alpha,
            )),
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

/// Scale a peniko gradient's opacity by `opacity`.
///
/// Stop colors are stored as [`DynamicColor`] with *unpremultiplied* (straight)
/// RGB components and separate alpha (the fourth component). Fading a gradient
/// must scale only the alpha, keeping the unpremultiplied stop color unchanged —
/// mirroring the solid-paint branch, which scales only alpha. Scaling all four
/// components would also darken the color (e.g. white → gray).
fn scale_gradient_opacity(gradient: &mut Gradient, opacity: f32) {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return;
    }
    for stop in gradient.stops.iter_mut() {
        stop.color.components[3] *= opacity;
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

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use vello_cpu::kurbo::{Point, Rect};
    use vello_cpu::peniko::{GradientKind, RadialGradientPosition};

    use super::*;
    use crate::model::document::{GradientExplicit, GradientLength, RadialGradient};
    use crate::model::transform::TransformOp;

    /// A concentric `<radialGradient>` centered at (50%, 50%) with `r="30%"`.
    fn radial(units: GradientUnits, transform: Vec<TransformOp>) -> RadialGradient {
        RadialGradient {
            id: "g".into(),
            href: None,
            cx: GradientLength::Percentage(50.0),
            cy: GradientLength::Percentage(50.0),
            r: GradientLength::Percentage(30.0),
            fx: GradientLength::Percentage(50.0),
            fy: GradientLength::Percentage(50.0),
            fr: GradientLength::Number(0.0),
            units,
            stops: vec![],
            transform,
            spread_method: SpreadMethod::Reflect,
            color_interpolation: ColorInterpolation::Auto,
            explicit: GradientExplicit::default(),
        }
    }

    /// The radial position peniko recorded for `g`.
    fn radial_position(g: &Gradient) -> RadialGradientPosition {
        assert_eq!(g.extend, Extend::Reflect);
        let GradientKind::Radial(pos) = &g.kind else {
            panic!("expected a radial gradient");
        };
        *pos
    }

    /// Resolve `rg` against a 240x80 box at the origin, without a viewBox
    /// scale, into its radial position and paint transform.
    fn resolve(
        rg: &RadialGradient,
    ) -> Option<(RadialGradientPosition, Option<vello_cpu::kurbo::Affine>)> {
        let bbox = Rect::new(0.0, 0.0, 240.0, 80.0);
        let (g, paint_transform) =
            radial_to_peniko(rg, 240.0, 80.0, (1.0, 1.0), &bbox, ColorInterpolation::Srgb)?;
        Some((radial_position(&g), paint_transform))
    }

    fn assert_point_approx(got: Point, want: (f64, f64)) {
        assert!(
            (got.x - want.0).abs() < 1e-4 && (got.y - want.1).abs() < 1e-4,
            "point {got:?} != {want:?}"
        );
    }

    /// Without a `gradientTransform` the gradient lives in pixmap-local space:
    /// the ellipse of a 240x80 box is a 72px circle squeezed by 1/3 in `y`.
    #[test]
    fn radial_without_transform_stays_in_pixmap_space() {
        let (pos, paint_transform) =
            resolve(&radial(GradientUnits::ObjectBoundingBox, vec![])).expect("gradient paints");
        assert_point_approx(pos.start_center, (120.0, 40.0));
        assert_point_approx(pos.end_center, (120.0, 40.0));
        assert!(pos.start_radius == 0.0 && (pos.end_radius - 72.0).abs() < 1e-4);
        let squeeze = paint_transform.expect("ellipse carries a paint transform");
        assert_point_approx(squeeze * Point::new(120.0, 112.0), (120.0, 64.0));
        assert_point_approx(squeeze * Point::new(192.0, 40.0), (192.0, 40.0));
    }

    /// `gradientTransform` is applied before the bounding-box scale: with
    /// `scale(0.5)` the unit square maps onto the first quarter of a 240x80
    /// box, so the circle stays at (0.5, 0.5) r=0.3 in gradient space while the
    /// paint transform scales it by (120, 40).
    #[test]
    fn radial_object_bbox_transform_scales_gradient_space() {
        let rg = radial(
            GradientUnits::ObjectBoundingBox,
            vec![TransformOp::Scale(0.5, 0.5)],
        );
        let (pos, paint_transform) = resolve(&rg).expect("gradient paints");
        assert_point_approx(pos.start_center, (0.5, 0.5));
        assert_point_approx(pos.end_center, (0.5, 0.5));
        assert!(pos.start_radius == 0.0 && (pos.end_radius - 0.3).abs() < 1e-6);
        let placement = paint_transform.expect("transformed gradient carries a paint transform");
        assert_point_approx(placement * Point::new(0.5, 0.5), (60.0, 20.0));
        assert_point_approx(placement * Point::new(0.8, 0.5), (96.0, 20.0));
        assert_point_approx(placement * Point::new(0.5, 0.8), (60.0, 32.0));
    }

    /// A rotation turns the circle into a tilted ellipse that no
    /// circle-plus-axis-aligned scale could express; the circle itself is left
    /// untouched and only the placement is rotated.
    #[test]
    fn radial_object_bbox_rotation_only_rotates_placement() {
        let rg = radial(
            GradientUnits::ObjectBoundingBox,
            vec![TransformOp::Rotate(90.0, 0.0, 0.0)],
        );
        let (pos, paint_transform) = resolve(&rg).expect("gradient paints");
        assert_point_approx(pos.end_center, (0.5, 0.5));
        let placement = paint_transform.expect("transformed gradient carries a paint transform");
        assert_point_approx(placement * Point::new(0.5, 0.5), (-120.0, 40.0));
    }

    /// `userSpaceOnUse`: the gradient stays in user space and the placement
    /// applies the transform first, then the viewBox scale and bbox origin.
    #[test]
    fn radial_user_space_transform_precedes_viewbox_scale() {
        let mut rg = radial(
            GradientUnits::UserSpaceOnUse,
            vec![TransformOp::Translate(10.0, 20.0)],
        );
        rg.cx = GradientLength::Number(100.0);
        rg.cy = GradientLength::Number(50.0);
        rg.r = GradientLength::Number(20.0);
        rg.fx = GradientLength::Number(100.0);
        rg.fy = GradientLength::Number(50.0);
        let bbox = Rect::new(300.0, 400.0, 700.0, 480.0);
        let (g, paint_transform) = radial_to_peniko(
            &rg,
            400.0,
            80.0,
            (2.0, 2.0),
            &bbox,
            ColorInterpolation::Srgb,
        )
        .expect("gradient paints");
        let placement = paint_transform.expect("transformed gradient carries a paint transform");
        let pos = radial_position(&g);
        assert_point_approx(pos.end_center, (100.0, 50.0));
        assert!((pos.end_radius - 20.0).abs() < 1e-5);
        // (100, 50) → +10,+20 → ×2 → −(300, 400)
        assert_point_approx(placement * Point::new(100.0, 50.0), (-80.0, -260.0));
    }

    /// A non-invertible `gradientTransform` has no gradient space to sample
    /// from and paints nothing.
    #[test]
    fn radial_non_invertible_transform_paints_nothing() {
        for transform in [
            vec![TransformOp::Scale(0.0, 1.0)],
            vec![TransformOp::Matrix([1.0, 0.0, 1.0, 0.0, 0.0, 0.0])],
        ] {
            let rg = radial(GradientUnits::ObjectBoundingBox, transform);
            assert!(resolve(&rg).is_none());
        }
    }
}
