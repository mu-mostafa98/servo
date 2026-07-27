/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG style construction — bridges Servo's CSS computed values and
//! presentation attributes with the SVG engine's [`NodeStyle`] types.
//!
//! This module handles:
//! - Converting [`ComputedValues`] to [`FillParams`], [`StrokeParams`], [`NodeStyle`]
//! - Parsing SVG presentation attributes (fill, stroke, opacity, etc.)
//! - Merging CSS transform with SVG `transform` attribute

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::values::computed::Image as ComputedImage;
use style::values::computed::basic_shape::ClipPath;
use style::values::computed::svg::{
    SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray, VectorEffect as StyloVectorEffect,
};
use style::values::generics::svg::SVGLength;
use style::values::specified::box_ as stylo_box;
use svg_engine::style::gradient::PaintServer;
use svg_engine::style::transform_ops::{TransformOp, parse_transform_str};
use svg_engine::style::*;
use svgtypes::Color as SvgColor;
use web_atoms::ns;

use super::css::{CssClassRules, apply_css_class_rules};
use super::transforms::css_transform_from_computed;
use crate::context::LayoutContext;

// ======================= FromComputedValues Trait =======================

/// Bridge from Servo's [`ComputedValues`] to SVG engine types.
pub trait FromComputedValues: Sized {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self>;
}

// ======================= ResolvedPaint =======================

enum ResolvedPaint {
    Color(SvgColor),
    PaintServer(String),
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
        SVGPaintKind::PaintServer(url) => {
            match url {
                style::url::ComputedUrl::Valid(u) => {
                    if let Some(fragment) = u.fragment() {
                        return ResolvedPaint::PaintServer(fragment.to_owned());
                    }
                },
                style::url::ComputedUrl::Invalid(s) => {
                    let trimmed = s.trim_start_matches('#');
                    if !trimmed.is_empty() {
                        return ResolvedPaint::PaintServer(trimmed.to_owned());
                    }
                },
            }
            ResolvedPaint::None
        },
        _ => ResolvedPaint::None,
    }
}

// ======================= FromComputedValues impls =======================

impl FromComputedValues for FillParams {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.fill, values);
        let opacity = match inherited_svg.fill_opacity {
            SVGOpacity::Opacity(opacity) => opacity,
            _ => 1.0,
        };
        let fill_rule = match inherited_svg.fill_rule {
            style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
            style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
        };
        match paint {
            ResolvedPaint::Color(color) => Some(FillParams {
                color: Some(color),
                paint_server: None,
                opacity,
                fill_rule,
            }),
            ResolvedPaint::PaintServer(id) => Some(FillParams {
                color: None,
                paint_server: Some(PaintServer::Gradient(id)),
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
                        color: Some(SvgColor::new_rgba(
                            (srgb.components.0.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.components.1.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.components.2.clamp(0.0, 1.0) * 255.0) as u8,
                            (srgb.alpha.clamp(0.0, 1.0) * 255.0) as u8,
                        )),
                        paint_server: None,
                        opacity,
                        fill_rule,
                    })
                }
            },
        }
    }
}

impl FromComputedValues for StrokeParams {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.stroke, values);
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
                color: Some(color),
                paint_server: None,
                opacity,
                width,
                line_cap,
                line_join,
                miter_limit,
                dash_array,
                dash_offset,
            }),
            ResolvedPaint::PaintServer(id) => Some(StrokeParams {
                color: None,
                paint_server: Some(PaintServer::Gradient(id)),
                opacity,
                width,
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

impl FromComputedValues for NodeStyle {
    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let svg_visibility = match values.get_inherited_box().visibility {
            style::computed_values::visibility::T::Visible => Visibility::Visible,
            style::computed_values::visibility::T::Hidden |
            style::computed_values::visibility::T::Collapse => Visibility::Hidden,
        };
        let display = values.get_box().display;
        let svg_display = if display.outside() == stylo_box::DisplayOutside::None ||
            display.inside() == stylo_box::DisplayInside::None
        {
            Display::None
        } else {
            Display::Inline
        };

        let ve = values.get_svg().vector_effect;
        let vector_effect_hint = if ve.intersects(StyloVectorEffect::NON_SCALING_STROKE) {
            Some(VectorEffect::NonScalingStroke)
        } else {
            None
        };

        let clip_path_ref = match &values.get_svg().clip_path {
            ClipPath::Url(style::url::ComputedUrl::Valid(u)) => u.fragment().map(|s| s.to_owned()),
            ClipPath::Url(style::url::ComputedUrl::Invalid(s)) => {
                let trimmed = s.trim_start_matches('#');
                if !trimmed.is_empty() {
                    Some(trimmed.to_owned())
                } else {
                    None
                }
            },
            _ => None,
        };
        let mask_ref = values
            .get_svg()
            .mask_image
            .0
            .first()
            .and_then(|img| match img {
                ComputedImage::Url(style::url::ComputedUrl::Valid(u)) => {
                    u.fragment().map(|s| s.to_owned())
                },
                ComputedImage::Url(style::url::ComputedUrl::Invalid(s)) => {
                    let trimmed = s.trim_start_matches('#');
                    if !trimmed.is_empty() {
                        Some(trimmed.to_owned())
                    } else {
                        None
                    }
                },
                _ => None,
            });
        let effects = match (clip_path_ref, mask_ref) {
            (None, None) => None,
            (clip, mask) => Some(NodeEffects {
                clip_path: clip,
                mask,
                filter: None,
            }),
        };
        let sr = values.get_inherited_svg().shape_rendering;
        let shape_rendering_hint = match sr {
            style::computed_values::shape_rendering::T::Optimizespeed => {
                Some(ShapeRendering::OptimizeSpeed)
            },
            style::computed_values::shape_rendering::T::Crispedges => {
                Some(ShapeRendering::CrispEdges)
            },
            style::computed_values::shape_rendering::T::Geometricprecision => {
                Some(ShapeRendering::GeometricPrecision)
            },
            _ => None,
        };

        Some(NodeStyle {
            visibility: svg_visibility,
            display: svg_display,
            fill: FillParams::from_computed_values(values),
            stroke: StrokeParams::from_computed_values(values),
            render_hints: Some(RenderHints {
                vector_effect: vector_effect_hint,
                shape_rendering: shape_rendering_hint,
                color_rendering: None,
                color_interpolation: None,
                paint_order: None,
                text_rendering: None,
                image_rendering: None,
            }),
            effects,
            opacity: values.get_effects().opacity,
        })
    }
}

// ======================= Element helpers =======================

pub(crate) fn get_attr(element: &ServoLayoutElement, attr: &str) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .map(|s| s.to_string())
}

