/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use net_traits::image_cache::{ImageCache, RawPixelKey};
use resvg::usvg;
use uuid::Uuid;
use webrender_api::ImageKey;
use webrender_api::units::DeviceIntSize;

pub(crate) fn rasterize_svg_tree(
    image_cache: &dyn ImageCache,
    tree: &usvg::Tree,
    svg_id: Uuid,
    raster_size: DeviceIntSize,
) -> Option<ImageKey> {
    const MAX_SVG_PIXMAP_DIMENSION: i32 = 5000;

    let width = raster_size.width.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let height = raster_size.height.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;

    let key = RawPixelKey { svg_id, width, height };

    // Reuse the already-rasterized pixels when neither the SVG's contents nor its
    // size have changed since the last layout.
    if let Some(image_key) = image_cache.raw_pixel_image_key(key) {
        return Some(image_key);
    }

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

    let natural_size = tree.size().to_int_size();
    if natural_size.width() == 0 || natural_size.height() == 0 {
        return None;
    }
    let transform = usvg::Transform::from_scale(
        width as f32 / natural_size.width() as f32,
        height as f32 / natural_size.height() as f32,
    );

    resvg::render(tree, transform, &mut pixmap.as_mut());
    let bytes = pixmap.take();

    image_cache.upload_raw_pixels(key, bytes);
    image_cache.raw_pixel_image_key(key)
}
