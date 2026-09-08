/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Software SVG render engine for Servo.
//!
//! Converts an [`SvgRenderTree`] (built from DOM in `layout::svg_builder`) into
//! WebRender display list commands via [`render_svg_tree`].
//!
//! # Architecture
//!
//! | Module | Role |
//! |--------|------|
//! | [`shapes`] | Pure data structs for SVG geometric shapes (rect, circle, etc.) |
//! | [`style`] | SVG property data types (fill, stroke, gradient, transform, …) |
//! | [`render_tree`] | [`SvgRenderTree`] node tree and definition types |
//! | [`error`] | Error types for SVG parsing failures |
//! | [`traversal`] | Recursive tree walk that produces the display list |
//! | [`renderer`] | Per-shape [`Render`] trait impls + fill/stroke/gradient pipelines |
//! | [`tessellator`] | Polygon triangulation + scanline rasterization |
//! | [`effects`] | Clip-path, mask, and filter resolution |
//!
//! The entry point is [`render_svg_tree`], called from
//! `layout::display_list::mod.rs`.  Shape construction happens in
//! `layout::svg_builder.rs`.

pub mod attr_parsers;
pub mod error;
pub mod image;
pub mod render_tree;
pub mod shapes;
pub mod style;
pub mod text;
pub mod visitor;

mod effects;
mod renderer;
mod tessellator;
mod traversal;

pub use render_tree::SvgTag;
pub use renderer::gradient::color_at_t_with_space;
pub use traversal::render_svg_tree;

pub use self::image::SvgImage;
pub use self::text::{DominantBaseline, ShapedGlyph, TextAnchor, TextSpan};

use webrender_api::units::{LayoutPoint, LayoutRect, LayoutSize};
use webrender_api::{ClipChainId, ExtendMode, GradientStop, SpatialId};

/// A CPU-rasterized image (e.g. from vello_cpu path rendering) ready to be
/// uploaded to WebRender and pushed as a single image display item.
#[derive(Debug, Clone)]
pub struct RasterizedImage {
    /// X position in layout space.
    pub x: f32,
    /// Y position in layout space.
    pub y: f32,
    /// Width of the pixel data, in device pixels.
    pub width: u32,
    /// Height of the pixel data, in device pixels.
    pub height: u32,
    /// Device scale factor used to rasterize the image (device pixel ratio).
    /// The on-screen size in layout space is `width / scale × height / scale`.
    pub scale: f32,
    /// RGBA pixel data.
    pub data: Vec<u8>,
    /// Content hash used to key the image cache.
    pub content_hash: u64,
}

/// A native WebRender gradient, fully resolved to absolute layout coordinates
/// and ready to be pushed via `create_gradient`/`create_radial_gradient` +
/// `push_gradient`/`push_radial_gradient`.
#[derive(Debug, Clone)]
pub enum GradientKind {
    Linear {
        start: LayoutPoint,
        end: LayoutPoint,
        stops: Vec<GradientStop>,
        extend_mode: ExtendMode,
    },
    Radial {
        center: LayoutPoint,
        radius: LayoutSize,
        stops: Vec<GradientStop>,
        extend_mode: ExtendMode,
    },
}

/// A native gradient display item emitted during traversal and replayed by the
/// layout layer in document order (alongside [`RasterizedImage`]s).
///
/// Native gradient items are deferred (like rasters) so they preserve paint
/// order against vello-rasterized shapes, but they carry their own `spatial_id`
/// and `clip_chain_id` so they still respect reference frames and rounded-rect
/// clips.
#[derive(Debug, Clone)]
pub struct GradientCmd {
    pub bounds: LayoutRect,
    pub spatial_id: SpatialId,
    pub clip_chain_id: ClipChainId,
    pub kind: GradientKind,
}

/// Deferred render output, replayed in document order by the layout layer.
///
/// `Raster` carries a CPU-rasterized bitmap; `Gradient` carries a native
/// gradient. Keeping both in one ordered list is what preserves z-order between
/// native gradient shapes and vello-rasterized shapes.
#[derive(Debug, Clone)]
pub enum RenderOutput {
    Raster(RasterizedImage),
    Gradient(GradientCmd),
}
