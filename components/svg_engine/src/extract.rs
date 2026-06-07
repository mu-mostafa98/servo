/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use style::properties::ComputedValues;
use style::values::computed::svg:: {SVGPaint, SVGOpacity, SVGPaintKind};
use style::color::ColorSpace;
use webrender_api::ColorF;

use crate::render_tree::*;
use crate::styles::*;
use crate::shapes::*;

pub fn extract_node_style(computed_values: &ComputedValues) -> NodeStyle {
    NodeStyle{
        fill: Some(extract_fill_params(computed_values)),
    }
    
}

pub fn extract_fill_params(computed_values: &ComputedValues) -> FillParams {
    
    let inhirited_svg = computed_values.get_inherited_svg();
    let color = resolve_svg_paint(&inhirited_svg.fill, computed_values);
    let opacity = match inhirited_svg.fill_opacity {
        SVGOpacity::Opacity(opacity) => opacity,
        _ => 1.0,
    };
    let fill_rule = match inhirited_svg.fill_rule {
        style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
        style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
    };
    FillParams {
        color,
        opacity,
        fill_rule,
    }
}

fn resolve_svg_paint(svg_paint: &SVGPaint, computed_values: &ComputedValues) -> Option<ColorF> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
            ))
        }
        SVGPaintKind::None => None,
        _ => None,
    }
}

pub fn extract_tag(name: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<SvgTag> {
    match name {
        "rect" => extract_rect(get_attr).map(|s| SvgTag::Shape(Shape::Rect(s))),
        _ => None,
    }
}

fn extract_rect(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Rectangle> {
    Some(Rectangle {
        x: parse_length("x", get_attr)?,
        y: parse_length("y", get_attr)?,
        width: parse_length("width", get_attr)?,
        height: parse_length("height", get_attr)?,
        rx: parse_length("rx", get_attr),
        ry: parse_length("ry", get_attr),
    })
}

fn parse_length(attr: &str, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<f32> {
    let value = get_attr(attr)?;
    value.trim_end_matches("px").trim().parse::<f32>().ok()
}