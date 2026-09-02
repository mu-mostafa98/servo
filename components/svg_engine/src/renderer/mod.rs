/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape renderers — convert SVG shapes into WebRender display list commands.
//!
//! Each shape in [`crate::shapes`] implements the [`Render`] trait, which
//! produces the corresponding [`webrender_api::DisplayListBuilder`] commands.
//! The [`crate::traversal`] module calls [`Render::render`] during SVG tree
//! traversal — there is no central dispatch match to maintain.
//!
//! # Module Map
//!
//! | Module | Role |
//! |--------|------|
//! | [`render_trait`] | [`Render`] trait, [`RenderContext`], Shape dispatch |
//! | [`providers`] | [`PaintResourceProvider`], [`ClipMaskProvider`], [`FilterProvider`] |
//! | [`helpers`] | Color conversion, clip chain utilities, hint resolution |
//! | `circle`, `ellipse`, … | Per-shape [`Render`] implementations |

pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod fill;
pub(crate) mod gradient;
pub(crate) mod helpers;
pub(crate) mod image;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod pattern;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod providers;
pub(crate) mod rect;
pub(crate) mod render_trait;
pub(crate) mod stroke;
pub(crate) mod text;
pub(crate) mod transform;

// Re-export the public API so existing imports stay working.
pub(crate) use helpers::{
    ZERO_LENGTH_EPSILON, clip_chain_option, effective_stroke_width, make_common_props,
    paint_order_stroke_before_fill, shape_rendering_value, to_colorf,
};
pub(crate) use providers::{ClipMaskProvider, FilterProvider, MarkerProvider, PaintResourceProvider};
pub(crate) use render_trait::{Render, RenderContext};
