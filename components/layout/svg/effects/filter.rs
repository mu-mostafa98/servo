/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `filter` resolution: turns a `<filter>` element (and its `fe*` primitives)
//! into a [`usvg::Filter`].

use std::sync::Arc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use resvg::usvg::{self, filter, ApproxZeroUlps};
use script::layout_dom::ServoLayoutElement;

use crate::svg::builder::SvgContext;
use crate::svg::primitives::attrs::{
    element_id, length_attr, length_attr_opt, length_or_percentage_attr, number_attr,
    parse_number_list,
};

/// Returns the `id` referenced by an element's `filter="url(#id)"` attribute,
/// or `None` when the element has no filter (or `filter="none"`).
fn filter_reference(element: &ServoLayoutElement<'_>) -> Option<String> {
    let value = element
        .attribute_as_str(&ns!(), &LocalName::from("filter"))?
        .trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = value.strip_prefix("url(")?.strip_suffix(')')?;
    let inner = inner.trim().trim_matches('"').trim_matches('\'');
    let id = inner.strip_prefix('#')?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// The result of resolving an element's `filter` attribute.
pub(crate) enum FilterOutcome {
    /// No filter: the attribute is absent, `none`, or malformed — render normally.
    None,
    /// A valid filter to apply.
    Filter(Arc<filter::Filter>),
    /// The `filter` references a missing or empty `<filter>` element — per usvg the
    /// whole element must be dropped.
    Invalid,
}

/// Resolves an element's `filter` reference into a [`usvg::Filter`], mirroring usvg's
/// `convert_group`/`filter::convert` handshake: `filter="none"` is a no-op, while a
/// dangling reference or an empty filter drops the element.
pub(crate) fn resolve_filter<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> FilterOutcome {
    let Some(ref_id) = filter_reference(element) else {
        return FilterOutcome::None;
    };
    let Some(linked) = ctx.defs.get(&ref_id) else {
        return FilterOutcome::Invalid;
    };
    match build_filter(linked, object_bbox) {
        Some(filter) => FilterOutcome::Filter(filter),
        None => FilterOutcome::Invalid,
    }
}

/// Builds a [`usvg::Filter`] from a `<filter>` element, mirroring usvg's
/// `parser::filter::convert_url`.
///
/// `object_bbox` is the bounding box of the element *being filtered* (used by
/// `filterUnits="objectBoundingBox"`, the default, and by
/// `primitiveUnits="objectBoundingBox"`).
fn build_filter(
    element: &ServoLayoutElement<'_>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> Option<Arc<filter::Filter>> {
    let id_str = element_id(element)?;

    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("filterUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let primitive_units = match element
        .attribute_as_str(&ns!(), &LocalName::from("primitiveUnits"))
        .unwrap_or("userSpaceOnUse")
    {
        "objectBoundingBox" => usvg::Units::ObjectBoundingBox,
        _ => usvg::Units::UserSpaceOnUse,
    };

    // Filter region defaults to `-10% -10% 120% 120%`; percentages are resolved as
    // a fraction of the object bbox (`length_or_percentage_attr` maps `%` → `/100`),
    // the same convention usvg uses for `objectBoundingBox`.
    let rect = match units {
        usvg::Units::ObjectBoundingBox => {
            let x = length_or_percentage_attr(element, "x", -0.1);
            let y = length_or_percentage_attr(element, "y", -0.1);
            let w = length_or_percentage_attr(element, "width", 1.2);
            let h = length_or_percentage_attr(element, "height", 1.2);
            let r = usvg::NonZeroRect::from_xywh(x, y, w, h)?;
            // `objectBoundingBox` maps the `(0,0)-(1,1)` square onto the object bbox.
            r.bbox_transform(object_bbox?)
        },
        usvg::Units::UserSpaceOnUse => {
            let x = length_attr_opt(element, "x");
            let y = length_attr_opt(element, "y");
            let w = length_attr_opt(element, "width");
            let h = length_attr_opt(element, "height");
            match (x, y, w, h) {
                (Some(x), Some(y), Some(w), Some(h)) => usvg::NonZeroRect::from_xywh(x, y, w, h)?,
                // No explicit user-space region: fall back to the object bbox.
                _ => object_bbox?,
            }
        },
    };

    let primitives = build_filter_primitives(element, primitive_units, object_bbox, rect);
    if primitives.is_empty() {
        // An empty filter is invalid, mirroring usvg's `convert_url`.
        return None;
    }

    Some(Arc::new(filter::Filter::new(
        usvg::NonEmptyString::new(id_str)?,
        rect,
        primitives,
    )))
}

/// Builds the filter primitives (`fe*` children) of a `<filter>` element.
fn build_filter_primitives(
    filter_element: &ServoLayoutElement<'_>,
    primitive_units: usvg::Units,
    object_bbox: Option<usvg::NonZeroRect>,
    filter_region: usvg::NonZeroRect,
) -> Vec<filter::Primitive> {
    let mut primitives: Vec<filter::Primitive> = Vec::new();
    let mut result_idx = 0usize;
    let mut used_names: Vec<String> = Vec::new();

    // Scale factor for `primitiveUnits="objectBoundingBox"`: primitive lengths
    // (`stdDeviation`, `dx`/`dy`, `scale`, `radius`) are multiplied by the bbox size.
    let scale = match primitive_units {
        usvg::Units::ObjectBoundingBox => match object_bbox {
            Some(bbox) => bbox.size(),
            None => return Vec::new(),
        },
        usvg::Units::UserSpaceOnUse => usvg::Size::from_wh(1.0, 1.0).unwrap(),
    };

    for child in filter_element.as_node().dom_children() {
        let Some(child_el) = child.as_element() else {
            continue;
        };
        let tag = child_el.local_name().to_string();

        let kind = match tag.as_str() {
            "feGaussianBlur" => filter_gaussian_blur(&child_el, &primitives, scale),
            "feDropShadow" => filter_drop_shadow(&child_el, &primitives, scale),
            "feColorMatrix" => filter_color_matrix(&child_el, &primitives),
            "feOffset" => filter_offset(&child_el, &primitives, scale),
            "feBlend" => filter_blend(&child_el, &primitives),
            "feFlood" => filter_flood(&child_el),
            "feComposite" => filter_composite(&child_el, &primitives),
            "feMerge" => filter_merge(&child_el, &primitives),
            "feComponentTransfer" => filter_component_transfer(&child_el, &primitives),
            "feTurbulence" => filter_turbulence(&child_el),
            "feDisplacementMap" => filter_displacement_map(&child_el, &primitives, scale),
            "feConvolveMatrix" => match filter_convolve_matrix(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feMorphology" => filter_morphology(&child_el, &primitives, scale),
            "feDiffuseLighting" => match filter_diffuse_lighting(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feSpecularLighting" => match filter_specular_lighting(&child_el, &primitives) {
                Some(kind) => kind,
                None => filter::Kind::Flood(filter::Flood::new(
                    usvg::Color::black(),
                    usvg::Opacity::ZERO,
                )),
            },
            "feTile" => filter::Kind::Tile(filter::Tile::new(resolve_filter_input(
                &child_el, "in", &primitives,
            ))),
            _ => continue,
        };

        let color_interpolation = match child_el
            .attribute_as_str(&ns!(), &LocalName::from("color-interpolation-filters"))
        {
            Some("sRGB") => filter::ColorInterpolation::SRGB,
            _ => filter::ColorInterpolation::LinearRGB,
        };

        let result = gen_filter_result(&child_el, &mut result_idx, &mut used_names);

        // Primitive subregion defaults to the whole filter region; per-primitive
        // `x`/`y`/`width`/`height` (rarely used) are not yet supported.
        primitives.push(filter::Primitive::new(
            filter_region,
            color_interpolation,
            result,
            kind,
        ));
    }

    primitives
}

/// Generates a `result` name for a filter primitive, mirroring usvg's `gen_result`:
/// the explicit `result` attribute is used verbatim, otherwise a unique `resultN`
/// name is minted.
fn gen_filter_result(
    element: &ServoLayoutElement<'_>,
    idx: &mut usize,
    used: &mut Vec<String>,
) -> String {
    if let Some(s) = element.attribute_as_str(&ns!(), &LocalName::from("result")) {
        let s = s.to_string();
        used.push(s.clone());
        *idx += 1;
        s
    } else {
        loop {
            *idx += 1;
            let name = format!("result{}", *idx);
            if !used.contains(&name) {
                return name;
            }
        }
    }
}

/// Resolves a filter primitive's `in`/`in2` reference, mirroring usvg's
/// `resolve_input`: an unset input falls back to the previous primitive's result
/// (or `SourceGraphic` for the first primitive), and a reference to an unknown
/// result likewise falls back.
fn resolve_filter_input(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    primitives: &[filter::Primitive],
) -> filter::Input {
    match element.attribute_as_str(&ns!(), &LocalName::from(attr)) {
        Some(s) => {
            let input = parse_filter_input(s);
            if let filter::Input::Reference(ref name) = input {
                if !primitives.iter().any(|p| p.result() == name.as_str()) {
                    return match primitives.last() {
                        Some(prev) => filter::Input::Reference(prev.result().to_string()),
                        None => filter::Input::SourceGraphic,
                    };
                }
            }
            input
        },
        None => match primitives.last() {
            Some(prev) => filter::Input::Reference(prev.result().to_string()),
            None => filter::Input::SourceGraphic,
        },
    }
}

fn parse_filter_input(s: &str) -> filter::Input {
    match s {
        "SourceGraphic" => filter::Input::SourceGraphic,
        "SourceAlpha" => filter::Input::SourceAlpha,
        "BackgroundImage" | "BackgroundAlpha" | "FillPaint" | "StrokePaint" => {
            filter::Input::SourceGraphic
        },
        _ => filter::Input::Reference(s.to_string()),
    }
}

/// Parses a `stdDeviation` attribute (`x`, or `x y`) into a pair, scaled by the
/// primitive units. Returns `(0, 0)` for an unset/malformed value.
fn filter_std_dev(
    element: &ServoLayoutElement<'_>,
    scale: usvg::Size,
    default: &str,
) -> (usvg::PositiveF32, usvg::PositiveF32) {
    let text = element
        .attribute_as_str(&ns!(), &LocalName::from("stdDeviation"))
        .unwrap_or(default);
    let nums = parse_number_list(text);
    let (sx, sy) = match (nums.first(), nums.get(1)) {
        (Some(a), Some(b)) => (*a, *b),
        (Some(a), None) => (*a, *a),
        _ => (0.0, 0.0),
    };
    let sx = usvg::PositiveF32::new(sx * scale.width()).unwrap_or(usvg::PositiveF32::ZERO);
    let sy = usvg::PositiveF32::new(sy * scale.height()).unwrap_or(usvg::PositiveF32::ZERO);
    (sx, sy)
}

/// Parses a color attribute (e.g. `flood-color`, `lighting-color`), defaulting to
/// `default` when absent/unparseable.
fn filter_color(element: &ServoLayoutElement<'_>, attr: &str, default: usvg::Color) -> usvg::Color {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<svgtypes::Color>().ok())
        .map(|c| usvg::Color::new_rgb(c.red, c.green, c.blue))
        .unwrap_or(default)
}

/// Parses a 0..1 opacity attribute (e.g. `flood-opacity`), defaulting to 1.
fn filter_opacity(element: &ServoLayoutElement<'_>, attr: &str) -> usvg::Opacity {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(|s| s.parse::<f32>().ok())
        .and_then(|v| usvg::Opacity::new(v.clamp(0.0, 1.0)))
        .unwrap_or(usvg::Opacity::ONE)
}

fn filter_gaussian_blur(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let (sx, sy) = filter_std_dev(element, scale, "0 0");
    filter::Kind::GaussianBlur(filter::GaussianBlur::new(
        resolve_filter_input(element, "in", primitives),
        sx,
        sy,
    ))
}

fn filter_drop_shadow(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let (sx, sy) = filter_std_dev(element, scale, "2 2");
    let color = filter_color(element, "flood-color", usvg::Color::black());
    let opacity = filter_opacity(element, "flood-opacity");
    filter::Kind::DropShadow(filter::DropShadow::new(
        resolve_filter_input(element, "in", primitives),
        length_attr(element, "dx", 2.0) * scale.width(),
        length_attr(element, "dy", 2.0) * scale.height(),
        sx,
        sy,
        color,
        opacity,
    ))
}

fn filter_color_matrix(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let values = element
        .attribute_as_str(&ns!(), &LocalName::from("values"))
        .map(parse_number_list)
        .unwrap_or_default();
    let kind = match element.attribute_as_str(&ns!(), &LocalName::from("type")) {
        Some("saturate") => {
            let n = values.first().copied().unwrap_or(1.0).clamp(0.0, 1.0);
            filter::ColorMatrixKind::Saturate(usvg::PositiveF32::new(n).unwrap())
        },
        Some("hueRotate") => {
            filter::ColorMatrixKind::HueRotate(values.first().copied().unwrap_or(0.0))
        },
        Some("luminanceToAlpha") => filter::ColorMatrixKind::LuminanceToAlpha,
        _ => {
            if values.len() == 20 {
                filter::ColorMatrixKind::Matrix(values)
            } else {
                filter::ColorMatrixKind::default()
            }
        },
    };
    filter::Kind::ColorMatrix(filter::ColorMatrix::new(
        resolve_filter_input(element, "in", primitives),
        kind,
    ))
}

fn filter_offset(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    filter::Kind::Offset(filter::Offset::new(
        resolve_filter_input(element, "in", primitives),
        length_attr(element, "dx", 0.0) * scale.width(),
        length_attr(element, "dy", 0.0) * scale.height(),
    ))
}

fn filter_blend(element: &ServoLayoutElement<'_>, primitives: &[filter::Primitive]) -> filter::Kind {
    let mode = match element.attribute_as_str(&ns!(), &LocalName::from("mode")) {
        Some("multiply") => usvg::BlendMode::Multiply,
        Some("screen") => usvg::BlendMode::Screen,
        Some("darken") => usvg::BlendMode::Darken,
        Some("lighten") => usvg::BlendMode::Lighten,
        _ => usvg::BlendMode::Normal,
    };
    filter::Kind::Blend(filter::Blend::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        mode,
    ))
}

fn filter_flood(element: &ServoLayoutElement<'_>) -> filter::Kind {
    let color = filter_color(element, "flood-color", usvg::Color::black());
    let opacity = filter_opacity(element, "flood-opacity");
    filter::Kind::Flood(filter::Flood::new(color, opacity))
}

fn filter_composite(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let operator = match element.attribute_as_str(&ns!(), &LocalName::from("operator")) {
        Some("in") => filter::CompositeOperator::In,
        Some("out") => filter::CompositeOperator::Out,
        Some("atop") => filter::CompositeOperator::Atop,
        Some("xor") => filter::CompositeOperator::Xor,
        Some("arithmetic") => filter::CompositeOperator::Arithmetic {
            k1: number_attr(element, "k1", 0.0),
            k2: number_attr(element, "k2", 0.0),
            k3: number_attr(element, "k3", 0.0),
            k4: number_attr(element, "k4", 0.0),
        },
        _ => filter::CompositeOperator::Over,
    };
    filter::Kind::Composite(filter::Composite::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        operator,
    ))
}

fn filter_merge(element: &ServoLayoutElement<'_>, primitives: &[filter::Primitive]) -> filter::Kind {
    let mut inputs = Vec::new();
    for child in element.as_node().dom_children() {
        if let Some(child_el) = child.as_element() {
            inputs.push(resolve_filter_input(&child_el, "in", primitives));
        }
    }
    filter::Kind::Merge(filter::Merge::new(inputs))
}

fn filter_component_transfer(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> filter::Kind {
    let mut func_r = filter::TransferFunction::Identity;
    let mut func_g = filter::TransferFunction::Identity;
    let mut func_b = filter::TransferFunction::Identity;
    let mut func_a = filter::TransferFunction::Identity;
    for child in element.as_node().dom_children() {
        let Some(child_el) = child.as_element() else {
            continue;
        };
        let tag = child_el.local_name().to_string();
        let Some(func) = filter_transfer_function(&child_el) else {
            continue;
        };
        match tag.as_str() {
            "feFuncR" => func_r = func,
            "feFuncG" => func_g = func,
            "feFuncB" => func_b = func,
            "feFuncA" => func_a = func,
            _ => {},
        }
    }
    filter::Kind::ComponentTransfer(filter::ComponentTransfer::new(
        resolve_filter_input(element, "in", primitives),
        func_r,
        func_g,
        func_b,
        func_a,
    ))
}

fn filter_transfer_function(element: &ServoLayoutElement<'_>) -> Option<filter::TransferFunction> {
    let ty = element.attribute_as_str(&ns!(), &LocalName::from("type"))?;
    let table = element
        .attribute_as_str(&ns!(), &LocalName::from("tableValues"))
        .map(parse_number_list);
    Some(match ty {
        "identity" => filter::TransferFunction::Identity,
        "table" => filter::TransferFunction::Table(table.unwrap_or_default()),
        "discrete" => filter::TransferFunction::Discrete(table.unwrap_or_default()),
        "linear" => filter::TransferFunction::Linear {
            slope: number_attr(element, "slope", 1.0),
            intercept: number_attr(element, "intercept", 0.0),
        },
        "gamma" => filter::TransferFunction::Gamma {
            amplitude: number_attr(element, "amplitude", 1.0),
            exponent: number_attr(element, "exponent", 1.0),
            offset: number_attr(element, "offset", 0.0),
        },
        _ => return None,
    })
}

fn filter_turbulence(element: &ServoLayoutElement<'_>) -> filter::Kind {
    let (base_x, base_y) = {
        let list = element
            .attribute_as_str(&ns!(), &LocalName::from("baseFrequency"))
            .map(parse_number_list)
            .unwrap_or_default();
        let (x, y) = match (list.first(), list.get(1)) {
            (Some(a), Some(b)) => (*a, *b),
            (Some(a), None) => (*a, *a),
            _ => (0.0, 0.0),
        };
        let x = usvg::PositiveF32::new(x).unwrap_or(usvg::PositiveF32::ZERO);
        let y = usvg::PositiveF32::new(y).unwrap_or(usvg::PositiveF32::ZERO);
        (x, y)
    };
    let num_octaves = number_attr(element, "numOctaves", 1.0).max(0.0).round() as u32;
    let kind = match element.attribute_as_str(&ns!(), &LocalName::from("type")) {
        Some("fractalNoise") => filter::TurbulenceKind::FractalNoise,
        _ => filter::TurbulenceKind::Turbulence,
    };
    filter::Kind::Turbulence(filter::Turbulence::new(
        base_x,
        base_y,
        num_octaves,
        number_attr(element, "seed", 0.0).trunc() as i32,
        element.attribute_as_str(&ns!(), &LocalName::from("stitchTiles")) == Some("stitch"),
        kind,
    ))
}

fn filter_displacement_map(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let channel = |attr: &str| match element.attribute_as_str(&ns!(), &LocalName::from(attr)) {
        Some("R") => filter::ColorChannel::R,
        Some("G") => filter::ColorChannel::G,
        Some("B") => filter::ColorChannel::B,
        _ => filter::ColorChannel::A,
    };
    let scale = (scale.width() + scale.height()) / 2.0;
    filter::Kind::DisplacementMap(filter::DisplacementMap::new(
        resolve_filter_input(element, "in", primitives),
        resolve_filter_input(element, "in2", primitives),
        number_attr(element, "scale", 0.0) * scale,
        channel("xChannelSelector"),
        channel("yChannelSelector"),
    ))
}

fn filter_convolve_matrix(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let (order_x, order_y) = {
        let list = element
            .attribute_as_str(&ns!(), &LocalName::from("order"))
            .map(parse_number_list)
            .unwrap_or_default();
        let ox = list.first().copied().unwrap_or(3.0).max(1.0) as u32;
        let oy = list.get(1).copied().unwrap_or(ox as f32).max(1.0) as u32;
        (ox, oy)
    };
    let matrix = element
        .attribute_as_str(&ns!(), &LocalName::from("kernelMatrix"))
        .map(parse_number_list)
        .unwrap_or_default();
    if matrix.len() != (order_x * order_y) as usize {
        return None;
    }

    let mut kernel_sum: f32 = matrix.iter().sum();
    kernel_sum = (kernel_sum * 1_000_000.0).round() / 1_000_000.0;
    if kernel_sum.approx_zero_ulps(4) {
        kernel_sum = 1.0;
    }
    let divisor = element
        .attribute_as_str(&ns!(), &LocalName::from("divisor"))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(kernel_sum);
    if divisor.approx_zero_ulps(4) {
        return None;
    }
    let divisor = usvg::NonZeroF32::new(divisor)?;

    let default_target = ((order_x as f32) / 2.0).floor() as u32;
    let target_x = number_attr(element, "targetX", default_target as f32) as i32;
    let target_y = number_attr(element, "targetY", default_target as f32) as i32;
    if target_x < 0 || target_x >= order_x as i32 || target_y < 0 || target_y >= order_y as i32 {
        return None;
    }

    let matrix_data =
        filter::ConvolveMatrixData::new(target_x as u32, target_y as u32, order_x, order_y, matrix)?;

    let edge_mode = match element.attribute_as_str(&ns!(), &LocalName::from("edgeMode")) {
        Some("none") => filter::EdgeMode::None,
        Some("wrap") => filter::EdgeMode::Wrap,
        _ => filter::EdgeMode::Duplicate,
    };
    let preserve_alpha = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAlpha"))
        .unwrap_or("false")
        == "true";

    Some(filter::Kind::ConvolveMatrix(filter::ConvolveMatrix::new(
        resolve_filter_input(element, "in", primitives),
        matrix_data,
        divisor,
        number_attr(element, "bias", 0.0),
        edge_mode,
        preserve_alpha,
    )))
}

fn filter_morphology(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
    scale: usvg::Size,
) -> filter::Kind {
    let operator = match element.attribute_as_str(&ns!(), &LocalName::from("operator")) {
        Some("dilate") => filter::MorphologyOperator::Dilate,
        _ => filter::MorphologyOperator::Erode,
    };
    let list = element
        .attribute_as_str(&ns!(), &LocalName::from("radius"))
        .map(parse_number_list)
        .unwrap_or_default();
    let mut rx = list.first().copied().unwrap_or(scale.width());
    let mut ry = list.get(1).copied().unwrap_or(rx);
    // A zero radius on either axis resets it to 1.0 (Chrome/Safari behaviour).
    if rx.approx_zero_ulps(4) && ry.approx_zero_ulps(4) {
        rx = 1.0;
        ry = 1.0;
    } else if rx.approx_zero_ulps(4) {
        rx = 1.0;
    } else if ry.approx_zero_ulps(4) {
        ry = 1.0;
    }
    let rx = usvg::PositiveF32::new(rx * scale.width()).unwrap_or(usvg::PositiveF32::new(1.0).unwrap());
    let ry = usvg::PositiveF32::new(ry * scale.height()).unwrap_or(usvg::PositiveF32::new(1.0).unwrap());
    filter::Kind::Morphology(filter::Morphology::new(
        resolve_filter_input(element, "in", primitives),
        operator,
        rx,
        ry,
    ))
}

fn filter_diffuse_lighting(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let light_source = filter_light_source(element)?;
    Some(filter::Kind::DiffuseLighting(filter::DiffuseLighting::new(
        resolve_filter_input(element, "in", primitives),
        number_attr(element, "surfaceScale", 1.0),
        number_attr(element, "diffuseConstant", 1.0),
        filter_color(element, "lighting-color", usvg::Color::white()),
        light_source,
    )))
}

fn filter_specular_lighting(
    element: &ServoLayoutElement<'_>,
    primitives: &[filter::Primitive],
) -> Option<filter::Kind> {
    let light_source = filter_light_source(element)?;
    let specular_exponent = number_attr(element, "specularExponent", 1.0);
    if !(1.0..=128.0).contains(&specular_exponent) {
        return None;
    }
    Some(filter::Kind::SpecularLighting(filter::SpecularLighting::new(
        resolve_filter_input(element, "in", primitives),
        number_attr(element, "surfaceScale", 1.0),
        number_attr(element, "specularConstant", 1.0),
        specular_exponent,
        filter_color(element, "lighting-color", usvg::Color::white()),
        light_source,
    )))
}

fn filter_light_source(element: &ServoLayoutElement<'_>) -> Option<filter::LightSource> {
    let child = element.as_node().dom_children().find_map(|c| {
        let el = c.as_element()?;
        let tag = el.local_name().to_string();
        matches!(tag.as_str(), "feDistantLight" | "fePointLight" | "feSpotLight")
            .then_some(el)
    })?;
    let tag = child.local_name().to_string();
    Some(match tag.as_str() {
        "feDistantLight" => filter::LightSource::DistantLight(filter::DistantLight {
            azimuth: number_attr(&child, "azimuth", 0.0),
            elevation: number_attr(&child, "elevation", 0.0),
        }),
        "fePointLight" => filter::LightSource::PointLight(filter::PointLight {
            x: number_attr(&child, "x", 0.0),
            y: number_attr(&child, "y", 0.0),
            z: number_attr(&child, "z", 0.0),
        }),
        "feSpotLight" => filter::LightSource::SpotLight(filter::SpotLight {
            x: number_attr(&child, "x", 0.0),
            y: number_attr(&child, "y", 0.0),
            z: number_attr(&child, "z", 0.0),
            points_at_x: number_attr(&child, "pointsAtX", 0.0),
            points_at_y: number_attr(&child, "pointsAtY", 0.0),
            points_at_z: number_attr(&child, "pointsAtZ", 0.0),
            specular_exponent: usvg::PositiveF32::new(number_attr(&child, "specularExponent", 1.0))
                .unwrap_or_else(|| usvg::PositiveF32::new(1.0).unwrap()),
            limiting_cone_angle: child
                .attribute_as_str(&ns!(), &LocalName::from("limitingConeAngle"))
                .and_then(|s| s.parse::<f32>().ok()),
        }),
        _ => return None,
    })
}
