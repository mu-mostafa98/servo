/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal — walks the [`SvgRenderTree`] and emits
//! WebRender display list commands via each shape's [`Render`] impl.
//!
//! The entry point is [`render_svg_tree`], which sets up the viewport clip
//! and viewBox transform, then recursively walks the node tree.

use std::collections::HashMap;

use euclid::Transform2D;
use webrender_api::{
    DisplayListBuilder, ClipChainId, SpatialId,
    PropertyBinding, ReferenceFrameKind, TransformStyle,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::render_tree::*;
use crate::renderer::transform;
use crate::renderer::{Render, RenderContext};
use crate::style::gradient::GradientDef;

/// Render an SVG render tree into the WebRender display list.
pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    svg_size: LayoutSize,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    // Apply SVG viewport clip.
    let svg_bounds = LayoutRect::from_origin_and_size(*svg_origin, svg_size);
    let svg_clip_id = wr.define_clip_rect(spatial_id, svg_bounds);
    let svg_clip_chain = wr.define_clip_chain(
        if clip_chain_id == ClipChainId::INVALID {
            None
        } else {
            Some(clip_chain_id)
        },
        [svg_clip_id],
    );

    // Apply viewBox transform if present.
    let (root_origin, root_spatial_id, pop_frame) =
        if let Some(ref vb) = tree.viewport.view_box {
            let scale_x = svg_size.width / vb.width;
            let scale_y = svg_size.height / vb.height;
            let t1: Transform2D<f32, (), ()> = Transform2D::translation(-vb.min_x, -vb.min_y);
            let s: Transform2D<f32, (), ()> = Transform2D::scale(scale_x, scale_y);
            let combined = t1.then(&s);
            let lt = transform::to_layout_transform(&combined);
            let frame_id = wr.push_reference_frame(
                *svg_origin,
                spatial_id,
                TransformStyle::Flat,
                PropertyBinding::Value(lt),
                ReferenceFrameKind::Transform {
                    is_2d_scale_translation: false,
                    should_snap: false,
                    paired_with_perspective: false,
                },
            );
            (LayoutPoint::new(0.0, 0.0), frame_id, true)
        } else {
            (*svg_origin, spatial_id, false)
        };

    render_node(&tree.root, &root_origin, root_spatial_id, svg_clip_chain, wr, &tree.gradients);

    if pop_frame {
        wr.pop_reference_frame();
    }
}

/// Recursively render a single node and its children.
fn render_node(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
    gradients: &HashMap<String, GradientDef>,
) {
    let mut cur_spatial_id = spatial_id;
    let mut cur_origin = *svg_origin;
    let mut pushed_count: u32 = 0;

    // Apply each transform operation in order.
    for op in &node.style.transform {
        let result = transform::apply_transform_op(op, cur_origin, cur_spatial_id, wr);
        cur_origin = result.child_origin;
        cur_spatial_id = result.child_spatial_id;
        if result.pushed_frame {
            pushed_count += 1;
        }
    }

    // Render this node if it's a shape.
    if let SvgTag::Shape(shape) = &node.tag {
        let mut ctx = RenderContext {
            style: &node.style,
            svg_origin: cur_origin,
            spatial_id: cur_spatial_id,
            clip_chain_id,
            wr: &mut *wr,
            gradients,
        };
        shape.render(&mut ctx);
    }

    // Recurse into children, unless this is a <defs> container whose
    // children are definitions and must not be rendered directly.
    if let SvgTag::Container(Container::Defs) = &node.tag {
        // do not recurse — <defs> children are non-visual definitions
    } else {
        for child in &node.children {
            render_node(child, &cur_origin, cur_spatial_id, clip_chain_id, wr, gradients);
        }
    }

    // Pop any reference frames in reverse order.
    for _ in 0..pushed_count {
        wr.pop_reference_frame();
    }
}
