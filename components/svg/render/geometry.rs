/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Render-space geometry: clip geometry resolution and shape → bez-path
//! conversion used by the WebRender / vello_cpu pipeline.
//!
//! Lives in the render half (not [`crate::model`]) because it produces
//! WebRender display-list types ([`LayoutRect`], [`BorderRadius`]) and
//! [`kurbo::BezPath`] rasterization geometry, not pure data.

use kurbo::Shape as _;
use webrender_api::BorderRadius;
use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};

use crate::model::document::ClipPathUnits;
use crate::model::element::shape::{Circle, Ellipse, Path, Polygon, Polyline, Rectangle, Shape};
use crate::model::geometry::{PathCommand, PathData, Point};
use crate::model::style::FillRule;

/// How clip/mask geometry maps from its local coordinates into the clipped
/// element's coordinate space.
///
/// `objectBoundingBox` coordinates are fractions (0..1) of the clipped
/// element's bounding box: each axis is scaled by that bbox's width/height and
/// offset by its origin (in the current spatial node's space, i.e. after the
/// `svg_origin` translation). `userSpaceOnUse` coordinates are used as-is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipSpace {
    pub sx: f32,
    pub sy: f32,
    pub ox: f32,
    pub oy: f32,
}

impl ClipSpace {
    /// Resolve the clip coordinate transform.
    ///
    /// `bbox` is the clipped/masked element's local-space bounding box, used to
    /// scale `objectBoundingBox` fractions. `None` (or a degenerate bbox) falls
    /// back to identity scale, which matches the SVG spec's "no geometry"
    /// behaviour: the clip is empty and the element is clipped away.
    pub(crate) fn new(
        svg_origin: LayoutPoint,
        units: ClipPathUnits,
        bbox: Option<LayoutRect>,
    ) -> Self {
        if units == ClipPathUnits::ObjectBoundingBox {
            if let Some(b) = bbox {
                if b.size().width > 0.0 && b.size().height > 0.0 {
                    return Self {
                        sx: b.size().width,
                        sy: b.size().height,
                        ox: svg_origin.x + b.min.x,
                        oy: svg_origin.y + b.min.y,
                    };
                }
            }
        }
        Self {
            sx: 1.0,
            sy: 1.0,
            ox: svg_origin.x,
            oy: svg_origin.y,
        }
    }

    /// The uniform scale for a length that cannot be resolved per-axis — the
    /// radius of a `<circle>` under `objectBoundingBox`, per the SVG 1.1 note:
    /// normalized to the length of the bbox diagonal (√((w²+h²)/2)).
    pub(crate) fn uniform_scale(self) -> f32 {
        ((self.sx * self.sx + self.sy * self.sy) / 2.0).sqrt()
    }
}

/// Clip geometry result — the resolved shape a clip-path/mask applies.
///
/// * [`ClipGeometry::RoundedRect`] and [`ClipGeometry::Rect`] map directly to
///   WebRender's native `define_clip_rounded_rect` / `define_clip_rect`.
/// * [`ClipGeometry::Path`] carries the actual [`kurbo::BezPath`] (already in
///   clip space) so the effect layer can rasterize it into an image-mask clip.
#[derive(Debug, Clone)]
pub(crate) enum ClipGeometry {
    RoundedRect {
        bounds: LayoutRect,
        radii: BorderRadius,
    },
    Rect {
        bounds: LayoutRect,
    },
    Path {
        bounds: LayoutRect,
        path: kurbo::BezPath,
        fill_rule: FillRule,
    },
}

/// A polygon/path clip that must be applied during vello_cpu rasterization.
///
/// WebRender 0.70's "quad" rendering path (used for `Rectangle`, `Image`, and
/// gradient primitives) panics when a clip chain contains an image-mask clip
/// (`bug: image-masks not expected on rect/quads`), so arbitrary polygon/path
/// clips cannot be expressed as WebRender clip items. Instead the traversal
/// carries the raw path geometry and applies it as a vello clip when the
/// clipped shape is rasterized.
#[derive(Debug, Clone)]
pub(crate) struct ComplexClip {
    /// The clip path in target user space (already translated by the node's
    /// `cur_origin`/`svg_origin`).
    pub path: kurbo::BezPath,
    pub fill_rule: FillRule,
}

// ======================= Shape → clip geometry / bez path =======================

