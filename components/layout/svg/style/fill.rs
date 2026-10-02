/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `fill` / `fill-opacity` / `fill-rule` — the `ComputedValues` → [`FillParams`]
//! bridge and the matching presentation-attribute application.

use script::layout_dom::ServoLayoutElement;
use style::color::ColorSpace;
use style::values::computed::svg::{SVGOpacity, SVGPaintKind};
use servo_svg::style::paint_servers::PaintServer;
use servo_svg::style::{FillParams, FillRule, NodeStyle};
use servo_svg::units::{Id, Opacity};
use svgtypes::Color as SvgColor;

use super::{FromComputedValues, ResolvedPaint, resolve_svg_paint};
use crate::svg::primitives::attrs::{get_attr, parse_inline_style_prop};
use crate::svg::primitives::paint::parse_paint_server;

impl FromComputedValues for FillParams {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.fill, values);
        let opacity = match inherited_svg.fill_opacity {
            SVGOpacity::Opacity(opacity) => Opacity::new(opacity),
            _ => Opacity::ONE,
        };
        let fill_rule = match inherited_svg.fill_rule {
            style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
            style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
        };
        match paint {
            ResolvedPaint::Color(color) => Some(FillParams {
                paint_server: Some(PaintServer::Solid(color)),
                opacity,
                fill_rule,
            }),
            ResolvedPaint::PaintServer(id) => Some(FillParams {
                // An invalid/missing reference falls back to black at resolve
                // time (see `resolve_paint_server`).
                paint_server: Some(PaintServer::Ref { id: Id::new(id), fallback: None }),
                opacity,
                fill_rule,
            }),
            ResolvedPaint::None => {
                if matches!(inherited_svg.fill.kind, SVGPaintKind::None) {
                    None
                } else {
                    let current_color = values.clone_color();
                    let srgb = current_color.to_color_space(ColorSpace::Srgb);
                    Some(FillParams {
                        paint_server: Some(PaintServer::Solid(SvgColor::new_rgba(
                            (srgb.components.0.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.components.1.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.components.2.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.alpha.clamp(0.0, 1.0) * 255.0) as u8,
                        ))),
                        opacity,
                        fill_rule,
                    })
                }
            },
        }
    }
}

pub(crate) fn apply_fill_presentation_attrs(
    element: &ServoLayoutElement,
    style: &mut NodeStyle,
    current_color: &SvgColor,
) {
    let style_attr = get_attr(element, "style");
    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    let fill_value = match read_attr("fill") {
        Some(v) => v,
        None => return,
    };
    if fill_value.eq_ignore_ascii_case("none") {
        style.fill = None;
        return;
    }

    let fill = style.fill.get_or_insert_with(|| FillParams {
        paint_server: None,
        opacity: Opacity::ONE,
        fill_rule: FillRule::NonZero,
    });
    fill.paint_server = parse_paint_server(&fill_value, current_color);
    if let Some(v) = read_attr("fill-opacity") {
        fill.opacity = Opacity::new(v.parse::<f32>().unwrap_or(1.0));
    }
    if let Some(v) = read_attr("fill-rule") {
        fill.fill_rule = match v.trim() {
            "evenodd" | "even-odd" => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
    }
}
