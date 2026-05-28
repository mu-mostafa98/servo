use style::color::ColorSpace;
use style::properties::ComputedValues;
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray, SVGWidth,
};
use webrender_api::ColorF;

use crate::lengths::SvgLength;
use crate::shapes::{
    FillParams, FillRule, Geometry, NodeEffects, RenderHints, StrokeParams, SvgLineCap,
    SvgLineJoin, SvgTag, VectorEffect, Visibility,
};
use crate::{path, points};

/// Extract fill parameters from computed style.
pub fn extract_fill_params(style: &ComputedValues) -> FillParams {
    let isvg = style.get_inherited_svg();
    let color = resolve_svg_paint(&isvg.fill, style);
    let opacity = match isvg.fill_opacity {
        SVGOpacity::Opacity(o) => o,
        _ => 1.0,
    };
    let fill_rule = match style.get_inherited_svg().clip_rule {
        style::computed_values::clip_rule::T::Nonzero => FillRule::NonZero,
        style::computed_values::clip_rule::T::Evenodd => FillRule::EvenOdd,
    };
    FillParams { color, opacity, fill_rule }
}

/// Extract stroke parameters from computed style.
pub fn extract_stroke_params(style: &ComputedValues) -> StrokeParams {
    use style::computed_values::stroke_linecap::T as LineCapStyle;
    use style::computed_values::stroke_linejoin::T as LineJoinStyle;

    let isvg = style.get_inherited_svg();

    let color = resolve_svg_paint(&isvg.stroke, style);

    let width = match &isvg.stroke_width {
        SVGWidth::LengthPercentage(lp) => {
            lp.to_used_value(app_units::Au::from_f32_px(300.0))
                .to_f32_px()
        },
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
                Some(
                    v.iter()
                        .map(|lp| {
                            lp.to_used_value(app_units::Au::from_f32_px(300.0))
                                .to_f32_px()
                        })
                        .collect(),
                )
            }
        },
        SVGStrokeDashArray::ContextValue => None,
    };

    let dashoffset = match &isvg.stroke_dashoffset {
        style::values::computed::svg::SVGLength::LengthPercentage(lp) => {
            lp.to_used_value(app_units::Au::from_f32_px(300.0))
                .to_f32_px()
        },
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

/// Extract opacity from computed effects.
pub fn extract_opacity(style: &ComputedValues) -> f32 {
    style.get_effects().opacity
}

/// Extract rendering hints from computed style.
pub fn extract_render_hints(_style: &ComputedValues) -> RenderHints {
    // TODO: Extract vector_effect from style once the property is exposed
    // on the appropriate style struct (SVG or InheritedSVG).
    RenderHints::default()
}

/// Extract visibility from computed style.
pub fn extract_visibility(style: &ComputedValues) -> Visibility {
    use style::computed_values::visibility::T as VisibilityStyle;
    match style.get_inherited_box().visibility {
        VisibilityStyle::Visible => Visibility::Visible,
        VisibilityStyle::Hidden => Visibility::Hidden,
        VisibilityStyle::Collapse => Visibility::Collapse,
    }
}

/// Extract node effects (transform, clip-path, mask) from DOM attributes.
pub fn extract_effects(get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Box<NodeEffects>> {
    let transform = get_attr("transform")
        .and_then(|s| crate::transform::parse_transform(&s));

    let clip_path = get_attr("clip-path")
        .map(|s| s.to_string());

    let mask = get_attr("mask")
        .map(|s| s.to_string());

    if transform.is_some() || clip_path.is_some() || mask.is_some() {
        Some(Box::new(NodeEffects { transform, clip_path, mask }))
    } else {
        None
    }
}

/// Extract geometry for a given SVG tag by reading DOM attributes.
pub fn extract_geometry(tag: &SvgTag, get_attr: &dyn Fn(&str) -> Option<String>) -> Option<Box<Geometry>> {
    match tag {
        SvgTag::Shape(Geometry::Rect { .. }) => Some(Box::new(Geometry::Rect {
            x: get_attr("x").and_then(|s| SvgLength::parse(&s)),
            y: get_attr("y").and_then(|s| SvgLength::parse(&s)),
            width: get_attr("width").and_then(|s| SvgLength::parse(&s)),
            height: get_attr("height").and_then(|s| SvgLength::parse(&s)),
            rx: get_attr("rx").and_then(|s| SvgLength::parse(&s)),
            ry: get_attr("ry").and_then(|s| SvgLength::parse(&s)),
        })),
        SvgTag::Shape(Geometry::Circle { .. }) => Some(Box::new(Geometry::Circle {
            cx: get_attr("cx").and_then(|s| SvgLength::parse(&s)),
            cy: get_attr("cy").and_then(|s| SvgLength::parse(&s)),
            r: get_attr("r").and_then(|s| SvgLength::parse(&s)),
        })),
        SvgTag::Shape(Geometry::Ellipse { .. }) => Some(Box::new(Geometry::Ellipse {
            cx: get_attr("cx").and_then(|s| SvgLength::parse(&s)),
            cy: get_attr("cy").and_then(|s| SvgLength::parse(&s)),
            rx: get_attr("rx").and_then(|s| SvgLength::parse(&s)),
            ry: get_attr("ry").and_then(|s| SvgLength::parse(&s)),
        })),
        SvgTag::Shape(Geometry::Line { .. }) => Some(Box::new(Geometry::Line {
            x1: get_attr("x1").and_then(|s| SvgLength::parse(&s)),
            y1: get_attr("y1").and_then(|s| SvgLength::parse(&s)),
            x2: get_attr("x2").and_then(|s| SvgLength::parse(&s)),
            y2: get_attr("y2").and_then(|s| SvgLength::parse(&s)),
        })),
        SvgTag::Shape(Geometry::Polygon { .. }) => {
            let pts = get_attr("points").and_then(|s| points::parse_points(&s));
            match pts {
                Some(p) => Some(Box::new(Geometry::Polygon { points: p })),
                None => None,
            }
        },
        SvgTag::Shape(Geometry::Polyline { .. }) => {
            let pts = get_attr("points").and_then(|s| points::parse_points(&s));
            match pts {
                Some(p) => Some(Box::new(Geometry::Polyline { points: p })),
                None => None,
            }
        },
        SvgTag::Shape(Geometry::Path { .. }) => {
            let p = get_attr("d").and_then(|s| path::parse_path(&s));
            match p {
                Some(p) => Some(Box::new(Geometry::Path { path: p })),
                None => None,
            }
        },
        _ => None,
    }
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
        },
        SVGPaintKind::None => None,
        _ => None, // PaintServer / ContextFill / ContextStroke — Phase 2
    }
}
