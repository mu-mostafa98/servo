/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint resolution: gradients, paint-server collection, and `fill`/`stroke`
//! mapping from Servo computed styles to usvg paint.
//!
//! These are leaf functions — they depend only on Servo style types and usvg
//! constructors, never on the builder. The recursive `<pattern>` builder (which
//! calls back into [`crate::svg::usvg_builder::build_usvg_node`]) lives in
//! [`crate::svg::effects::paint`].

use std::collections::HashMap;
use std::sync::Arc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::Length;
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray};
use style::values::generics::svg::SVGLength;

use crate::svg::primitives::attrs::{
    element_id, element_layout_type, number_or_percentage_attr, parse_transform,
};

/// Paint servers referenced by `url(#id)` and collected from
/// `<linearGradient>`, `<radialGradient>` and `<pattern>` elements before the
/// main tree walk.
#[derive(Default)]
pub(crate) struct Gradients {
    pub(crate) linear: HashMap<String, Arc<usvg::LinearGradient>>,
    pub(crate) radial: HashMap<String, Arc<usvg::RadialGradient>>,
    pub(crate) pattern: HashMap<String, Arc<usvg::Pattern>>,
}

/// Collects `<linearGradient>`/`<radialGradient>` definitions (and their
/// `<stop>` children) into `gradients`, and records `<pattern>` elements into
/// `pattern_elements` for a second build pass.
pub(crate) fn collect_paint_servers<'a>(
    node: ServoLayoutNode<'a>,
    gradients: &mut Gradients,
    pattern_elements: &mut Vec<ServoLayoutElement<'a>>,
) {
    let Some(element) = node.as_element() else {
        return;
    };
    match element_layout_type(&element) {
        LayoutElementType::SVGLinearGradientElement => {
            if let (Some(id), Some(grad)) = (element_id(&element), build_linear_gradient(&element))
            {
                gradients.linear.insert(id, Arc::new(grad));
            }
            return;
        },
        LayoutElementType::SVGRadialGradientElement => {
            if let (Some(id), Some(grad)) = (element_id(&element), build_radial_gradient(&element))
            {
                gradients.radial.insert(id, Arc::new(grad));
            }
            return;
        },
        LayoutElementType::SVGPatternElement => {
            if element_id(&element).is_some() {
                pattern_elements.push(element);
            }
            return;
        },
        _ => {},
    }

    for child in node.dom_children() {
        collect_paint_servers(child, gradients, pattern_elements);
    }
}

fn build_linear_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::LinearGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let x1 = number_or_percentage_attr(element, "x1", 0.0);
    let y1 = number_or_percentage_attr(element, "y1", 0.0);
    let x2 = number_or_percentage_attr(element, "x2", 1.0);
    let y2 = number_or_percentage_attr(element, "y2", 0.0);
    let base = build_base_gradient(element, id)?;
    Some(usvg::LinearGradient::new(base, x1, y1, x2, y2))
}

fn build_radial_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::RadialGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let cx = number_or_percentage_attr(element, "cx", 0.5);
    let cy = number_or_percentage_attr(element, "cy", 0.5);
    let r = usvg::PositiveF32::new(number_or_percentage_attr(element, "r", 0.5))?;
    let fx = number_or_percentage_attr(element, "fx", cx);
    let fy = number_or_percentage_attr(element, "fy", cy);
    let fr = usvg::PositiveF32::new(number_or_percentage_attr(element, "fr", 0.0))?;
    let base = build_base_gradient(element, id)?;
    Some(usvg::RadialGradient::new(base, cx, cy, r, fx, fy, fr))
}

fn build_base_gradient(
    element: &ServoLayoutElement<'_>,
    id: usvg::NonEmptyString,
) -> Option<usvg::BaseGradient> {
    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("gradientUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("gradientTransform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let spread_method = match element
        .attribute_as_str(&ns!(), &LocalName::from("spreadMethod"))
        .unwrap_or("pad")
    {
        "reflect" => usvg::SpreadMethod::Reflect,
        "repeat" => usvg::SpreadMethod::Repeat,
        _ => usvg::SpreadMethod::Pad,
    };

    let mut stops = Vec::new();
    for child in element.as_node().dom_children() {
        if let Some(stop) = child.as_element().and_then(|e| build_stop(&e)) {
            stops.push(stop);
        }
    }

    Some(usvg::BaseGradient::new(
        id,
        units,
        transform,
        spread_method,
        stops,
    ))
}

fn build_stop(element: &ServoLayoutElement<'_>) -> Option<usvg::Stop> {
    if element_layout_type(element) != LayoutElementType::SVGStopElement {
        return None;
    }
    let offset_raw = element.attribute_as_str(&ns!(), &LocalName::from("offset"))?;
    let offset = if let Some(pct) = offset_raw.strip_suffix('%') {
        pct.parse::<f32>().ok()? / 100.0
    } else {
        offset_raw.parse::<f32>().ok()?
    };
    let offset = usvg::StopOffset::new(offset.clamp(0.0, 1.0))?;

    let color = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-color"))
        .and_then(|s| s.parse::<svgtypes::Color>().ok())
        .unwrap_or_else(svgtypes::Color::black);

    let stop_opacity = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-opacity"))
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);
    // Fold any alpha carried by `stop-color` (`rgba()`/8-digit hex) into the stop
    // opacity, mirroring usvg's `split_alpha`.
    let opacity = (stop_opacity * color.alpha as f32 / 255.0).clamp(0.0, 1.0);

    Some(usvg::Stop::new(
        offset,
        usvg::Color::new_rgb(color.red, color.green, color.blue),
        usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE),
    ))
}

