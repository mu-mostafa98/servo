use style::color::ColorSpace;
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind};
use svg_engine::style::*;
use svgtypes::Color as SvgColor;
use webrender_api::ColorF;

pub fn build_style(computed_values: &style::properties::ComputedValues) -> NodeStyle {
    NodeStyle {
        fill: extract_fill(computed_values),
        ..Default::default()
    }
}

fn extract_fill(computed_values: &style::properties::ComputedValues) -> Option<FillParams> {
    let inherited_svg = computed_values.get_inherited_svg();
    let color = resolve_svg_paint(&inherited_svg.fill, computed_values);
    let opacity = match inherited_svg.fill_opacity {
        SVGOpacity::Opacity(opacity) => opacity,
        _ => 1.0,
    };
    let fill_rule = match inherited_svg.fill_rule {
        style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
        style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
    };

    color.map(|c| FillParams {
        color: Some(c),
        opacity,
        fill_rule,
    })
}

fn resolve_svg_paint(
    svg_paint: &SVGPaint,
    computed_values: &style::properties::ComputedValues,
) -> Option<SvgColor> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            let cf = ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
            );
            Some(SvgColor {
                red: (cf.r * 255.0) as u8,
                green: (cf.g * 255.0) as u8,
                blue: (cf.b * 255.0) as u8,
                alpha: (cf.a * 255.0) as u8,
            })
        },
        SVGPaintKind::None => None,
        _ => None,
    }
}
