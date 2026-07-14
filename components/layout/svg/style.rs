/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind};
use style::values::specified::box_ as stylo_box;
use svg_engine::style::*;
use svgtypes::Color as SvgColor;
use web_atoms::ns;

use crate::context::LayoutContext;

pub trait FromComputedValues: Sized {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self>;
}

enum ResolvedPaint {
    Color(SvgColor),
    // PaintServer(String),
    None,
}

fn resolve_svg_paint(
    svg_paint: &SVGPaint,
    computed_values: &style::properties::ComputedValues,
) -> ResolvedPaint {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            ResolvedPaint::Color(SvgColor::new_rgba(
                (srgb.components.0.clamp(0.0, 1.0) * 255.0) as u8,
                (srgb.components.1.clamp(0.0, 1.0) * 255.0) as u8,
                (srgb.components.2.clamp(0.0, 1.0) * 255.0) as u8,
                (srgb.alpha.clamp(0.0, 1.0) * 255.0) as u8,
            ))
        },
        SVGPaintKind::None => ResolvedPaint::None,
        // SVGPaintKind::PaintServer(url) => {},
        _ => ResolvedPaint::None,
    }
}

impl FromComputedValues for FillParams {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.fill, values);
        let opacity = match inherited_svg.fill_opacity {
            SVGOpacity::Opacity(opacity) => opacity.clamp(0.0, 1.0),
            _ => 1.0,
        };
        let fill_rule = match inherited_svg.fill_rule {
            style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
            style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
        };

        match paint {
            ResolvedPaint::Color(color) => Some(FillParams {
                color: Some(color),
                opacity,
                fill_rule,
            }),
            // ResolvedPaint::PaintServer(id) => {},
            ResolvedPaint::None => None,
        }
    }
}

impl FromComputedValues for NodeStyle {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let svg_visibility = match values.get_inherited_box().visibility {
            style::computed_values::visibility::T::Visible => Visibility::Visible,
            style::computed_values::visibility::T::Hidden => Visibility::Hidden,
            style::computed_values::visibility::T::Collapse => Visibility::Collapse,
        };

        let display = values.get_box().display;
        let svg_display = if display.outside() == stylo_box::DisplayOutside::None ||
            display.inside() == stylo_box::DisplayInside::None
        {
            Display::None
        } else {
            Display::Inline
        };

        Some(NodeStyle {
            visibility: svg_visibility,
            display: svg_display,
            fill: FillParams::from_computed_values(values),
            opacity: values.get_effects().opacity,
        })
    }
}

pub(crate) fn get_attr(element: &ServoLayoutElement, attr: &str) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .map(|s| s.to_string())
}

pub(crate) fn build_style(node: ServoLayoutNode, context: &LayoutContext) -> NodeStyle {
    let element = node.as_element().unwrap();

    let mut style = if element.style_data().is_some() {
        let computed = node.style(&context.style_context);
        NodeStyle::from_computed_values(&computed).unwrap_or_default()
    } else {
        NodeStyle::default()
    };

    style
}