pub(crate) fn build_fill(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let (paint, color_alpha) = resolve_paint(&inherited.fill, computed, gradients)?;

    // `fill-opacity` multiplies the alpha already carried by the `fill` color
    // (e.g. `fill="rgba(..., 0.4)"`), per SVG2.
    let opacity = (match inherited.fill_opacity {
        SVGOpacity::Opacity(op) => op,
        _ => 1.0,
    } * color_alpha)
        .clamp(0.0, 1.0);
    let rule = match inherited.fill_rule {
        style::computed_values::fill_rule::T::Evenodd => usvg::FillRule::EvenOdd,
        _ => usvg::FillRule::NonZero,
    };

    let mut fill = usvg::Fill::new(paint);
    fill.opacity = usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE);
    fill.rule = rule;
    Some(fill)
}

pub(crate) fn build_stroke(
    computed: &ComputedValues,
    gradients: &Gradients,
    diagonal: f32,
) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let (paint, color_alpha) = resolve_paint(&inherited.stroke, computed, gradients)?;

    // A negative `stroke-width` is invalid CSS and is rejected by the parser
    // (`SVGWidth = NonNegativeLengthPercentage`), so it falls back to the
    // initial value `1` here in the computed style. We faithfully pass that
    // through to usvg/resvg, which renders a 1px stroke — matching Chrome/Edge.
    let width = match &inherited.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => nn_lp.0.resolve(Length::new(diagonal)).px(),
        _ => 1.0,
    };
    if width <= 0.0 {
        return None;
    }

    let mut stroke = usvg::Stroke::new(paint);
    stroke.width = usvg::StrokeWidth::new(width).unwrap_or(usvg::StrokeWidth::new(1.0).unwrap());
    // `stroke-opacity` multiplies the alpha already carried by the `stroke`
    // color, mirroring the fill path above.
    stroke.opacity = match inherited.stroke_opacity {
        SVGOpacity::Opacity(op) => {
            usvg::Opacity::new((op * color_alpha).clamp(0.0, 1.0)).unwrap_or(usvg::Opacity::ONE)
        },
        _ => usvg::Opacity::new(color_alpha).unwrap_or(usvg::Opacity::ONE),
    };
    stroke.linecap = match inherited.stroke_linecap {
        style::computed_values::stroke_linecap::T::Round => usvg::LineCap::Round,
        style::computed_values::stroke_linecap::T::Square => usvg::LineCap::Square,
        _ => usvg::LineCap::Butt,
    };
    stroke.linejoin = match inherited.stroke_linejoin {
        style::computed_values::stroke_linejoin::T::Round => usvg::LineJoin::Round,
        style::computed_values::stroke_linejoin::T::Bevel => usvg::LineJoin::Bevel,
        _ => usvg::LineJoin::Miter,
    };
    stroke.miterlimit = usvg::StrokeMiterlimit::new(inherited.stroke_miterlimit.0);
    if let SVGStrokeDashArray::Values(vs) = &inherited.stroke_dasharray {
        if !vs.is_empty() {
            let mut dasharray: Vec<f32> = vs
                .iter()
                .map(|v| v.0.resolve(Length::new(diagonal)).px())
                .collect();

            // Per SVG2, an odd-length dash array is repeated to yield an even
            // length, so dashes and gaps alternate correctly. usvg's own XML
            // parser does the same (`parser::style::conv_dasharray`); since we
            // build the tree programmatically we must replicate it here,
            // otherwise `tiny_skia_path::StrokeDash::new` rejects the odd list
            // and the stroke renders solid.
            if dasharray.len() % 2 != 0 {
                let mut doubled = dasharray.clone();
                doubled.extend_from_slice(&dasharray);
                dasharray = doubled;
            }

            stroke.dasharray = Some(dasharray);
        }
    }
    stroke.dashoffset = match &inherited.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => lp.resolve(Length::new(diagonal)).px(),
        _ => 0.0,
    };

    Some(stroke)
}

fn resolve_paint(
    svg_paint: &SVGPaint,
    computed: &ComputedValues,
    gradients: &Gradients,
) -> Option<(usvg::Paint, f32)> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            // `fill`/`stroke` may carry an alpha channel (`rgba()`, `hsla()`,
            // 4/8-digit hex, `color(..., / alpha)`). usvg's `Paint::Color` is
            // RGB-only, so we fold the color's own alpha into the fill/stroke
            // opacity instead of dropping it (which would render the shape at
            // full opacity).
            let alpha = srgb.alpha.clamp(0.0, 1.0);
            Some((
                usvg::Paint::Color(usvg::Color::new_rgb(
                    (srgb.components.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (srgb.components.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (srgb.components.2.clamp(0.0, 1.0) * 255.0).round() as u8,
                )),
                alpha,
            ))
        },
        SVGPaintKind::None => None,
        SVGPaintKind::PaintServer(url) => {
            let fragment = match url {
                style::url::ComputedUrl::Valid(u) => u.fragment().map(|s| s.to_string()),
                style::url::ComputedUrl::Invalid(s) => {
                    let trimmed = s.trim_start_matches('#');
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                },
            };
            let fragment = fragment?;
            let paint = if let Some(g) = gradients.linear.get(&fragment) {
                usvg::Paint::LinearGradient(g.clone())
            } else if let Some(g) = gradients.radial.get(&fragment) {
                usvg::Paint::RadialGradient(g.clone())
            } else if let Some(p) = gradients.pattern.get(&fragment) {
                usvg::Paint::Pattern(p.clone())
            } else {
                // Unresolved paint server: fall back to black, matching usvg's
                // behaviour when a referenced paint server is missing.
                usvg::Paint::Color(usvg::Color::black())
            };
            Some((paint, 1.0))
        },
        _ => None,
    }
}
