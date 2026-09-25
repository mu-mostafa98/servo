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

use crate::model::geometry::{PathCommand, PathData, Point};
use crate::model::element::shape::{Circle, Ellipse, Path, Polygon, Polyline, Rectangle, Shape};
use crate::model::style::FillRule;
use crate::model::document::ClipPathUnits;

/// Scale factor for objectBoundingBox clip-path coordinates (0..1 → 0..100).
pub(crate) const OBJECT_BBOX_REF_SIZE: f32 = 100.0;

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
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        match self {
            Shape::Rect(r) => r.clip_info(svg_origin, units),
            Shape::Circle(c) => c.clip_info(svg_origin, units),
            Shape::Ellipse(e) => e.clip_info(svg_origin, units),
            Shape::Polygon(p) => p.clip_info(svg_origin, units),
            Shape::Polyline(p) => p.clip_info(svg_origin, units),
            Shape::Path(p) => p.clip_info(svg_origin, units),
            Shape::Line(_) => None,
        }
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
                    Some(RoundedRect::new(x0, y0, x1, y1, RoundedRectRadii::from(radius)).to_path(0.1))
                } else {
                    Some(Rect::new(x0, y0, x1, y1).to_path(0.1))
                }
            },
            Shape::Circle(c) => {
                Some(Circle::new((c.cx.get() as f64, c.cy.get() as f64), c.r.get() as f64).to_path(0.1))
            },
            Shape::Ellipse(e) => {
                let (rx, ry) = e.resolved_radii()?;
                if rx.get() <= 0.0 || ry.get() <= 0.0 {
                    return None;
                }
                Some(Ellipse::new(
                    (e.cx.get() as f64, e.cy.get() as f64),
                    Vec2::new(rx.get() as f64, ry.get() as f64),
                    0.0,
                ).to_path(0.1))
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
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (cx, cy, r) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.cx.get() * OBJECT_BBOX_REF_SIZE,
                self.cy.get() * OBJECT_BBOX_REF_SIZE,
                self.r.get() * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (self.cx.get(), self.cy.get(), self.r.get())
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + cx - r, svg_origin.y + cy - r),
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
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (x, y, w, h) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.x.get() * OBJECT_BBOX_REF_SIZE,
                self.y.get() * OBJECT_BBOX_REF_SIZE,
                self.width.get() * OBJECT_BBOX_REF_SIZE,
                self.height.get() * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (
                self.x.get(),
                self.y.get(),
                self.width.get(),
                self.height.get(),
            )
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + x, svg_origin.y + y),
            LayoutSize::new(w, h),
        );
        let radii = match (self.rx, self.ry) {
            (Some(rx), _) if rx.get() > 0.0 => {
                let ry = self.ry.unwrap_or(rx);
                Some(all_equal_radius(
                    rx.get().clamp(0.0, w / 2.0),
                    ry.get().clamp(0.0, h / 2.0),
                ))
            },
            (_, Some(ry)) if ry.get() > 0.0 => Some(all_equal_radius(
                ry.get().clamp(0.0, h / 2.0),
                ry.get().clamp(0.0, h / 2.0),
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
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        let (rx_len, ry_len) = self.resolved_radii()?;
        let (cx, cy, rx, ry) = if units == ClipPathUnits::ObjectBoundingBox {
            (
                self.cx.get() * OBJECT_BBOX_REF_SIZE,
                self.cy.get() * OBJECT_BBOX_REF_SIZE,
                rx_len.get() * OBJECT_BBOX_REF_SIZE,
                ry_len.get() * OBJECT_BBOX_REF_SIZE,
            )
        } else {
            (self.cx.get(), self.cy.get(), rx_len.get(), ry_len.get())
        };
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(svg_origin.x + cx - rx, svg_origin.y + cy - ry),
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
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        if self.points.len() < 3 {
            return None;
        }
        Some(clip_path_geometry(
            &points_to_bez(&self.points, true),
            svg_origin,
            units,
        ))
    }
}

impl Polyline {
    /// Clip geometry for this polyline. A clip path fills the polyline as if
    /// it were closed (per SVG 2), so the mask path is closed for filling.
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        if self.points.len() < 3 {
            return None;
        }
        Some(clip_path_geometry(
            &points_to_bez(&self.points, true),
            svg_origin,
            units,
        ))
    }
}

impl Path {
    /// Clip geometry for this path (the exact path outline, curves preserved).
    pub(crate) fn clip_info(
        &self,
        svg_origin: &LayoutPoint,
        units: ClipPathUnits,
    ) -> Option<ClipGeometry> {
        if self.path.commands.is_empty() {
            return None;
        }
        Some(clip_path_geometry(
            &path_data_to_bez(&self.path),
            svg_origin,
            units,
        ))
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

/// Translate a local-space [`kurbo::BezPath`] into clip space: scale by the
/// objectBoundingBox reference size when `units == ObjectBoundingBox`, then
/// translate by the shape's origin in the current coordinate system.
pub(crate) fn transform_clip_path(
    path: &kurbo::BezPath,
    svg_origin: &LayoutPoint,
    units: ClipPathUnits,
) -> kurbo::BezPath {
    let mut transformed = path.clone();
    if units == ClipPathUnits::ObjectBoundingBox {
        transformed.apply_affine(kurbo::Affine::scale(OBJECT_BBOX_REF_SIZE as f64));
    }
    transformed.apply_affine(kurbo::Affine::translate((
        svg_origin.x as f64,
        svg_origin.y as f64,
    )));
    transformed
}

/// Build a [`ClipGeometry::Path`] from a local-space [`kurbo::BezPath`]:
/// transform it into clip space, derive its bounding box, and tag it with the
/// default (non-zero) fill rule.
pub(crate) fn clip_path_geometry(
    path: &kurbo::BezPath,
    svg_origin: &LayoutPoint,
    units: ClipPathUnits,
) -> ClipGeometry {
    let transformed = transform_clip_path(path, svg_origin, units);
    let bbox = transformed.bounding_box();
    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(bbox.x0 as f32, bbox.y0 as f32),
        LayoutSize::new((bbox.width() as f32).max(1.0), (bbox.height() as f32).max(1.0)),
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
