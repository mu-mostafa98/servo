/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::units::LayoutPoint;
use webrender_api::{ClipChainId, DisplayListBuilder, SpatialId};
use crate::render_tree::*;
use crate::renderer::{Render, RenderContext};

pub fn render_svg_tree(
    tree: &SvgRenderTree,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    render_node(&tree.root, svg_origin, spatial_id, clip_chain_id, wr);
}

fn render_node(
    node: &SvgRenderNode,
    svg_origin: &LayoutPoint,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    if !node.style.is_displayed() {
        return;
    }

    if let SvgTag::Shape(shape) = &node.tag {
        if node.style.is_visible() {
            let mut ctx = RenderContext {
                style: &node.style,
                svg_origin: *svg_origin,
                spatial_id,
                clip_chain_id,
                wr,
            };
            shape.render(&mut ctx);
        }
    }

    for child in &node.children {
        render_node(child, svg_origin, spatial_id, clip_chain_id, wr);
    }
}
