/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering half of the engine.
//!
//! Consumes the [`crate::model`] tree and emits WebRender display-list
//! commands. Kept private — the public entry point is [`crate::render_svg_tree`].

pub(crate) mod effects;
pub(crate) mod geometry;
pub(crate) mod renderer;
pub(crate) mod tessellator;
pub(crate) mod traversal;

use crate::model::resource::ResourceKey;
use crate::model::shapes::Shape;
use crate::model::style::NodeStyle;
use crate::model::element::{Container, SvgNode, SvgTag};

impl SvgNode {
    /// Flatten container groups and invoke `f(shape, style)` for every shape
    /// leaf under this node. Non-shape leaves (text, image, …) are skipped.
    ///
    /// Used by clip/mask collection and pattern/marker content rendering so
    /// that nested `<g>`/`<use>`/`<symbol>` wrappers inside a definition are
    /// honoured instead of being silently dropped.
    pub(crate) fn for_each_shape_leaf<F>(&self, f: &mut F)
    where
        F: FnMut(&Shape, &NodeStyle),
    {
        match &self.tag {
            SvgTag::Shape(shape) => f(shape, &self.style),
            // `<defs>` content is never rendered directly — skip it.
            SvgTag::Container(Container::Defs) => {},
            _ => {
                for child in &self.children {
                    child.for_each_shape_leaf(f);
                }
            },
        }
    }
}

/// Reconstruct the WebRender image key from an opaque model [`ResourceKey`].
pub(crate) fn to_wr_image_key(key: ResourceKey) -> webrender_api::ImageKey {
    webrender_api::ImageKey(webrender_api::IdNamespace(key.namespace), key.id)
}

/// Reconstruct the WebRender font instance key from an opaque model
/// [`ResourceKey`].
pub(crate) fn to_wr_font_key(key: ResourceKey) -> webrender_api::FontInstanceKey {
    webrender_api::FontInstanceKey(webrender_api::IdNamespace(key.namespace), key.id)
}
