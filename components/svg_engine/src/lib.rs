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
use webrender_api::{
    AlphaType, ClipChainId, ColorF, CommonItemProperties, DisplayListBuilder, ExtendMode,
    GradientStop, ImageKey, ImageRendering, PrimitiveFlags, SpatialId,
};

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

/// Uploads CPU-rasterized RGBA pixels into the WebRender image cache and
/// returns the resulting [`ImageKey`].
///
/// Kept minimal (a single method) so [`crate`] does not depend on `net_traits`;
/// the layout layer adapts its `ImageCache` to this trait.
pub trait RasterImageUploader {
    /// Upload raw RGBA pixels keyed by `hash`; return the image key, or `None`
    /// if the upload produced no key.
    fn upload(&self, hash: u64, data: Vec<u8>, width: u32, height: u32) -> Option<ImageKey>;
}

/// Inline raster sink: pushes CPU-rasterized images directly into the display
/// list in document order, instead of deferring them for a later replay.
///
/// Rasters carry no spatial id — their geometry is baked into absolute layout
/// space — so they are pushed with the *outer* SVG element's `spatial_id` and
/// `clip_chain_id`, plus the fragment `clip_rect` and primitive `flags`, exactly
/// as the old replay path did via `common_properties`.
pub struct RasterSink<'a> {
    pub uploader: &'a dyn RasterImageUploader,
    /// Outer SVG element spatial id.
    pub spatial_id: SpatialId,
    /// Outer SVG element clip chain (clip-path/mask).
    pub clip_chain_id: ClipChainId,
    /// Fragment clip rect, mirroring `common_properties`.
    pub clip_rect: LayoutRect,
    /// Style primitive flags, mirroring `common_properties`.
    pub flags: PrimitiveFlags,
    /// Document origin (`svg_origin`); added to each raster's position.
    pub origin: LayoutPoint,
}

impl RasterSink<'_> {
    /// Upload `raster` and push it as a single image display item.
    pub(crate) fn emit(&self, wr: &mut DisplayListBuilder, raster: RasterizedImage) {
        let Some(key) = self.uploader.upload(
            raster.content_hash,
            raster.data,
            raster.width,
            raster.height,
        ) else {
            return;
        };
        let img_rect = LayoutRect::from_origin_and_size(
            LayoutPoint::new(self.origin.x + raster.x, self.origin.y + raster.y),
            LayoutSize::new(
                raster.width as f32 / raster.scale,
                raster.height as f32 / raster.scale,
            ),
        );
        let common = CommonItemProperties {
            clip_rect: self.clip_rect,
            spatial_id: self.spatial_id,
            clip_chain_id: self.clip_chain_id,
            flags: self.flags,
        };
        wr.push_image(
            &common,
            img_rect,
            ImageRendering::Auto,
            AlphaType::PremultipliedAlpha,
            key,
            ColorF::WHITE,
        );
    }
}
