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

/// Whether `ty` is a group-like element that is converted by the builder's
/// [`crate::svg::usvg_builder::build_group`].
pub(crate) fn is_group_element(ty: LayoutElementType) -> bool {
    matches!(
        ty,
        LayoutElementType::SVGSVGElement | LayoutElementType::SVGGElement
    )
}

/// Determines the natural size of the root `<svg>` element from its `width`/
/// `height` attributes, defaulting to 100×100 when absent.
pub(crate) fn resolve_size(element: &ServoLayoutElement<'_>) -> Option<usvg::Size> {
    let width = element
        .attribute_as_str(&ns!(), &LocalName::from("width"))
        .and_then(parse_length_attr);
    let height = element
        .attribute_as_str(&ns!(), &LocalName::from("height"))
        .and_then(parse_length_attr);

    usvg::Size::from_wh(width.unwrap_or(100.0), height.unwrap_or(100.0))
}