impl Shape {
    /// Return clip geometry for this shape, if supported.
    ///
    /// Returns `None` for shapes that cannot participate in clip paths
    /// (line only — has no area).
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        match self {
            Shape::Rect(r) => r.clip_info(space),
            Shape::Circle(c) => c.clip_info(space),
            Shape::Ellipse(e) => e.clip_info(space),
            Shape::Polygon(p) => p.clip_info(space),
            Shape::Polyline(p) => p.clip_info(space),
            Shape::Path(p) => p.clip_info(space),
            Shape::Line(_) => None,
        }
    }

    /// Axis-aligned bounding box of the shape's fill geometry in local
    /// coordinates (no stroke expansion), used to resolve the SVG filter
    /// region (`objectBoundingBox` units). Returns a zero rect for shapes with
    /// no area (line) or degenerate geometry.
    pub(crate) fn local_bounds(&self) -> LayoutRect {
        let Some(bez) = self.to_bez_path() else {
            return LayoutRect::zero();
        };
        let b = bez.bounding_box();
        LayoutRect::from_origin_and_size(
            LayoutPoint::new(b.x0 as f32, b.y0 as f32),
            LayoutSize::new((b.x1 - b.x0) as f32, (b.y1 - b.y0) as f32),
        )
    }

    /// Convert the shape to a [`kurbo::BezPath`] in its local coordinate
    /// space, used for vello_cpu rasterization (gradient fills/strokes).
    pub(crate) fn to_bez_path(&self) -> Option<kurbo::BezPath> {
        use kurbo::{BezPath, Circle, Ellipse, Rect, RoundedRect, RoundedRectRadii, Vec2};

        match self {
            Shape::Rect(r) => {
                let x0 = r.x.get() as f64;
                let y0 = r.y.get() as f64;
                let x1 = (r.x.get() + r.width.get()) as f64;
                let y1 = (r.y.get() + r.height.get()) as f64;
                let (rx_len, ry_len) = r.resolved_radii();
                let rx = rx_len.get() as f64;
                let ry = ry_len.get() as f64;
                if rx > 0.0 || ry > 0.0 {
                    let radius = (rx + ry) / 2.0;
                    Some(
                        RoundedRect::new(x0, y0, x1, y1, RoundedRectRadii::from(radius))
                            .to_path(0.1),
                    )
                } else {
                    Some(Rect::new(x0, y0, x1, y1).to_path(0.1))
                }
            },
            Shape::Circle(c) => Some(
                Circle::new((c.cx.get() as f64, c.cy.get() as f64), c.r.get() as f64).to_path(0.1),
            ),
            Shape::Ellipse(e) => {
                let (rx, ry) = e.resolved_radii()?;
                if rx.get() <= 0.0 || ry.get() <= 0.0 {
                    return None;
                }
                Some(
                    Ellipse::new(
                        (e.cx.get() as f64, e.cy.get() as f64),
                        Vec2::new(rx.get() as f64, ry.get() as f64),
                        0.0,
                    )
                    .to_path(0.1),
                )
            },
            Shape::Line(l) => {
                let mut bez = BezPath::new();
                bez.move_to((l.x1.get() as f64, l.y1.get() as f64));
                bez.line_to((l.x2.get() as f64, l.y2.get() as f64));
                Some(bez)
            },
            Shape::Polyline(p) => (p.points.len() >= 2).then(|| points_to_bez(&p.points, false)),
            Shape::Polygon(p) => (p.points.len() >= 3).then(|| points_to_bez(&p.points, true)),
            Shape::Path(p) => Some(path_data_to_bez(&p.path)),
        }
    }
}

impl Circle {
    /// Clip geometry for this circle.
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        let cx = self.cx.get() * space.sx + space.ox;
        let cy = self.cy.get() * space.sy + space.oy;
        let r = self.r.get() * space.uniform_scale();
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(cx - r, cy - r),
            LayoutSize::new(r * 2.0, r * 2.0),
        );
        Some(ClipGeometry::RoundedRect {
            bounds,
            radii: all_equal_radius(r, r),
        })
    }
}

impl Rectangle {
    /// Clip geometry for this rectangle.
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        let x = self.x.get() * space.sx + space.ox;
        let y = self.y.get() * space.sy + space.oy;
        let w = self.width.get() * space.sx;
        let h = self.height.get() * space.sy;
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x, y),
            LayoutSize::new(w, h),
        );
        let radii = match (self.rx, self.ry) {
            (Some(rx), _) if rx.get() > 0.0 => {
                let ry = self.ry.unwrap_or(rx);
                Some(all_equal_radius(
                    (rx.get() * space.sx).clamp(0.0, w / 2.0),
                    (ry.get() * space.sy).clamp(0.0, h / 2.0),
                ))
            },
            (_, Some(ry)) if ry.get() > 0.0 => Some(all_equal_radius(
                (ry.get() * space.sy).clamp(0.0, h / 2.0),
                (ry.get() * space.sy).clamp(0.0, h / 2.0),
            )),
            _ => None,
        };
        Some(match radii {
            Some(r) => ClipGeometry::RoundedRect { bounds, radii: r },
            None => ClipGeometry::Rect { bounds },
        })
    }
}

