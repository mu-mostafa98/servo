use style::color::ColorSpace;
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray,
};
use style::values::generics::svg::SVGLength;
use svg_engine::style::*;
use svgtypes::Color as SvgColor;
use webrender_api::ColorF;

pub fn build_style(computed_values: &style::properties::ComputedValues) -> NodeStyle {
    NodeStyle {
        fill: extract_fill(computed_values),
        stroke: extract_stroke(computed_values),
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

fn extract_stroke(computed_values: &style::properties::ComputedValues) -> Option<StrokeParams> {
    let inherited_svg = computed_values.get_inherited_svg();
    let color = resolve_svg_paint(&inherited_svg.stroke, computed_values);
    let opacity = match inherited_svg.stroke_opacity {
        SVGOpacity::Opacity(opacity) => opacity,
        _ => 1.0,
    };

    let width = match &inherited_svg.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => {
            nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0)
        },
        _ => 1.0,
    };

    if color.is_none() || width <= 0.0 {
        return None;
    }

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
        SVGStrokeDashArray::Values(values) => {
            if values.is_empty() {
                None
            } else {
                Some(
                    values
                        .iter()
                        .map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0))
                        .collect(),
                )
            }
        },
        _ => None,
    };

    let dash_offset = match &inherited_svg.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => {
            lp.to_length().map(|l| l.px()).unwrap_or(0.0)
        },
        _ => 0.0,
    };

    Some(StrokeParams {
        color,
        opacity,
        width,
        line_cap,
        line_join,
        miter_limit,
        dash_array,
        dash_offset,
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