/// Extract the fragment from a `url(#fragment)` CSS/SVG URL value.
pub(crate) fn extract_url_fragment(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed.strip_prefix("url(") {
        let inner = inner.trim_end_matches(')').trim();
        inner.strip_prefix('#').map(|s| s.to_owned())
    } else {
        trimmed.strip_prefix('#').map(|s| s.to_owned())
    }
}

pub(crate) fn parse_inline_style_prop(style_value: &str, prop_name: &str) -> Option<String> {
    for part in style_value.split(';') {
        let mut parts = part.splitn(2, ':');
        let key = parts.next()?.trim();
        let val = parts.next()?.trim();
        if key.eq_ignore_ascii_case(prop_name) && !val.is_empty() {
            return Some(val.to_owned());
        }
    }
    None
}

// ======================= Presentation Attribute Merge =======================

fn apply_stroke_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
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
        color: None,
        paint_server: None,
        opacity: 1.0,
        width: 1.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        miter_limit: 4.0,
        dash_array: None,
        dash_offset: 0.0,
    });

    match PaintServer::from_attr(&stroke_value) {
        Some(PaintServer::Solid(c)) => {
            stroke.color = Some(c);
            stroke.paint_server = None;
        },
        Some(PaintServer::Gradient(id)) => {
            stroke.color = None;
            stroke.paint_server = Some(PaintServer::Gradient(id));
        },
        Some(PaintServer::Pattern(_)) => {
            stroke.color = None;
            stroke.paint_server = None;
        },
        None => {
            stroke.color = None;
            stroke.paint_server = None;
        },
    }
    if let Some(v) = read_attr("stroke-width") {
        stroke.width = v
            .trim_end_matches("px")
            .parse::<f32>()
            .unwrap_or(1.0)
            .max(0.0);
    }
    if let Some(v) = read_attr("stroke-opacity") {
        stroke.opacity = v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
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
            "round" => LineJoin::Round,
            "bevel" => LineJoin::Bevel,
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

fn apply_fill_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
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
        color: None,
        paint_server: None,
        opacity: 1.0,
        fill_rule: FillRule::NonZero,
    });
    match PaintServer::from_attr(&fill_value) {
        Some(PaintServer::Solid(c)) => {
            fill.color = Some(c);
            fill.paint_server = None;
        },
        Some(PaintServer::Gradient(id)) => {
            fill.color = None;
            fill.paint_server = Some(PaintServer::Gradient(id));
        },
        Some(PaintServer::Pattern(id)) => {
            fill.color = None;
            fill.paint_server = Some(PaintServer::Pattern(id));
        },
        None => {
            fill.color = None;
            fill.paint_server = None;
        },
    }
    if let Some(v) = read_attr("fill-opacity") {
        fill.opacity = v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
    }
    if let Some(v) = read_attr("fill-rule") {
        fill.fill_rule = match v.trim() {
            "evenodd" | "even-odd" => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
    }
}

// ======================= Render Hints from DOM =======================

