use webrender_api::{
    BorderDetails, BorderRadius, BorderSide, BorderStyle, ClipChainId, ClipMode,
    CommonItemProperties, ComplexClipRegion, NormalBorder, PrimitiveFlags, PropertyBinding,
    ReferenceFrameKind, SpatialId, SpatialTreeItemKey, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSideOffsets, LayoutSize},
};

use crate::shapes::{Geometry, SvgRenderNode, SvgRenderTree, SvgTag};
use crate::styles::NodeStyles;

// ── Render state ─────────────────────────────────────────────────────

/// Accumulated rendering state pushed/popped during tree walk.
/// Phase 2+ will track transform, clip, mask here.
pub struct RenderState {}

impl Default for RenderState {
    fn default() -> Self {
        Self {}
    }
}

// ── Entry point ──────────────────────────────────────────────────────

/// Render an SVG render tree into the WebRender display list.
pub fn render_svg_element(
    tree: &SvgRenderTree,
    viewport_bounds: LayoutRect,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    let viewport_size = LayoutSize::new(viewport_bounds.width(), viewport_bounds.height());
    let origin = viewport_bounds.min;

    let mut render_state = RenderState::default();
    render_node(
        &tree.root,
        &mut render_state,
        &origin,
        viewport_size,
        spatial_id,
        parent_clip_chain_id,
        wr,
    );
}

// ── Tree walker ──────────────────────────────────────────────────────

