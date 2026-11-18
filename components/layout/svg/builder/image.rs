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
use crate::svg::primitives::geometry::resolve_length_percentage;
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
    computed: Option<&style::properties::ComputedValues>,
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
    // `x`/`y` are SVG geometry properties in the CSS cascade (§8.8); read them
    // from the computed value when Stylo resolved the element, so CSS/inline-
    // style geometry works identically to the shape elements (which read x/y
    // from `computed.get_svg()`). Unstyled nodes fall back to the raw attribute.
    // `width`/`height` remain attribute-only (not in the CSS cascade), but
    // default to `auto` under SVG2 — see `read_size`/`resolve_image_size` below.
    let (x, y) = match computed {
        Some(cv) => {
            let svg = cv.get_svg();
            (
                resolve_length_percentage(&svg.clone_x(), vw),
                resolve_length_percentage(&svg.clone_y(), vh),
            )
        },
        None => (read_len("x", vw, 0.0), read_len("y", vh, 0.0)),
    };
    // `width`/`height` default to `auto` under SVG2 (SVG 1.1 defaulted to 0):
    // an absent/`auto`/invalid axis is derived from the referenced image's
    // intrinsic dimensions / aspect ratio. Parse them as `Option<f32>` where
    // `None` means `auto`; `parse_length_value` already returns `None` for
    // `auto` and unparseable values, which is exactly the fallback we want.
    let read_size = |name: &str, reference: f32| -> Option<f32> {
        get(name).and_then(|v| parse_length_value(&v, fs, reference))
    };
    let w_attr = read_size("width", vw);
    let h_attr = read_size("height", vh);

    let get_xlink = |name: &str| {
        element
            .attribute_as_str(&ns!(xlink), &LocalName::from(name))
            .map(|s| s.to_string())
    };
    let href = get("href").or_else(|| get_xlink("href"));

    // Resolve the referenced image *before* sizing so its intrinsic dimensions
    // are available to the `auto` derivation below. A None/empty href, a
    // pending load, or a decode failure all yield `None`, in which case the
    // renderer falls back to a placeholder and a reflow re-resolves once ready.
    let resolved_image = href.as_deref().and_then(|href_str| {
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
    });
    let natural: Option<(u32, u32)> = resolved_image.as_ref().map(|image| {
        let md = image.metadata();
        (md.width, md.height)
    });

    // Resolve the final width/height from the (possibly `auto`) axes.
    let (w, h) = resolve_image_size(w_attr, h_attr, natural)?;

    // Rasterize a vector image (SVG-in-SVG) at the resolved size, scaled to
    // device pixels; a raster image passes through with its existing key.
    let (image_key, natural_width, natural_height) = match resolved_image {
        Some(Image::Raster(raster)) => (
            raster.id.map(image_key_to_resource),
            Some(raster.metadata.width),
            Some(raster.metadata.height),
        ),
        Some(Image::Vector(vector_image)) => {
            let scale = context.style_context.device_pixel_ratio();
            let raster_size = webrender_api::units::DeviceIntSize::new(
                (w * scale.0).round().max(1.0) as i32,
                (h * scale.0).round().max(1.0) as i32,
            );
            context
                .image_resolver
                .rasterize_vector_image(
                    vector_image.id,
                    raster_size,
                    node.opaque(),
                    vector_image.svg_id,
                )
                .map(|raster| {
                    (
                        raster.id.map(image_key_to_resource),
                        Some(raster.metadata.width),
                        Some(raster.metadata.height),
                    )
                })
                .unwrap_or((None, None, None))
        },
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

/// Resolve `<image>` `width`/`height` under SVG2 `auto` sizing.
///
/// `None` means `auto`: the axis is derived from the intrinsic dimensions /
/// aspect ratio per the CSS Default Sizing Algorithm, with the SVG 300×150
/// default when the resource carries no intrinsic size. An explicit `0`
/// disables rendering and a negative value is an error — both yield `None`.
fn resolve_image_size(
    w_attr: Option<f32>,
    h_attr: Option<f32>,
    natural: Option<(u32, u32)>,
) -> Option<(f32, f32)> {
    let (nw, nh) = match natural {
        Some((w, h)) if w > 0 && h > 0 => (w as f32, h as f32),
        _ => (300.0, 150.0),
    };
    // A non-positive explicit length is invalid for `<image>`.
    let explicit = |v: f32| (v > 0.0).then_some(v);
    match (w_attr, h_attr) {
        (Some(w), Some(h)) => Some((explicit(w)?, explicit(h)?)),
        (Some(w), None) => {
            let w = explicit(w)?;
            Some((w, w * nh / nw))
        },
        (None, Some(h)) => {
            let h = explicit(h)?;
            Some((h * nw / nh, h))
        },
        (None, None) => Some((nw, nh)),
    }
}

/// Resolve an external image URL to a raster image key + natural pixel
/// dimensions. Used by `feImage`, whose primitive has no laid-out size at parse
/// time.
///
/// Raster images are returned as-is. Vector (`Image::Vector`) sources are
/// rasterized at their natural dimensions — falling back to the SVG 300×150
/// default when they carry no intrinsic size — scaled to device pixels; the
/// feImage image task then stretches the raster to fill the primitive subregion.
/// A still-pending rasterization yields `None` and the caller degrades to a
/// transparent placeholder until a reflow re-resolves it once loaded.
pub(super) fn resolve_raster_image_key(
    node: ServoLayoutNode,
    context: &LayoutContext,
    href: &str,
) -> Option<(Option<ResourceKey>, Option<u32>, Option<u32>)> {
    let base = node.base_url();
    let resolved = base.join(href.trim()).ok()?;
    let image = context
        .image_resolver
        .get_cached_image_for_url(
            node.opaque(),
            resolved,
            LayoutImageDestination::BoxTreeConstruction,
            InternalRequest::No,
        )
        .ok()?;
    match image {
        Image::Raster(raster) => Some((
            raster.id.map(image_key_to_resource),
            Some(raster.metadata.width),
            Some(raster.metadata.height),
        )),
        Image::Vector(vector_image) => {
            // feImage has no laid-out size at parse time, so rasterize the SVG
            // at its intrinsic dimensions (the SVG 300×150 default when it has
            // none), like a raster feImage. The image task stretches the raster
            // to the primitive subregion at render time.
            let natural = if vector_image.metadata.width > 0 &&
                vector_image.metadata.height > 0
            {
                (
                    vector_image.metadata.width as f32,
                    vector_image.metadata.height as f32,
                )
            } else {
                (300.0, 150.0)
            };
            let scale = context.style_context.device_pixel_ratio();
            let raster_size = webrender_api::units::DeviceIntSize::new(
                (natural.0 * scale.0).round().max(1.0) as i32,
                (natural.1 * scale.0).round().max(1.0) as i32,
            );
            context
                .image_resolver
                .rasterize_vector_image(
                    vector_image.id,
                    raster_size,
                    node.opaque(),
                    vector_image.svg_id,
                )
                .map(|raster| {
                    (
                        raster.id.map(image_key_to_resource),
                        Some(raster.metadata.width),
                        Some(raster.metadata.height),
                    )
                })
        },
    }
}

/// Convert a WebRender image key into the opaque model resource key.
fn image_key_to_resource(key: webrender_api::ImageKey) -> ResourceKey {
    ResourceKey {
        namespace: key.0.0,
        id: key.1,
    }
}
