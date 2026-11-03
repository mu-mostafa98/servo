/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rendering half of the engine.
//!
//! Consumes the [`crate::model`] tree and emits WebRender display-list
//! commands. Kept private — the public entry point is [`crate::render_svg_tree`].

pub(crate) mod effects;
pub(crate) mod geometry;
pub(crate) mod hit_test;
pub(crate) mod renderer;
pub(crate) mod tessellator;
pub(crate) mod traversal;

use crate::model::element::shape::Shape;
use crate::model::element::{Container, SvgImage, SvgNode, SvgTag, TextSpan};
use crate::model::resource::ResourceKey;
use crate::model::style::NodeStyle;

/// A renderable leaf of a clip/mask definition subtree: a shape, a text run,
/// or an image. Used by clip/mask collection so that text glyphs and images
/// are included alongside shapes (unlike [`SvgNode::for_each_shape_leaf`],
/// which skips them).
#[derive(Clone, Copy)]
pub(crate) enum EffectLeaf<'a> {
    Shape(&'a Shape),
    Text(&'a TextSpan),
    Image(&'a SvgImage),
}

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

    /// Flatten container groups and invoke `f(leaf, style)` for every renderable
    /// leaf (shape, text run, image) under this node. `<defs>` is skipped.
    ///
    /// Unlike [`SvgNode::for_each_shape_leaf`], this also visits text and image
    /// leaves so clip-path/mask content can include glyphs and images.
    pub(crate) fn for_each_effect_leaf<F>(&self, f: &mut F)
    where
        F: for<'a> FnMut(EffectLeaf<'a>, &'a NodeStyle),
    {
        match &self.tag {
            SvgTag::Shape(shape) => f(EffectLeaf::Shape(shape), &self.style),
            SvgTag::Text(span) => f(EffectLeaf::Text(span), &self.style),
            SvgTag::Image(img) => f(EffectLeaf::Image(img), &self.style),
            SvgTag::Container(Container::Defs) => {},
            _ => {
                for child in &self.children {
                    child.for_each_effect_leaf(f);
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
