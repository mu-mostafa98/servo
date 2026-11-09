/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Fill-only SVG hit testing.
//!
//! Maps a point (in the root viewport's CSS-pixel space, relative to the SVG
//! element's top-left) back through the root/nested `viewBox` and per-node
//! transforms, then walks the tree topmost-first looking for the first shape
//! whose fill contains the point. Returns that shape's
//! [`SvgNode::source_node`] (an `OpaqueNode.0` address) so the layout layer can
//! turn it back into a DOM node for `:hover` / pointer-event dispatch.

use euclid::{Point2D, Transform2D};
use kurbo::Shape as _;
use webrender_api::units::{LayoutPoint, LayoutSize};

use crate::model::document::{SvgTree, ViewBox};
use crate::model::element::{Container, SvgNode, SvgTag};
use crate::render::renderer::transform::compute_transform_matrix;
use crate::render::traversal::compute_viewbox_transform;

/// Hit-test `point` (CSS px, relative to the root SVG viewport origin) against
/// `tree`, returning the first fill-hit node's [`SvgNode::source_node`].
pub fn hit_test_svg_tree(
    tree: &SvgTree,
    point: LayoutPoint,
    size: LayoutSize,
) -> Option<usize> {
    let point = Point2D::new(point.x, point.y);
    let point = apply_root_viewbox_inverse(tree, point, size)?;
    hit_test_node(&tree.root, point)
}

/// Map `point` from root viewport space back into the root `viewBox` user
/// space. With no `viewBox`, the two spaces coincide.
fn apply_root_viewbox_inverse(
    tree: &SvgTree,
    point: Point2D<f32, ()>,
    size: LayoutSize,
) -> Option<Point2D<f32, ()>> {
    let Some(vb) = &tree.viewport.view_box else {
        return Some(point);
    };
    let (sx, sy, ox, oy) = compute_viewbox_transform(
        vb.width.get(),
        vb.height.get(),
        size.width,
        size.height,
        tree.viewport.aspect_ratio.as_ref(),
    );
    Some(viewbox_to_viewport(vb, sx, sy, ox, oy).inverse()?.transform_point(point))
}

/// The `viewBox` → viewport transform used by the renderer:
/// `T(-min) · S(s) · T(o)` maps user space → viewport space.
fn viewbox_to_viewport(
    vb: &ViewBox,
    sx: f32,
    sy: f32,
    ox: f32,
    oy: f32,
) -> Transform2D<f32, (), ()> {
    let t1 = Transform2D::<f32, (), ()>::translation(-vb.min_x.get(), -vb.min_y.get());
    let s = Transform2D::<f32, (), ()>::scale(sx, sy);
    let t2 = Transform2D::<f32, (), ()>::translation(ox, oy);
    t1.then(&s).then(&t2)
}

/// Hit-test `point` (in the node's parent coordinate space) against `node`.
fn hit_test_node(node: &SvgNode, point: Point2D<f32, ()>) -> Option<usize> {
    if !node.style.is_displayed() {
        return None;
    }
    // `<defs>` and `<symbol>` content is only rendered when referenced via
    // `<use>`, never directly, so it is never a hit target.
    if matches!(
        &node.tag,
        SvgTag::Container(Container::Defs | Container::Symbol)
    ) {
        return None;
    }

    // Node's own transforms: point (parent space) → node local space.
    let local = compute_transform_matrix(&node.transforms)
        .inverse()?
        .transform_point(point);

    // A nested `<svg>` establishes a viewport: subtract its x/y, then apply
    // the inverse of its viewBox → viewport transform.
    let local = match &node.viewport {
        Some(vp) => {
            let p_viewport = Point2D::new(local.x - vp.x.get(), local.y - vp.y.get());
            match &vp.viewport.view_box {
                Some(vb) => {
                    let (sx, sy, ox, oy) = compute_viewbox_transform(
                        vb.width.get(),
                        vb.height.get(),
                        vp.viewport.width.get(),
                        vp.viewport.height.get(),
                        vp.viewport.aspect_ratio.as_ref(),
                    );
                    viewbox_to_viewport(vb, sx, sy, ox, oy)
                        .inverse()?
                        .transform_point(p_viewport)
                },
                None => p_viewport,
            }
        },
        None => local,
    };

    // Children paint on top of the parent's own content, so hit-test them
    // first, in reverse document order (topmost sibling first).
    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test_node(child, local) {
            return Some(hit);
        }
    }

    // The element itself. `visibility: hidden` and `pointer-events: none`
    // disable only this element (descendants were already tested above),
    // matching the renderer's per-element visibility check.
    if !node.style.is_visible() || node.style.pointer_events_none {
        return None;
    }

    // Fill-only hit testing: a shape with no fill is never a target.
    if node.style.fill.is_none() {
        return None;
    }

    match &node.tag {
        SvgTag::Shape(shape) => {
            let bez = shape.to_bez_path()?;
            if bez.contains(kurbo::Point::new(local.x as f64, local.y as f64)) {
                return node.source_node;
            }
        },
        SvgTag::Text(_) | SvgTag::Image(_) | SvgTag::Container(_) => {},
    }
    None
}