fn apply_render_hints_from_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let hints = style.render_hints.get_or_insert_with(|| RenderHints {
        vector_effect: None,
        shape_rendering: None,
        color_rendering: None,
        color_interpolation: None,
        text_rendering: None,
        image_rendering: None,
        paint_order: None,
    });
    if let Some(val) = get_attr(element, "color-rendering") {
        hints.color_rendering = match val.trim() {
            "optimizeSpeed" => Some(ColorRendering::OptimizeSpeed),
            "optimizeQuality" => Some(ColorRendering::OptimizeQuality),
            _ => None,
        };
    }
    if let Some(val) = get_attr(element, "color-interpolation") {
        hints.color_interpolation = match val.trim() {
            "linearRGB" => Some(ColorInterpolation::LinearRGB),
            "sRGB" => Some(ColorInterpolation::Srgb),
            _ => None,
        };
    }
    if let Some(val) = get_attr(element, "paint-order") {
        hints.paint_order = match val.trim() {
            "stroke fill" | "stroke" => Some(PaintOrder::StrokeFill),
            "fill stroke" | "fill" | "normal" => Some(PaintOrder::FillStroke),
            _ => None,
        };
    }
}

// ======================= Main Style Construction =======================

pub(crate) fn build_style(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &CssClassRules,
) -> (NodeStyle, Vec<TransformOp>) {
    let element = node.as_element().unwrap();
    let (mut style, css_transform) = if element.style_data().is_some() {
        let computed = node.style(&context.style_context);
        let style = NodeStyle::from_computed_values(&computed).unwrap_or_default();
        (style, css_transform_from_computed(&computed))
    } else {
        (NodeStyle::default(), Vec::new())
    };
    let attr_ops = parse_transform_str(&get_attr(&element, "transform").unwrap_or_default());
    let transforms: Vec<TransformOp> = [css_transform, attr_ops].concat();
    apply_css_class_rules(&element, css_rules, &mut style);
    apply_presentation_attrs(&element, &mut style);
    apply_render_hints_from_attrs(&element, &mut style);
    (style, transforms)
}

fn apply_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    apply_stroke_presentation_attrs(element, style);
    apply_fill_presentation_attrs(element, style);

    let style_attr = get_attr(element, "style");
    let read_attr_or_inline = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };
    // visibility
    if let Some(v) = read_attr_or_inline("visibility") {
        match v.trim() {
            "hidden" | "collapse" => style.visibility = Visibility::Hidden,
            _ => {},
        }
    }
    // opacity
    if let Some(v) = read_attr_or_inline("opacity") {
        if let Ok(op) = v.parse::<f32>() {
            style.opacity = op.clamp(0.0, 1.0);
        }
    }
    // display
    if let Some(v) = read_attr_or_inline("display") {
        if v.trim().eq_ignore_ascii_case("none") {
            style.display = Display::None;
        }
    }

    apply_filter_attribute(&element, style);
}

/// Apply the `filter` attribute to a style's effects.
///
/// Filter URLs are not available via Stylo computed values in Servo builds
/// (the `Filter` type uses `Impossible` for its URL parameter), so we read
/// the DOM attribute directly. The attribute is already parsed as a
/// presentation attribute by `SVGElement::synthesize_presentational_hints`
/// but cannot round-trip through Stylo's computed-value types.
fn apply_filter_attribute(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let filter_ref = get_attr(element, "filter")
        .as_deref()
        .and_then(extract_url_fragment);
    if let Some(filter_id) = filter_ref {
        let existing = style.effects.take().unwrap_or(NodeEffects {
            clip_path: None,
            mask: None,
            filter: None,
        });
        style.effects = Some(NodeEffects {
            clip_path: existing.clip_path,
            mask: existing.mask,
            filter: Some(filter_id),
        });
    }
}

/// Build a NodeStyle for elements inside `<pattern>` and `<mask>` definitions.
pub(crate) fn build_style_from_attrs(node: ServoLayoutNode, context: &LayoutContext) -> NodeStyle {
    let element = node.as_element().unwrap();
    if element.style_data().is_some() {
        let computed = node.style(&context.style_context);
        NodeStyle::from_computed_values(&computed).unwrap_or_default()
    } else {
        let mut style = NodeStyle::default();
        apply_stroke_presentation_attrs(&element, &mut style);
        apply_fill_presentation_attrs(&element, &mut style);
        let read_attr_or_inline = |name: &str| -> Option<String> {
            get_attr(&element, name).or_else(|| {
                get_attr(&element, "style").and_then(|s| parse_inline_style_prop(&s, name))
            })
        };
        if let Some(v) = read_attr_or_inline("visibility") {
            match v.trim() {
                "hidden" | "collapse" => style.visibility = Visibility::Hidden,
                _ => {},
            }
        }
        if let Some(v) = read_attr_or_inline("opacity") {
            if let Ok(op) = v.parse::<f32>() {
                style.opacity = op.clamp(0.0, 1.0);
            }
        }
        if let Some(v) = read_attr_or_inline("display") {
            if v.trim().eq_ignore_ascii_case("none") {
                style.display = Display::None;
            }
        }
        style
    }
}
