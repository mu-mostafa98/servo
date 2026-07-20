/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree traversal.

use crate::render_tree::*;
use crate::renderer::Render;

pub fn render_svg_tree(
    tree: &SvgRenderTree,
    // TODO: svg_origin, svg_size, spatial_id, clip_chain_id, wr
) {
    // TODO: implement viewport clip + viewBox reference frame
    render_node(&tree.root);
    // TODO: pop viewBox reference frame
}

fn render_node(
    node: &SvgRenderNode,
    // TODO: svg_origin, spatial_id, clip_chain_id, wr, providers, parent_scale
) {
    eprintln!("▼ node enter: id={:?}", node.id);

    // TODO: skip if display:none
    // TODO: implement transforms (apply_node_transforms)
    // TODO: implement effects — clip-path, mask, filter (resolve_node_effects)

    emit_element(node);
    recurse_children(node);

    // TODO: pop transform reference frames

    eprintln!("▲ node exit:  id={:?}", node.id);
}

fn emit_element(
    node: &SvgRenderNode,
    // TODO: cur_origin, cur_spatial_id, clip_chain, accumulated_scale, wr, params
) {
    match &node.tag {
        SvgTag::Shape(shape) => shape.render(),
        SvgTag::Text(text) => text.render(),
        SvgTag::Image(img) => img.render(),
        SvgTag::Container(_) => {},
    }
}

fn recurse_children(
    node: &SvgRenderNode,
    // TODO: cur_origin, cur_spatial_id, clip_chain, providers, accumulated_scale, wr
) {
    if matches!(&node.tag, SvgTag::Container(Container::Defs | Container::Symbol)) {
        return;
    }
    for child in &node.children {
        render_node(child);
    }
}

// TODO: implement viewport (build_viewport_clip, push_viewbox_frame,
// compute_viewbox_transform), effects (resolve_node_effects,
// push_filter_context), transforms (apply_node_transforms),
// geometry/leaf rendering (emit_geometry, emit_shape, emit_leaf),
// providers (ResourceProviders, EffectParams)
