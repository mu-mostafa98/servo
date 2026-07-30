/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.
//!
//! Walks a [`usvg::Tree`] recursively, applying transforms at each group,
//! and dispatching simple shapes to the renderer.

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{
    ClipChainId, DisplayListBuilder, PropertyBinding,
    ReferenceFrameKind, SpatialId, TransformStyle,
};

use crate::renderer::Render;

// ======================= Public Entry Point =======================

/// Render an SVG tree into a WebRender display list.
pub fn render_svg_tree(
    tree: &usvg::Tree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Clip to viewport bounds.
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let parent = (clip_chain_id != ClipChainId::INVALID).then_some(clip_chain_id);
    let svg_clip_chain = wr.define_clip_chain(parent, [svg_clip_id]);

    render_group(
        tree.root(),
        svg_origin,
        spatial_id,
        svg_clip_chain,
        wr,
    );
}

// ======================= Group Traversal =======================

fn render_group(
    group: &usvg::Group,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let (cur_origin, cur_spatial_id, pushed) =
        push_transform(group.transform(), svg_origin, spatial_id, wr);

    for child in group.children() {
        render_node(child, &cur_origin, cur_spatial_id, clip_chain_id, wr);
    }

    if pushed {
        wr.pop_reference_frame();
    }
}

fn push_transform(
    transform: usvg::Transform,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    wr: &mut DisplayListBuilder,
) -> (LayoutPoint, SpatialId, bool) {
    if transform.is_identity() {
        return (*svg_origin, spatial_id, false);
    }

    let lt = webrender_api::units::LayoutTransform::new(
        transform.sx, transform.ky, 0.0, 0.0,
        transform.kx, transform.sy, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        transform.tx + svg_origin.x, transform.ty + svg_origin.y, 0.0, 1.0,
    );

    let child_spatial_id = wr.push_reference_frame(
        LayoutPoint::zero(),
        spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(lt),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    );

    (LayoutPoint::zero(), child_spatial_id, true)
}

// ======================= Node Dispatch =======================

fn render_node(
    node: &usvg::Node,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    match node {
        usvg::Node::Group(group) => {
            render_group(group, svg_origin, spatial_id, clip_chain_id, wr);
        }
        usvg::Node::SimpleShape(shape) => {
            let mut ctx = crate::renderer::RenderContext {
                svg_origin: *svg_origin,
                spatial_id,
                clip_chain_id,
                wr,
                accumulated_scale: 1.0,
            };
            shape.render(&mut ctx);
        }
        usvg::Node::Path(_) => {}
        usvg::Node::Image(_) => {}
        usvg::Node::Text(_) => {}
    }
}
