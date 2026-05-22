use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray, SVGWidth,
};
use webrender_api::ColorF;

use crate::svg_engine::lengths::SvgLength;
use crate::svg_engine::shapes::{
    FillParams, ParsedGeometry, StrokeParams, SvgLineCap, SvgLineJoin, SvgTag,
};

/// Extract fill parameters from computed style.
pub fn extract_fill_params(style: &ComputedValues) -> FillParams {
    let isvg = style.get_inherited_svg();
    let color = resolve_svg_paint(&isvg.fill, style);
    let opacity = match isvg.fill_opacity {
        SVGOpacity::Opacity(o) => o,
        _ => 1.0,
    };
    FillParams { color, opacity }
}

/// Extract stroke parameters from computed style.
pub fn extract_stroke_params(style: &ComputedValues) -> StrokeParams {
    use style::computed_values::stroke_linecap::T as LineCapStyle;
    use style::computed_values::stroke_linejoin::T as LineJoinStyle;

    let isvg = style.get_inherited_svg();

    let color = resolve_svg_paint(&isvg.stroke, style);

    let width = match &isvg.stroke_width {
        SVGWidth::LengthPercentage(lp) => lp.to_used_value(app_units::Au::from_f32_px(300.0)).to_f32_px(),
        SVGWidth::ContextValue => 1.0,
    };

    let opacity = match isvg.stroke_opacity {
        SVGOpacity::Opacity(o) => o,
        _ => 1.0,
    };

    let dasharray = match &isvg.stroke_dasharray {
        SVGStrokeDashArray::Values(v) => {
            if v.is_empty() {
                None
            } else {
                Some(v.iter().map(|lp| lp.to_used_value(app_units::Au::from_f32_px(300.0)).to_f32_px()).collect())
            }
        }
        SVGStrokeDashArray::ContextValue => None,
    };

    let dashoffset = match &isvg.stroke_dashoffset {
        style::values::computed::svg::SVGLength::LengthPercentage(lp) => {
            lp.to_used_value(app_units::Au::from_f32_px(300.0)).to_f32_px()
        }
        _ => 0.0,
    };

    let linecap = match isvg.stroke_linecap {
        LineCapStyle::Butt => SvgLineCap::Butt,
        LineCapStyle::Round => SvgLineCap::Round,
        LineCapStyle::Square => SvgLineCap::Square,
    };

    let linejoin = match isvg.stroke_linejoin {
        LineJoinStyle::Miter => SvgLineJoin::Miter,
        LineJoinStyle::Round => SvgLineJoin::Round,
        LineJoinStyle::Bevel => SvgLineJoin::Bevel,
    };

    let miterlimit = isvg.stroke_miterlimit.0;

    StrokeParams {
        color,
        width,
        opacity,
        dasharray,
        dashoffset,
        linecap,
        linejoin,
        miterlimit,
    }
}

/// Extract geometry for a given SVG tag by reading DOM attributes.
pub fn extract_geometry(tag: SvgTag, get_attr: &dyn Fn(&str) -> Option<String>) -> ParsedGeometry {
    match tag {
        SvgTag::Rect => ParsedGeometry::Rect {
            x: get_attr("x").and_then(|s| SvgLength::parse(&s)),
            y: get_attr("y").and_then(|s| SvgLength::parse(&s)),
            width: get_attr("width").and_then(|s| SvgLength::parse(&s)),
            height: get_attr("height").and_then(|s| SvgLength::parse(&s)),
            rx: get_attr("rx").and_then(|s| SvgLength::parse(&s)),
            ry: get_attr("ry").and_then(|s| SvgLength::parse(&s)),
        },
        SvgTag::Circle => ParsedGeometry::Circle {
            cx: get_attr("cx").and_then(|s| SvgLength::parse(&s)),
            cy: get_attr("cy").and_then(|s| SvgLength::parse(&s)),
            r: get_attr("r").and_then(|s| SvgLength::parse(&s)),
        },
        SvgTag::Ellipse => ParsedGeometry::Ellipse {
            cx: get_attr("cx").and_then(|s| SvgLength::parse(&s)),
            cy: get_attr("cy").and_then(|s| SvgLength::parse(&s)),
            rx: get_attr("rx").and_then(|s| SvgLength::parse(&s)),
            ry: get_attr("ry").and_then(|s| SvgLength::parse(&s)),
        },
        SvgTag::Line => ParsedGeometry::Line {
            x1: get_attr("x1").and_then(|s| SvgLength::parse(&s)),
            y1: get_attr("y1").and_then(|s| SvgLength::parse(&s)),
            x2: get_attr("x2").and_then(|s| SvgLength::parse(&s)),
            y2: get_attr("y2").and_then(|s| SvgLength::parse(&s)),
        },
        SvgTag::Polygon => {
            let points = get_attr("points").and_then(|s| crate::svg_engine::points::parse_points(&s));
            match points {
                Some(p) => ParsedGeometry::Polygon { points: p },
                None => ParsedGeometry::None,
            }
        }
        SvgTag::Polyline => {
            let points = get_attr("points").and_then(|s| crate::svg_engine::points::parse_points(&s));
            match points {
                Some(p) => ParsedGeometry::Polyline { points: p },
                None => ParsedGeometry::None,
            }
        }
        SvgTag::Path => {
            let path = get_attr("d").and_then(|s| crate::svg_engine::path::parse_path(&s));
            match path {
                Some(p) => ParsedGeometry::Path { path: p },
                None => ParsedGeometry::None,
            }
        }
        _ => ParsedGeometry::None,
    }
}

/// Extract opacity from computed effects.
pub fn extract_opacity(style: &ComputedValues) -> f32 {
    style.get_effects().opacity
}

/// Resolve an SVGPaint to an optional ColorF.
fn resolve_svg_paint(paint: &SVGPaint, style: &ComputedValues) -> Option<ColorF> {
    match &paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = style.clone_color();
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
        _ => None, // PaintServer / ContextFill / ContextStroke — Phase 2
    }
}
