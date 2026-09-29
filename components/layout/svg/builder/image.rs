/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<image>` element assembly — resolves `href`/`xlink:href` to a WebRender
//! [`ImageKey`] through the layout image cache and builds an [`SvgImage`].

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutImageDestination, LayoutNode};
use net_traits::image_cache::Image;
use net_traits::request::InternalRequest;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_svg::element::SvgImage;
use servo_svg::resource::ResourceKey;
use web_atoms::ns;

use crate::context::LayoutContext;
use crate::svg::primitives::attrs::{get_attr, parse_length_value};
use crate::svg::primitives::viewport::parse_aspect_ratio;

/// Build an [`SvgImage`] from element attributes.
///
/// Resolves the `href`/`xlink:href` attribute to a WebRender [`ImageKey`] via
/// the layout image cache: the URL is resolved against the owner document's
/// base URL, then looked up (or requested) through `image_resolver`. When the
/// image is not yet loaded the key is `None` and the renderer draws a
/// placeholder; once it loads, a reflow re-runs this and yields `Some(key)`.
pub(super) fn build_image_tag(
    element: &ServoLayoutElement,
    node: ServoLayoutNode,
    context: &LayoutContext,
    vw: f32,
    vh: f32,
) -> Option<SvgImage> {
    let fs = 16.0;
    let get = |name: &str| get_attr(element, name);
    // `<image>` x/width percentages resolve against the viewport width, y/height
    // against the viewport height (§8.8).
    let read_len = |name: &str, reference: f32, default: f32| -> f32 {
        get(name)
            .and_then(|v| parse_length_value(&v, fs, reference))
            .unwrap_or(default)
    };
    let x = read_len("x", vw, 0.0);
    let y = read_len("y", vh, 0.0);
    let w = read_len("width", vw, 0.0).max(0.0);
    let h = read_len("height", vh, 0.0).max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let get_xlink = |name: &str| {
        element.attribute_as_str(&ns!(xlink), &LocalName::from(name)).map(|s| s.to_string())
    };
    let href = get("href").or_else(|| get_xlink("href"));
    // Resolve href → ImageKey + natural dimensions. Relative URLs are resolved
    // against the owner document's base URL; data: URIs parse directly.  A
    // None/empty href, a pending load, or a decode failure all yield
    // `image_key = None`, in which case the renderer falls back to a
    // placeholder.
    let raster_data: Option<(Option<webrender_api::ImageKey>, u32, u32)> =
        href.as_deref().and_then(|href_str| {
            let base = node.base_url();
            let resolved = base.join(href_str.trim()).ok()?;
            context
                .image_resolver
                .get_cached_image_for_url(
                    node.opaque(),
                    resolved,
                    LayoutImageDestination::BoxTreeConstruction,
                    InternalRequest::No,
                )
                .ok()
                .and_then(|image| match image {
                    Image::Raster(raster) => {
                        Some((raster.id, raster.metadata.width, raster.metadata.height))
                    },
                    Image::Vector(..) => None, // vector images need rasterization; not handled here
                })
        });
    let (image_key, natural_width, natural_height) = match raster_data {
        Some((id, w, h)) => (id.map(image_key_to_resource), Some(w), Some(h)),
        None => (None, None, None),
    };

    // Parse preserveAspectRatio — defaults to xMidYMid meet per SVG spec.
    let preserve_aspect_ratio = get("preserveAspectRatio")
        .map(|v| parse_aspect_ratio(&v))
        .unwrap_or_default();

    Some(SvgImage {
        x,
        y,
        width: w,
        height: h,
        href,
        image_key,
        natural_width,
        natural_height,
        preserve_aspect_ratio,
    })
}

/// Convert a WebRender image key into the opaque model resource key.
fn image_key_to_resource(key: webrender_api::ImageKey) -> ResourceKey {
    ResourceKey {
        namespace: key.0.0,
        id: key.1,
    }
}
