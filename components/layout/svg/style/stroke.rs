/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `stroke` / `stroke-*` — the `ComputedValues` → [`StrokeParams`] bridge and
//! the matching presentation-attribute application.

use script::layout_dom::ServoLayoutElement;
use style::color::ColorSpace;
use style::values::computed::svg::{SVGOpacity, SVGStrokeDashArray};
use style::values::generics::svg::SVGLength;
use servo_svg::style::paint_servers::PaintServer;
use servo_svg::style::{LineCap, LineJoin, NodeStyle, StrokeParams};
use servo_svg::units::{Id, Length, Opacity};
use svgtypes::Color as SvgColor;

use super::{FromComputedValues, ResolvedPaint, resolve_svg_paint};
use crate::svg::primitives::attrs::{get_attr, parse_inline_style_prop};
use crate::svg::primitives::paint::parse_paint_server;

impl FromComputedValues for StrokeParams {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.stroke, values);
        let opacity = match inherited_svg.stroke_opacity {
            SVGOpacity::Opacity(opacity) => Opacity::new(opacity),
            _ => Opacity::ONE,
        };
        let width = match &inherited_svg.stroke_width {
            SVGLength::LengthPercentage(nn_lp) => {
                nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0)
            },
            _ => 1.0,
        };
        let line_cap = match inherited_svg.stroke_linecap {
            style::computed_values::stroke_linecap::T::Butt => LineCap::Butt,
            style::computed_values::stroke_linecap::T::Round => LineCap::Round,
            style::computed_values::stroke_linecap::T::Square => LineCap::Square,
        };
        let line_join = match inherited_svg.stroke_linejoin {
            style::computed_values::stroke_linejoin::T::Miter => LineJoin::Miter,
            style::computed_values::stroke_linejoin::T::Round => LineJoin::Round,
            style::computed_values::stroke_linejoin::T::Bevel => LineJoin::Bevel,
        };
        let miter_limit = inherited_svg.stroke_miterlimit.0;
        let dash_array = match &inherited_svg.stroke_dasharray {
            SVGStrokeDashArray::Values(vs) => {
                if vs.is_empty() {
                    None
                } else {
                    Some(
                        vs.iter()
                            .map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0))
                            .collect(),
                    )
                }
            },
            _ => None,
        };
        let dash_offset = match &inherited_svg.stroke_dashoffset {
            SVGLength::LengthPercentage(lp) => lp.to_length().map(|l| l.px()).unwrap_or(0.0),
            _ => 0.0,
        };
        if width <= 0.0 {
            return None;
        }
        match paint {
            ResolvedPaint::Color(color) => Some(StrokeParams {
                paint_server: Some(PaintServer::Solid(color)),
                opacity,
                width: Length::new(width),
                line_cap,
                line_join,
                miter_limit,
                dash_array,
                dash_offset,
            }),
            ResolvedPaint::PaintServer(id) => Some(StrokeParams {
                // An invalid/missing reference falls back to black at resolve
                // time (see `resolve_paint_server`).
                paint_server: Some(PaintServer::Ref { id: Id::new(id), fallback: None }),
                opacity,
                width: Length::new(width),
                line_cap,
                line_join,
                miter_limit,
                dash_array,
                dash_offset,
            }),
            ResolvedPaint::None => None,
        }
    }
}

pub(crate) fn apply_stroke_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let style_attr = get_attr(element, "style");
    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    let stroke_value = match read_attr("stroke") {
        Some(v) => v,
        None => return,
    };
    if stroke_value.eq_ignore_ascii_case("none") {
        style.stroke = None;
        return;
    }

    let stroke = style.stroke.get_or_insert_with(|| StrokeParams {
        paint_server: None,
        opacity: Opacity::ONE,
        width: Length::new(1.0),
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        miter_limit: 4.0,
        dash_array: None,
        dash_offset: 0.0,
    });

    stroke.paint_server = parse_paint_server(&stroke_value);
    if let Some(v) = read_attr("stroke-width") {
        stroke.width = Length::new(
            v.trim_end_matches("px")
                .parse::<f32>()
                .unwrap_or(1.0)
                .max(0.0),
        );
    }
    if let Some(v) = read_attr("stroke-opacity") {
        stroke.opacity = Opacity::new(v.parse::<f32>().unwrap_or(1.0));
    }
    if let Some(v) = read_attr("stroke-linecap") {
        stroke.line_cap = match v.trim() {
            "round" => LineCap::Round,
            "square" => LineCap::Square,
            _ => LineCap::Butt,
        };
    }
    if let Some(v) = read_attr("stroke-linejoin") {
        stroke.line_join = match v.trim() {
            "miter-clip" => LineJoin::MiterClip,
            "round" => LineJoin::Round,
            "bevel" => LineJoin::Bevel,
            "arcs" => LineJoin::Arcs,
            _ => LineJoin::Miter,
        };
    }
    if let Some(v) = read_attr("stroke-miterlimit") {
        if let Ok(ml) = v.parse::<f32>() {
            stroke.miter_limit = ml;
        }
    }
    if let Some(v) = read_attr("stroke-dasharray") {
        if v.eq_ignore_ascii_case("none") {
            stroke.dash_array = None;
        } else {
            let dashes: Vec<f32> = v
                .split(|c| c == ',' || c == ' ')
                .filter_map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        t.parse::<f32>().ok()
                    }
                })
                .collect();
            stroke.dash_array = if dashes.is_empty() {
                None
            } else {
                Some(dashes)
            };
        }
    }
    if let Some(v) = read_attr("stroke-dashoffset") {
        stroke.dash_offset = v.trim_end_matches("px").parse::<f32>().unwrap_or(0.0);
    }
}