impl Ellipse {
    /// Clip geometry for this ellipse.
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        let (rx_len, ry_len) = self.resolved_radii()?;
        let cx = self.cx.get() * space.sx + space.ox;
        let cy = self.cy.get() * space.sy + space.oy;
        let rx = rx_len.get() * space.sx;
        let ry = ry_len.get() * space.sy;
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(cx - rx, cy - ry),
            LayoutSize::new(rx * 2.0, ry * 2.0),
        );
        Some(ClipGeometry::RoundedRect {
            bounds,
            radii: all_equal_radius(rx, ry),
        })
    }
}

impl Polygon {
    /// Clip geometry for this polygon (closed fill path around all points).
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        if self.points.len() < 3 {
            return None;
        }
        Some(clip_path_geometry(&points_to_bez(&self.points, true), space))
    }
}

impl Polyline {
    /// Clip geometry for this polyline. A clip path fills the polyline as if
    /// it were closed (per SVG 2), so the mask path is closed for filling.
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        if self.points.len() < 3 {
            return None;
        }
        Some(clip_path_geometry(&points_to_bez(&self.points, true), space))
    }
}

impl Path {
    /// Clip geometry for this path (the exact path outline, curves preserved).
    pub(crate) fn clip_info(&self, space: ClipSpace) -> Option<ClipGeometry> {
        if self.path.commands.is_empty() {
            return None;
        }
        Some(clip_path_geometry(&path_data_to_bez(&self.path), space))
    }
}

// ======================= Clip geometry helpers =======================

/// Build an open or closed [`kurbo::BezPath`] from a list of model [`Point`]s.
pub(crate) fn points_to_bez(points: &[Point], close: bool) -> kurbo::BezPath {
    let mut bez = kurbo::BezPath::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 {
            bez.move_to((p.x as f64, p.y as f64));
        } else {
            bez.line_to((p.x as f64, p.y as f64));
        }
    }
    // Closing an empty path (no preceding `MoveTo`) panics in kurbo. An empty
    // point list is valid SVG that renders nothing, so skip the close.
    if close && !points.is_empty() {
        bez.close_path();
    }
    bez
}

/// Convert a model [`PathData`] back into a [`kurbo::BezPath`] for
/// rasterization. This is the render half of the layout → model → render path
/// boundary: the layout layer produces [`PathData`] (from
/// `kurbo::BezPath::from_svg`) and here we round-trip it back into kurbo space.
pub(crate) fn path_data_to_bez(path: &PathData) -> kurbo::BezPath {
    let mut bez = kurbo::BezPath::new();
    for cmd in &path.commands {
        match cmd {
            PathCommand::MoveTo(p) => bez.move_to((p.x as f64, p.y as f64)),
            PathCommand::LineTo(p) => bez.line_to((p.x as f64, p.y as f64)),
            PathCommand::QuadTo(c, p) => {
                bez.quad_to((c.x as f64, c.y as f64), (p.x as f64, p.y as f64))
            },
            PathCommand::CurveTo(c1, c2, p) => bez.curve_to(
                (c1.x as f64, c1.y as f64),
                (c2.x as f64, c2.y as f64),
                (p.x as f64, p.y as f64),
            ),
            PathCommand::Close => bez.close_path(),
        }
    }
    bez
}

/// Translate a local-space [`kurbo::BezPath`] into clip space via [`ClipSpace`]:
/// per-axis scale (objectBoundingBox fractions, or identity) then translate by
/// the shape's origin in the current coordinate system.
pub(crate) fn transform_clip_path(path: &kurbo::BezPath, space: ClipSpace) -> kurbo::BezPath {
    let mut transformed = path.clone();
    transformed.apply_affine(kurbo::Affine::scale_non_uniform(
        space.sx as f64,
        space.sy as f64,
    ));
    transformed.apply_affine(kurbo::Affine::translate((
        space.ox as f64,
        space.oy as f64,
    )));
    transformed
}

/// Build a [`ClipGeometry::Path`] from a local-space [`kurbo::BezPath`]:
/// transform it into clip space, derive its bounding box, and tag it with the
/// default (non-zero) fill rule.
pub(crate) fn clip_path_geometry(path: &kurbo::BezPath, space: ClipSpace) -> ClipGeometry {
    let transformed = transform_clip_path(path, space);
    let bbox = transformed.bounding_box();
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(bbox.x0 as f32, bbox.y0 as f32),
        LayoutSize::new(
            (bbox.width() as f32).max(1.0),
            (bbox.height() as f32).max(1.0),
        ),
    );
    ClipGeometry::Path {
        bounds,
        path: transformed,
        fill_rule: FillRule::NonZero,
    }
}

/// Build a BorderRadius with the same (rx, ry) on all four corners.
pub(crate) fn all_equal_radius(rx: f32, ry: f32) -> BorderRadius {
    BorderRadius {
        top_left: LayoutSize::new(rx, ry),
        top_right: LayoutSize::new(rx, ry),
        bottom_left: LayoutSize::new(rx, ry),
        bottom_right: LayoutSize::new(rx, ry),
    }
}
