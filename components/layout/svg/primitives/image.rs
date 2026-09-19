/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `<image>` element: data-URI decode and aspect/align geometry.
//!
//! These are the parsing/math details [`crate::svg::usvg_builder::build_image`]
//! delegates to; the builder only assembles the resulting nodes.

use std::sync::Arc;

use data_url::DataUrl;
use html5ever::{LocalName, ns};
use image::ImageReader;
use layout_api::LayoutElement;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;

use crate::svg::primitives::attrs::{length_attr, length_attr_opt};
use crate::svg::primitives::geometry::aligned_pos;

/// Decodes an `<image>` element's `href` (or `xlink:href`) data URI into the raw
/// encoded bytes as an [`usvg::ImageKind`], together with the intrinsic pixel size.
///
/// External URLs are not yet supported: Servo's image pipeline only exposes
/// *decoded* pixels, not the raw encoded bytes usvg's `ImageKind::{PNG,JPEG,…}`
/// expects. The intrinsic size is a header-only probe; resvg decodes the bytes for
/// real at render time.
pub(crate) fn decode_image(
    element: &ServoLayoutElement<'_>,
) -> Option<(usvg::ImageKind, usvg::Size)> {
    let href = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))?;

    let data_url = DataUrl::process(href).ok()?;
    let mime = data_url.mime_type();
    if mime.type_ != "image" {
        return None;
    }

    let (bytes, _fragment) = data_url.decode_to_vec().ok()?;

    let (w, h) = ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())?;
    let size = usvg::Size::from_wh(w as f32, h as f32)?;

    let kind = match mime.subtype.as_str() {
        "png" => usvg::ImageKind::PNG(Arc::new(bytes)),
        "jpeg" | "jpg" => usvg::ImageKind::JPEG(Arc::new(bytes)),
        "gif" => usvg::ImageKind::GIF(Arc::new(bytes)),
        "webp" => usvg::ImageKind::WEBP(Arc::new(bytes)),
        _ => return None,
    };

    Some((kind, size))
}

/// Computes the `translate+scale` transform mapping an image's intrinsic size onto
/// its `x`/`y`/`width`/`height` box (honouring `preserveAspectRatio`), and returns
/// that box's bounding rect for effect resolution.
///
/// Geometry: `x`/`y` default to 0, `width`/`height` default to the intrinsic size,
/// and when only one of `width`/`height` is set the other preserves aspect ratio
/// (mirroring usvg's `parser::image::convert`).
pub(crate) fn image_geometry(
    element: &ServoLayoutElement<'_>,
    intrinsic: usvg::Size,
) -> Option<(usvg::Transform, usvg::NonZeroRect)> {
    let x = length_attr(element, "x", 0.0);
    let y = length_attr(element, "y", 0.0);
    let width_attr = length_attr_opt(element, "width");
    let height_attr = length_attr_opt(element, "height");
    let (width, height) = match (width_attr, height_attr) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, intrinsic.height() * (w / intrinsic.width())),
        (None, Some(h)) => (intrinsic.width() * (h / intrinsic.height()), h),
        (None, None) => (intrinsic.width(), intrinsic.height()),
    };

    let rect = usvg::NonZeroRect::from_xywh(x, y, width, height)?;

    // `preserveAspectRatio` (default `xMidYMid meet`) fits the intrinsic image into
    // the x/y/width/height box, scaling *uniformly* unless `none` is requested.
    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();
    let rect_size = rect.size();
    let aligned_size = if aspect.align == svgtypes::Align::None {
        rect_size
    } else if aspect.slice {
        intrinsic.expand_to(rect_size)
    } else {
        intrinsic.scale_to(rect_size)
    };
    let (aligned_x, aligned_y) = aligned_pos(
        aspect.align,
        rect.x(),
        rect.y(),
        rect.width() - aligned_size.width(),
        rect.height() - aligned_size.height(),
    );
    let view_box = aligned_size.to_non_zero_rect(aligned_x, aligned_y);

    // translate to the aligned origin, then scale the intrinsic size onto the aligned
    // box (resvg positions the image by transforming its intrinsic (0,0,w,h) rect).
    let translate_scale = usvg::Transform::from_row(
        view_box.width() / intrinsic.width(),
        0.0,
        0.0,
        view_box.height() / intrinsic.height(),
        view_box.x(),
        view_box.y(),
    );

    Some((translate_scale, rect))
}
