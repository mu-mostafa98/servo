/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `<image>` element.

use std::sync::Arc;

use data_url::DataUrl;
use html5ever::{LocalName, ns};
use image::ImageReader;
use layout_api::LayoutElement;
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use crate::svg::builder::SvgContext;
use crate::svg::effects::clip::{resolve_clip_path, ClipPathOutcome};
use crate::svg::effects::filter::{resolve_filter, FilterOutcome};
use crate::svg::effects::mask::{resolve_mask, MaskOutcome};
use crate::svg::primitives::attrs::{element_id, length_attr, length_attr_opt};
use crate::svg::primitives::geometry::aligned_pos;

/// Converts an `<image>` element into a [`usvg::Image`] node, reading the raster
/// data from the element's `href` (or `xlink:href`) data URI.
///
/// External URLs are not yet supported: Servo's image pipeline only exposes
/// *decoded* pixels, not the raw encoded bytes usvg's `ImageKind::{PNG,JPEG,…}`
/// expects. The image is always wrapped in an inner group carrying the position/scale
/// (align) transform, because resvg positions an image by transforming its intrinsic
/// (0,0,w,h) rect with the *accumulated group* transform (never `abs_transform`).
/// When the element has a clip-path/mask/filter/opacity, those go on an *outer*
/// group (transform = the element's own `transform` attribute), matching usvg's
/// parser, so a `userSpaceOnUse` clip isn't distorted by the align scale.
pub(crate) fn convert_image<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&ComputedValues>,
    ctx: &SvgContext<'a, 'dom>,
    parent_abs_transform: usvg::Transform,
) -> Vec<usvg::Node> {
    let Some(href) = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))
    else {
        return Vec::new();
    };

    let Ok(data_url) = DataUrl::process(href) else {
        // External URL (or malformed data URI): unsupported synchronously.
        return Vec::new();
    };
    let mime = data_url.mime_type();
    if mime.type_ != "image" {
        return Vec::new();
    }

    let Ok((bytes, _fragment)) = data_url.decode_to_vec() else {
        return Vec::new();
    };

    // Header-only probe of the intrinsic size; resvg decodes the bytes for real at
    // render time, so we hand it the raw encoded data, not pixels.
    let Some((w, h)) = ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
    else {
        return Vec::new();
    };
    let (intrinsic_w, intrinsic_h) = (w as f32, h as f32);

    let Some(size) = usvg::Size::from_wh(intrinsic_w, intrinsic_h) else {
        return Vec::new();
    };

    let kind = match mime.subtype.as_str() {
        "png" => usvg::ImageKind::PNG(Arc::new(bytes)),
        "jpeg" | "jpg" => usvg::ImageKind::JPEG(Arc::new(bytes)),
        "gif" => usvg::ImageKind::GIF(Arc::new(bytes)),
        "webp" => usvg::ImageKind::WEBP(Arc::new(bytes)),
        _ => return Vec::new(),
    };

    // Geometry: x/y default to 0, width/height default to the intrinsic size, and
    // when only one of width/height is set the other preserves aspect ratio
    // (mirroring usvg's `parser::image::convert`).
    let x = length_attr(element, "x", 0.0);
    let y = length_attr(element, "y", 0.0);
    let width_attr = length_attr_opt(element, "width");
    let height_attr = length_attr_opt(element, "height");
    let (width, height) = match (width_attr, height_attr) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, intrinsic_h * (w / intrinsic_w)),
        (None, Some(h)) => (intrinsic_w * (h / intrinsic_h), h),
        (None, None) => (intrinsic_w, intrinsic_h),
    };

    // `preserveAspectRatio` (default `xMidYMid meet`) fits the intrinsic image into
    // the x/y/width/height box, scaling *uniformly* unless `none` is requested. This
    // mirrors usvg's `parser::image::convert_inner` (fit_view_box + aligned_pos).
    let Some(rect) = usvg::NonZeroRect::from_xywh(x, y, width, height) else {
        return Vec::new();
    };
    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();
    let rect_size = rect.size();
    let aligned_size = if aspect.align == svgtypes::Align::None {
        rect_size
    } else if aspect.slice {
        size.expand_to(rect_size)
    } else {
        size.scale_to(rect_size)
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
    // box, with the element's own `transform` attribute applied on top (mirroring how
    // `build_shape_node` wraps a shape's `transform`).
    let translate_scale = usvg::Transform::from_row(
        view_box.width() / intrinsic_w,
        0.0,
        0.0,
        view_box.height() / intrinsic_h,
        view_box.x(),
        view_box.y(),
    );
    let element_transform = ctx.transform_attr(element);
    let image_ts = element_transform.pre_concat(translate_scale);
    let abs_transform = parent_abs_transform.pre_concat(image_ts);

    let id = element_id(element).unwrap_or_default();
    let visible = computed
        .map(|c| {
            !matches!(
                c.get_inherited_box().visibility,
                style::computed_values::visibility::T::Hidden |
                    style::computed_values::visibility::T::Collapse
            )
        })
        .unwrap_or(true);

    let Some(image) = usvg::Image::new(
        id,
        visible,
        size,
        usvg::ImageRendering::default(),
        kind,
        abs_transform,
    ) else {
        return Vec::new();
    };

    // Resolve clip-path/mask/filter against the image's bounding box, then wrap the
    // image (see the two-group structure below).
    let object_bbox = usvg::NonZeroRect::from_xywh(x, y, width, height);
    let element_opacity = computed.map(|c| c.get_effects().opacity).unwrap_or(1.0);
    let clip_path = match resolve_clip_path(element, ctx, object_bbox) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return Vec::new(),
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, ctx, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return Vec::new(),
        MaskOutcome::None => None,
    };
    let filter = match resolve_filter(element, ctx, object_bbox) {
        FilterOutcome::Filter(filter) => Some(filter),
        FilterOutcome::Invalid => return Vec::new(),
        FilterOutcome::None => None,
    };

    // The image is always wrapped in an inner group carrying the align transform
    // (x/y/width/height/preserveAspectRatio position+scale). clip-path/mask/filter
    // must NOT live on this group: resvg isolates the group that carries them and
    // applies the clip/mask in that group's shifted *local* coordinate system, so a
    // `userSpaceOnUse` clip authored in the element's user space would be scaled by
    // the align transform and clip the image away. Instead those properties go on an
    // *outer* group whose transform is the element's own `transform` attribute — the
    // same two-group structure usvg's parser produces (`image::convert_inner` wraps
    // the aligned group in a `convert_group` carrying the clip/mask/filter/opacity).
    let mut inner = usvg::Group::empty();
    inner.transform = translate_scale;
    inner.abs_transform = abs_transform;
    inner.push_child(usvg::Node::Image(Box::new(image)));

    if !element_transform.is_identity()
        || element_opacity < 1.0
        || clip_path.is_some()
        || mask.is_some()
        || filter.is_some()
    {
        let mut outer = usvg::Group::empty();
        outer.transform = element_transform;
        outer.abs_transform = parent_abs_transform.pre_concat(element_transform);
        outer.opacity = usvg::Opacity::new(element_opacity).unwrap_or(usvg::Opacity::ONE);
        outer.clip_path = clip_path;
        outer.mask = mask;
        if let Some(filter) = filter {
            outer.filters.push(filter);
        }
        outer.push_child(usvg::Node::Group(Box::new(inner)));
        vec![usvg::Node::Group(Box::new(outer))]
    } else {
        vec![usvg::Node::Group(Box::new(inner))]
    }
}
