/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint resolution: maps Servo computed `fill`/`stroke` to usvg paint.
//!
//! Solid colors only in this phase; paint servers (`url(#...)` gradients and
//! patterns) are deferred to a later phase and currently resolve to no paint.

use resvg::usvg;
use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::Length;
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray};
use style::values::generics::svg::SVGLength;

pub(crate) fn build_fill(computed: &ComputedValues) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let (paint, color_alpha) = resolve_paint(&inherited.fill, computed)?;

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

pub(crate) fn build_stroke(computed: &ComputedValues, diagonal: f32) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let (paint, color_alpha) = resolve_paint(&inherited.stroke, computed)?;

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

fn resolve_paint(svg_paint: &SVGPaint, computed: &ComputedValues) -> Option<(usvg::Paint, f32)> {
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
        // Paint servers (`url(#...)` gradients/patterns) are not supported in
        // this phase and resolve to no paint.
        _ => None,
    }
}