/// Recursive tree walk — processes a node and its children depth-first.
fn render_node(
    node: &SvgRenderNode,
    render_state: &mut RenderState,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    // Phase 2+: apply node.styles.effects (transform, clip, mask) → push render_state

    match &node.tag {
        SvgTag::Shape(geom) => {
            render_shape(geom, &node.styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
        },
        // Container nodes, Defs, Text, etc. — no self-rendering in Phase 1
        _ => {},
    }

    for child in &node.children {
        render_node(child, render_state, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
    }

    // Phase 2+: revert render_state
}

// ── Shape dispatcher ─────────────────────────────────────────────────

fn render_shape(
    geom: &Geometry,
    styles: &NodeStyles,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    match geom {
        Geometry::Rect { .. } => {
            render_rect(geom, styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
        },
        Geometry::Circle { .. } => {
            render_circle(geom, styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
        },
        Geometry::Ellipse { .. } => {
            render_ellipse(geom, styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
        },
        Geometry::Line { .. } => {
            render_line(geom, styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
        },
        // Path, Polyline, Polygon — no native WR primitive, skip for Phase 1
        _ => {},
    }
}

// ── Geometry resolution ──────────────────────────────────────────────

struct ResolvedRect {
    x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32,
}

struct ResolvedCircle {
    cx: f32, cy: f32, r: f32,
}

struct ResolvedEllipse {
    cx: f32, cy: f32, rx: f32, ry: f32,
}

struct ResolvedLine {
    x1: f32, y1: f32, x2: f32, y2: f32,
}

fn resolve_len(v: &Option<crate::lengths::SvgLength>, ref_len: f32, default: f32) -> f32 {
    v.as_ref().map(|l| l.resolve(ref_len)).unwrap_or(default)
}

fn resolve_rect(geom: &Geometry, viewport: LayoutSize) -> Option<ResolvedRect> {
    if let Geometry::Rect { x, y, width, height, rx, ry } = geom {
        let w = resolve_len(width, viewport.width, viewport.width);
        let h = resolve_len(height, viewport.height, viewport.height);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(ResolvedRect {
            x: resolve_len(x, viewport.width, 0.0),
            y: resolve_len(y, viewport.height, 0.0),
            w, h,
            rx: resolve_len(rx, viewport.width, 0.0),
            ry: resolve_len(ry, viewport.height, 0.0),
        })
    } else {
        None
    }
}

fn resolve_circle(geom: &Geometry, viewport: LayoutSize) -> Option<ResolvedCircle> {
    if let Geometry::Circle { cx, cy, r } = geom {
        let r = resolve_len(r, viewport.width.min(viewport.height), 0.0);
        if r <= 0.0 {
            return None;
        }
        Some(ResolvedCircle {
            cx: resolve_len(cx, viewport.width, 0.0),
            cy: resolve_len(cy, viewport.height, 0.0),
            r,
        })
    } else {
        None
    }
}

fn resolve_ellipse(geom: &Geometry, viewport: LayoutSize) -> Option<ResolvedEllipse> {
    if let Geometry::Ellipse { cx, cy, rx, ry } = geom {
        let rx = resolve_len(rx, viewport.width, 0.0);
        let ry = resolve_len(ry, viewport.height, 0.0);
        if rx <= 0.0 || ry <= 0.0 {
            return None;
        }
        Some(ResolvedEllipse {
            cx: resolve_len(cx, viewport.width, 0.0),
            cy: resolve_len(cy, viewport.height, 0.0),
            rx, ry,
        })
    } else {
        None
    }
}

fn resolve_line(geom: &Geometry, viewport: LayoutSize) -> Option<ResolvedLine> {
    if let Geometry::Line { x1, y1, x2, y2 } = geom {
        Some(ResolvedLine {
            x1: resolve_len(x1, viewport.width, 0.0),
            y1: resolve_len(y1, viewport.height, 0.0),
            x2: resolve_len(x2, viewport.width, 0.0),
            y2: resolve_len(y2, viewport.height, 0.0),
        })
    } else {
        None
    }
}

// ── Render helpers ───────────────────────────────────────────────────

fn make_common(bounds: LayoutRect, spatial_id: SpatialId, clip_chain_id: ClipChainId) -> CommonItemProperties {
    CommonItemProperties {
        clip_rect: bounds,
        spatial_id,
        clip_chain_id,
        flags: PrimitiveFlags::IS_BACKFACE_VISIBLE,
    }
}

fn make_clip_chain(
    spatial_id: SpatialId,
    rect: LayoutRect,
    radii: BorderRadius,
    mode: ClipMode,
    parent_clip: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) -> ClipChainId {
    let clip_id = wr.define_clip_rounded_rect(
        spatial_id,
        ComplexClipRegion { rect, radii, mode },
    );
    let parent = if parent_clip == ClipChainId::INVALID {
        None
    } else {
        Some(parent_clip)
    };
    wr.define_clip_chain(parent, [clip_id])
}

// ── Shape renderers ──────────────────────────────────────────────────

fn render_rect(
    geom: &Geometry,
    styles: &NodeStyles,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    let Some(resolved) = resolve_rect(geom, viewport) else { return };

    let x = svg_origin.x + resolved.x;
    let y = svg_origin.y + resolved.y;
    let bounds = LayoutRect::from_origin_and_size(LayoutPoint::new(x, y), LayoutSize::new(resolved.w, resolved.h));
    let has_r = resolved.rx > 0.0 || resolved.ry > 0.0;

    // Fill
    if let (Some(fill_color), true) = (&styles.fill.color, styles.fill.opacity > 0.0) {
        let fill_chain = if has_r {
            let rx = resolved.rx.max(1.0);
            let ry = resolved.ry.max(1.0);
            let radii = BorderRadius::uniform_size(LayoutSize::new(rx, ry));
            make_clip_chain(spatial_id, bounds, radii, ClipMode::Clip, parent_clip_chain_id, wr)
        } else {
            parent_clip_chain_id
        };
        let mut fill_color = *fill_color;
        fill_color.a *= styles.fill.opacity;
        let common = make_common(bounds, spatial_id, fill_chain);
        wr.push_rect(&common, bounds, fill_color);
    }

    // Stroke via border
    if styles.stroke.width > 0.0 && styles.stroke.color.is_some() {
        let stroke_color = styles.stroke.color.unwrap();
        if stroke_color.a <= 0.0 || styles.stroke.opacity <= 0.0 {
            return;
        }
        let mut sc = stroke_color;
        sc.a *= styles.stroke.opacity;

        let border_side = BorderSide { color: sc, style: BorderStyle::Solid };
        let border_width = styles.stroke.width.max(0.001);
        let half_border = border_width / 2.0;

        // Expand bounds slightly so the border is centered on the rect edge
        let stroke_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x - half_border, y - half_border),
            LayoutSize::new(resolved.w + border_width, resolved.h + border_width),
        );

        let radius = if has_r {
            BorderRadius::uniform_size(LayoutSize::new(resolved.rx, resolved.ry))
        } else {
            BorderRadius::zero()
        };

        let common = make_common(stroke_bounds, spatial_id, parent_clip_chain_id);
        wr.push_border(
            &common,
            stroke_bounds,
            LayoutSideOffsets::new_all_same(border_width),
            BorderDetails::Normal(NormalBorder {
                left: border_side,
                right: border_side,
                top: border_side,
                bottom: border_side,
                radius,
                do_aa: true,
            }),
        );
    }
}

fn render_circle(
    geom: &Geometry,
    styles: &NodeStyles,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    // Circle IS an ellipse with equal radii — delegate to render_ellipse.
    if let Geometry::Circle { cx, cy, r } = geom {
        let ellipse_geom = Geometry::Ellipse {
            cx: *cx,
            cy: *cy,
            rx: *r,
            ry: *r,
        };
        render_ellipse(&ellipse_geom, styles, svg_origin, viewport, spatial_id, parent_clip_chain_id, wr);
    }
}

fn render_ellipse(
    geom: &Geometry,
    styles: &NodeStyles,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    let Some(resolved) = resolve_ellipse(geom, viewport) else { return };
    let center = LayoutPoint::new(svg_origin.x + resolved.cx, svg_origin.y + resolved.cy);
    let size = LayoutSize::new(resolved.rx * 2.0, resolved.ry * 2.0);
    let bounds = LayoutRect::from_origin_and_size(LayoutPoint::new(center.x - resolved.rx, center.y - resolved.ry), size);
    let radii = BorderRadius::uniform_size(LayoutSize::new(resolved.rx, resolved.ry));

    // Fill
    if let Some(fill_color) = &styles.fill.color {
        let mut fc = *fill_color;
        fc.a *= styles.fill.opacity;
        let clip_chain = make_clip_chain(spatial_id, bounds, radii, ClipMode::Clip, parent_clip_chain_id, wr);
        let common = make_common(bounds, spatial_id, clip_chain);
        wr.push_rect(&common, bounds, fc);
    }

    // Stroke via border
    if styles.stroke.width > 0.0 {
        if let Some(stroke_color) = &styles.stroke.color {
            let mut sc = *stroke_color;
            sc.a *= styles.stroke.opacity;
            if sc.a <= 0.0 {
                return;
            }

            let sw = styles.stroke.width.max(0.001);
            let outer_rx = resolved.rx + sw / 2.0;
            let outer_ry = resolved.ry + sw / 2.0;
            let outer_size = LayoutSize::new(outer_rx * 2.0, outer_ry * 2.0);
            let outer_bounds = LayoutRect::from_origin_and_size(
                LayoutPoint::new(center.x - outer_rx, center.y - outer_ry),
                outer_size,
            );

            let border_side = BorderSide { color: sc, style: BorderStyle::Solid };
            let common = make_common(outer_bounds, spatial_id, parent_clip_chain_id);
            wr.push_border(
                &common,
                outer_bounds,
                LayoutSideOffsets::new_all_same(sw),
                BorderDetails::Normal(NormalBorder {
                    left: border_side,
                    right: border_side,
                    top: border_side,
                    bottom: border_side,
                    radius: BorderRadius::uniform_size(LayoutSize::new(outer_rx, outer_ry)),
                    do_aa: true,
                }),
            );
        }
    }
}

fn render_line(
    geom: &Geometry,
    styles: &NodeStyles,
    svg_origin: &LayoutPoint,
    viewport: LayoutSize,
    spatial_id: SpatialId,
    parent_clip_chain_id: ClipChainId,
    wr: &mut webrender_api::DisplayListBuilder,
) {
    let Some(resolved) = resolve_line(geom, viewport) else { return };

    let Some(stroke_color) = &styles.stroke.color else { return };
    if styles.stroke.width <= 0.0 {
        return;
    }

    let mut sc = *stroke_color;
    sc.a *= styles.stroke.opacity;

    let x1 = svg_origin.x + resolved.x1;
    let y1 = svg_origin.y + resolved.y1;
    let x2 = svg_origin.x + resolved.x2;
    let y2 = svg_origin.y + resolved.y2;

    let half_w = (styles.stroke.width / 2.0).max(0.5);

    if (x1 - x2).abs() < 0.001 && (y1 - y2).abs() < 0.001 {
        return; // zero-length line
    }

    if (x1 - x2).abs() < 0.001 {
        // Vertical line — thin rect
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x1 - half_w, y1.min(y2)),
            LayoutSize::new(styles.stroke.width, (y1 - y2).abs()),
        );
        let common = make_common(bounds, spatial_id, parent_clip_chain_id);
        wr.push_rect(&common, bounds, sc);
    } else if (y1 - y2).abs() < 0.001 {
        // Horizontal line — thin rect
        let bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(x1.min(x2), y1 - half_w),
            LayoutSize::new((x1 - x2).abs(), styles.stroke.width),
        );
        let common = make_common(bounds, spatial_id, parent_clip_chain_id);
        wr.push_rect(&common, bounds, sc);
    } else {
        // Diagonal line — rotated rect via reference frame
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);
        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;

        use std::sync::atomic::{AtomicU64, Ordering};
        use webrender_api::euclid::{Angle, Transform3D};
        static SVG_LINE_KEY: AtomicU64 = AtomicU64::new(1);
        let key = SVG_LINE_KEY.fetch_add(1, Ordering::Relaxed);
        let transform = Transform3D::rotation(0.0, 0.0, 1.0, Angle::radians(angle));
        let line_spatial_id = wr.push_reference_frame(
            LayoutPoint::new(mx, my),
            spatial_id,
            TransformStyle::Flat,
            PropertyBinding::Value(transform),
            ReferenceFrameKind::Transform {
                is_2d_scale_translation: false,
                should_snap: false,
                paired_with_perspective: false,
            },
            SpatialTreeItemKey::new(0, key),
        );

        let line_bounds = LayoutRect::from_origin_and_size(
            LayoutPoint::new(-len / 2.0, -half_w),
            LayoutSize::new(len, styles.stroke.width),
        );
        let common = make_common(line_bounds, line_spatial_id, parent_clip_chain_id);
        wr.push_rect(&common, line_bounds, sc);
        wr.pop_reference_frame();
    }
}
