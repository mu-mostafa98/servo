/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rasterizes a built [`usvg::Tree`] into WebRender image-cache pixels.

use std::hash::{Hash, Hasher};

use net_traits::image_cache::ImageCache;
use resvg::usvg;
use style::dom::OpaqueNode;
use webrender_api::ImageKey;
use webrender_api::units::DeviceIntSize;

/// Rasterizes `tree` at `raster_size`, applying the root `view_box`→viewport
/// transform (or, absent a viewBox, a non-uniform scale onto the natural size), and
/// uploads the result to WebRender's image cache.
pub(crate) fn rasterize_svg_tree(
    image_cache: &dyn ImageCache,
    tree: &usvg::Tree,
    node: OpaqueNode,
    raster_size: DeviceIntSize,
    view_box: Option<usvg::ViewBox>,
) -> Option<ImageKey> {
    const MAX_SVG_PIXMAP_DIMENSION: i32 = 5000;

    let width = raster_size.width.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let height = raster_size.height.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let img_size = usvg::Size::from_wh(width as f32, height as f32)?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

    let transform = match view_box {
        Some(vb) => vb.to_transform(img_size),
        None => {
            let natural_size = tree.size().to_int_size();
            if natural_size.width() == 0 || natural_size.height() == 0 {
                return None;
            }
            usvg::Transform::from_scale(
                width as f32 / natural_size.width() as f32,
                height as f32 / natural_size.height() as f32,
            )
        },
    };

    resvg::render(tree, transform, &mut pixmap.as_mut());
    let bytes = pixmap.take();

    // Key by (DOM node, raster size) so a re-layout of the same SVG after a mutation
    // updates the pixels in place rather than leaking a new WebRender image key.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.id().hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    let hash = hasher.finish();

    image_cache.upload_raw_pixels(hash, bytes, width, height);
    image_cache.raw_pixel_image_key(hash)
}
