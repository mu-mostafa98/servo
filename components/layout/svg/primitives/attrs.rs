/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Attribute/value parsing helpers shared across the SVG converters.

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNodeType};
use resvg::usvg;
use script::layout_dom::ServoLayoutElement;
use style::values::computed::{LengthPercentage, NonNegativeLengthPercentageOrAuto};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use svgtypes::{LengthUnit, TransformListParser, TransformListToken};

/// The `id` attribute of `element`, when non-empty.
pub(crate) fn element_id(element: &ServoLayoutElement<'_>) -> Option<String> {
    let id = element.attribute_as_str(&ns!(), &LocalName::from("id"))?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Returns the [`LayoutElementType`] for `element`, the layout-standard way to
/// discriminate element kinds (as opposed to matching the tag name). Falls back to
/// the generic [`LayoutElementType::Element`] for pseudo-elements, which have no
/// type id.
pub(crate) fn element_layout_type(element: &ServoLayoutElement<'_>) -> LayoutElementType {
    match element.type_id() {
        Some(LayoutNodeType::Element(ty)) => ty,
        _ => LayoutElementType::Element,
    }
}

/// Whether an element explicitly sets its own paint, in which case it should not
/// inherit fill/stroke from an enclosing `<use>` host.
pub(crate) fn element_has_explicit_fill(element: &ServoLayoutElement<'_>) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("fill"))
        .is_some() ||
        element
            .attribute_as_str(&ns!(), &LocalName::from("style"))
            .is_some()
}

/// Whether an element explicitly sets its own stroke, in which case it should not
/// inherit fill/stroke from an enclosing `<use>` host.
pub(crate) fn element_has_explicit_stroke(element: &ServoLayoutElement<'_>) -> bool {
    element
        .attribute_as_str(&ns!(), &LocalName::from("stroke"))
        .is_some() ||
        element
            .attribute_as_str(&ns!(), &LocalName::from("style"))
            .is_some()
}

/// Reads a plain numeric attribute, falling back to `default`.
pub(crate) fn number_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default)
}

pub(crate) fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

pub(crate) fn lp_or_auto_to_f32(lp: &NonNegativeLengthPercentageOrAuto) -> Option<f32> {
    match lp {
        GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
            Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0))
        },
        GenericLengthPercentageOrAuto::Auto => None,
    }
}

/// Parses a length attribute, returning `default` when missing or unparseable.
pub(crate) fn length_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    length_attr_opt(element, attr).unwrap_or(default)
}

pub(crate) fn length_attr_opt(element: &ServoLayoutElement<'_>, attr: &str) -> Option<f32> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(parse_length_attr)
}

pub(crate) fn parse_length_attr(value: &str) -> Option<f32> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value.parse::<f32>().ok()
}

/// Parses a gradient coordinate attribute (`x1`, `cx`, `r`, …).
///
/// Per SVG these accept either a bare `<number>` or a `<percentage>`; a
/// percentage is the same fraction expressed in the 0–100 range. Returns
/// `default` when missing or unparseable.
pub(crate) fn number_or_percentage_attr(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    default: f32,
) -> f32 {
    let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from(attr)) else {
        return default;
    };
    let value = value.trim();
    if let Some(pct) = value.strip_suffix('%') {
        pct.parse::<f32>()
            .ok()
            .map(|v| v / 100.0)
            .unwrap_or(default)
    } else {
        parse_length_attr(value).unwrap_or(default)
    }
}

/// Parses a `<length>|<percentage>` attribute value (used for a `pattern`'s
/// `x`/`y`/`width`/`height`). A percentage becomes a 0–1 fraction; any other
/// value is taken as a raw number (font-relative units are not resolved here).
pub(crate) fn length_or_percentage_attr(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    default: f32,
) -> f32 {
    let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from(attr)) else {
        return default;
    };
    match value.trim().parse::<svgtypes::Length>() {
        Ok(length) if length.unit == LengthUnit::Percent => length.number as f32 / 100.0,
        Ok(length) => length.number as f32,
        Err(_) => default,
    }
}

pub(crate) fn parse_transform(value: &str) -> usvg::Transform {
    let mut transform = usvg::Transform::identity();
    for token in TransformListParser::from(value) {
        let Ok(token) = token else {
            continue;
        };
        let t = match token {
            TransformListToken::Matrix { a, b, c, d, e, f } => usvg::Transform::from_row(
                a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
            ),
            TransformListToken::Translate { tx, ty } => {
                usvg::Transform::from_translate(tx as f32, ty as f32)
            },
            TransformListToken::Scale { sx, sy } => {
                usvg::Transform::from_scale(sx as f32, sy as f32)
            },
            TransformListToken::Rotate { angle } => usvg::Transform::from_rotate(angle as f32),
            TransformListToken::SkewX { angle } => usvg::Transform::from_row(
                1.0,
                0.0,
                (angle as f32).to_radians().tan(),
                1.0,
                0.0,
                0.0,
            ),
            TransformListToken::SkewY { angle } => usvg::Transform::from_row(
                1.0,
                (angle as f32).to_radians().tan(),
                0.0,
                1.0,
                0.0,
                0.0,
            ),
        };
        transform = transform.pre_concat(t);
    }
    transform
}

/// Parses a `viewBox` + `preserveAspectRatio` into a [`usvg::ViewBox`].
pub(crate) fn parse_view_box(element: &ServoLayoutElement<'_>) -> Option<usvg::ViewBox> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from("viewBox"))?;
    let vb = value.parse::<svgtypes::ViewBox>().ok()?;
    let rect = usvg::NonZeroRect::from_xywh(vb.x as f32, vb.y as f32, vb.w as f32, vb.h as f32)?;

    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();

    Some(usvg::ViewBox { rect, aspect })
}

/// Parses a whitespace/comma-separated list of numbers (for `x`/`y`/`dx`/`dy`/
/// `rotate` text-positioning lists and filter primitive numeric lists).
pub(crate) fn parse_number_list(value: &str) -> Vec<f32> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(parse_length_attr)
        .collect()
}

/// Whether `ty` is a group-like element that is converted by the builder's
/// [`crate::svg::usvg_builder::build_group`].
pub(crate) fn is_group_element(ty: LayoutElementType) -> bool {
    matches!(
        ty,
        LayoutElementType::SVGSVGElement |
            LayoutElementType::SVGGElement |
            LayoutElementType::SVGAElement |
            LayoutElementType::SVGClipPathElement |
            LayoutElementType::SVGMaskElement
    )
}

/// Determines the image size and view box from the root `<svg>` element.
pub(crate) fn resolve_size_and_view_box(
    element: &ServoLayoutElement<'_>,
) -> Option<(usvg::Size, Option<usvg::ViewBox>)> {
    let view_box = parse_view_box(element);

    let width = element
        .attribute_as_str(&ns!(), &LocalName::from("width"))
        .and_then(parse_length_attr);
    let height = element
        .attribute_as_str(&ns!(), &LocalName::from("height"))
        .and_then(parse_length_attr);

    let size = match (width, height, view_box.map(|vb| vb.rect)) {
        (Some(w), Some(h), _) => usvg::Size::from_wh(w, h),
        (Some(w), None, Some(vb)) => usvg::Size::from_wh(w, vb.height() * w / vb.width()),
        (None, Some(h), Some(vb)) => usvg::Size::from_wh(vb.width() * h / vb.height(), h),
        (None, None, Some(vb)) => usvg::Size::from_wh(vb.width(), vb.height()),
        _ => usvg::Size::from_wh(100.0, 100.0),
    }?;

    Some((size, view_box))
}
