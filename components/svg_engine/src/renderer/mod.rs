/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod fill;
pub(crate) mod gradient;
pub(crate) mod helpers;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod pattern;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod providers;
pub(crate) mod rect;
pub(crate) mod render_trait;
pub(crate) mod stroke;
pub(crate) mod transform;

pub(crate) use helpers::{
    ZERO_LENGTH_EPSILON, clip_chain_option, effective_stroke_width, make_common_props,
    paint_order_stroke_before_fill, shape_rendering_value, to_colorf,
};
pub(crate) use providers::{ClipMaskProvider, FilterProvider, PaintResourceProvider};
pub(crate) use render_trait::{Render, RenderContext};
