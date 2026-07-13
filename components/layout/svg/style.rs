/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG style construction — bridges Servo's CSS computed values and
//! presentation attributes with the SVG engine's [`NodeStyle`] types.
//!
//! This module handles:
//! - Converting [`ComputedValues`] to [`FillParams`], [`StrokeParams`], [`NodeStyle`]
//! - Parsing SVG presentation attributes (fill, stroke, opacity, etc.)
//! - Collecting and applying inline `<style>` CSS class rules
//! - Merging CSS transform with SVG `transform` attribute

use std::collections::HashMap;

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode, LayoutNodeType};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::values::computed::basic_shape::ClipPath;
use style::values::computed::Image as ComputedImage;
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

        let ve = values.get_svg().vector_effect;
        let vector_effect_hint = if ve.intersects(StyloVectorEffect::NON_SCALING_STROKE) {
            Some(VectorEffect::NonScalingStroke)
        } else {
            None
        };

        // Extract clip-path from computed values (via `synthesize_presentational_hints`).
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

        // Extract mask-image from computed values (via `synthesize_presentational_hints`).
        let mask_ref = values.get_svg().mask_image.0.first().and_then(|img| match img {
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

        // Only allocate effects when at least one effect is set.
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
            transform: Vec::new(),
            fill: FillParams::from_computed_values(values),
            stroke: StrokeParams::from_computed_values(values),
            render_hints: Some(RenderHints {
                vector_effect: vector_effect_hint,
                shape_rendering: shape_rendering_hint,
                color_rendering: None,
                color_interpolation: None,
                text_rendering: None,
                image_rendering: None,
                paint_order: None,
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

// ======================= SVG Inline CSS Support =======================

/// A simple mapping from class name to (property → value) parsed from
/// `<style>` elements inside an SVG subtree.
type CssClassRules = HashMap<String, HashMap<String, String>>;

/// Collect CSS class rules from all `<style>` elements inside the SVG DOM subtree.
pub(crate) fn collect_svg_css_rules<'dom>(root_node: ServoLayoutNode<'dom>) -> CssClassRules {
    let mut all_rules: CssClassRules = HashMap::new();
    let mut stack: Vec<ServoLayoutNode<'dom>> = vec![root_node];
    while let Some(node) = stack.pop() {
        if let Some(element) = node.as_element() {
            if element.local_name().as_ref() == "style" {
                if let Some(css_text) = extract_style_text_content(node) {
                    let rules = parse_svg_class_rules(&css_text);
                    for (cls, props) in rules {
                        all_rules.entry(cls).or_default().extend(props);
                    }
                }
            }
        }
        for child in node.dom_children() {
            stack.push(child);
        }
    }
    all_rules
}

fn extract_style_text_content<'dom>(node: ServoLayoutNode<'dom>) -> Option<String> {
    let mut text = String::new();
    for child in node.dom_children() {
        if let Some(LayoutNodeType::Text) = child.type_id() {
            text.push_str(&child.text_content());
        }
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_svg_class_rules(css_text: &str) -> CssClassRules {
    let mut rules: CssClassRules = HashMap::new();
    for block in css_text.split('}') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut parts = block.splitn(2, '{');
        let selector = parts.next().unwrap_or("").trim();
        let declarations = parts.next().unwrap_or("").trim();
        if selector.is_empty() || declarations.is_empty() {
            continue;
        }
        if !selector.starts_with('.') {
            continue;
        }
        let class_name = selector[1..].trim();
        if class_name.is_empty() || class_name.contains(' ') {
            continue;
        }
        let props = parse_svg_declarations(declarations);
        rules.insert(class_name.to_owned(), props);
    }
    rules
}

fn parse_svg_declarations(block: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for decl in block.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let mut parts = decl.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim().to_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if name.is_empty() || value.is_empty() {
            continue;
        }
        props.insert(name, value.to_owned());
    }
    props
}

pub(crate) fn apply_css_class_rules(
    element: &ServoLayoutElement,
    css_rules: &CssClassRules,
    style: &mut NodeStyle,
) {
    let Some(class_attr) = get_attr(element, "class") else {
        return;
    };
    for class_name in class_attr.split_whitespace() {
        let Some(props) = css_rules.get(class_name) else {
            continue;
        };
        for (prop, value) in props {
            apply_css_property(style, prop, value);
        }
    }
}

fn apply_css_property(style: &mut NodeStyle, prop: &str, value: &str) {
    match prop {
        "fill" | "fill-color" => {
            if let Some(ps) = PaintServer::from_attr(value) {
                match ps {
                    PaintServer::Solid(c) => {
                        style.fill = Some(FillParams {
                            color: Some(c),
                            paint_server: None,
                            opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
                            fill_rule: style
                                .fill
                                .as_ref()
                                .map(|f| f.fill_rule)
                                .unwrap_or(FillRule::NonZero),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.fill = Some(FillParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
                            fill_rule: style
                                .fill
                                .as_ref()
                                .map(|f| f.fill_rule)
                                .unwrap_or(FillRule::NonZero),
                        });
                    },
                    PaintServer::Pattern(_) => {},
                }
            } else if value.eq_ignore_ascii_case("none") {
                style.fill = None;
            }
        },
        "fill-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut fill) = style.fill {
                    fill.opacity = op.clamp(0.0, 1.0);
                }
            }
        },
        "stroke" | "stroke-color" => {
            if let Some(ps) = PaintServer::from_attr(value) {
                match ps {
                    PaintServer::Solid(c) => {
                        style.stroke = Some(StrokeParams {
                            color: Some(c),
                            paint_server: None,
                            opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                            width: style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0),
                            line_cap: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_cap)
                                .unwrap_or(LineCap::Butt),
                            line_join: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_join)
                                .unwrap_or(LineJoin::Miter),
                            miter_limit: style
                                .stroke
                                .as_ref()
                                .map(|s| s.miter_limit)
                                .unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style
                                .stroke
                                .as_ref()
                                .map(|s| s.dash_offset)
                                .unwrap_or(0.0),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.stroke = Some(StrokeParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                            width: style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0),
                            line_cap: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_cap)
                                .unwrap_or(LineCap::Butt),
                            line_join: style
                                .stroke
                                .as_ref()
                                .map(|s| s.line_join)
                                .unwrap_or(LineJoin::Miter),
                            miter_limit: style
                                .stroke
                                .as_ref()
                                .map(|s| s.miter_limit)
                                .unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style
                                .stroke
                                .as_ref()
                                .map(|s| s.dash_offset)
                                .unwrap_or(0.0),
                        });
                    },
                    PaintServer::Pattern(_) => {},
                }
            } else if value.eq_ignore_ascii_case("none") {
                style.stroke = None;
            }
        },
        "stroke-width" => {
            if let Ok(w) = value.trim_end_matches("px").parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.width = w.max(0.0);
                }
            }
        },
        "stroke-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.opacity = op.clamp(0.0, 1.0);
                }
            }
        },
        "stroke-linecap" => {
            let lc = match value {
                "round" => LineCap::Round,
                "square" => LineCap::Square,
                _ => LineCap::Butt,
            };
            if let Some(ref mut s) = style.stroke {
                s.line_cap = lc;
            }
        },
        "stroke-linejoin" => {
            let lj = match value {
                "round" => LineJoin::Round,
                "bevel" => LineJoin::Bevel,
                _ => LineJoin::Miter,
            };
            if let Some(ref mut s) = style.stroke {
                s.line_join = lj;
            }
        },
        "stroke-dasharray" => {
            if value != "none" {
                let dashes: Vec<f32> = value
                    .split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                if !dashes.is_empty() {
                    if let Some(ref mut s) = style.stroke {
                        s.dash_array = Some(dashes);
                    }
                }
            } else {
                if let Some(ref mut s) = style.stroke {
                    s.dash_array = None;
                }
            }
        },
        "stroke-dashoffset" => {
            if let Ok(off) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke {
                    s.dash_offset = off;
                }
            }
        },
        "opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                style.opacity = op.clamp(0.0, 1.0);
            }
        },
        "visibility" => {
            style.visibility = match value {
                "hidden" | "collapse" => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        },
        _ => {},
    }
}

// ======================= Field-level Presentation Attribute Merge =======================

/// Apply stroke-related SVG presentation attributes to an existing style,
/// field by field.  Only overrides fields where the corresponding attribute
/// exists on the element — computed values for absent attributes are preserved.
fn apply_stroke_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let style_attr = get_attr(element, "style");

    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    // Resolve color/paint-server first (this determines if stroke exists at all).
    let stroke_color_or_server = read_attr("stroke");
    if stroke_color_or_server.is_none() {
        // No stroke attribute — skip.  Computed style's stroke is untouched.
        return;
    }
    let stroke_value = stroke_color_or_server.unwrap();
    if stroke_value.eq_ignore_ascii_case("none") {
        style.stroke = None;
        return;
    }

    // Ensure we have a StrokeParams to modify (take computed or create fresh).
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

    // Apply color/paint-server
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
            // Pattern stroke paint servers are valid per SVG spec but not
            // yet supported by the rendering pipeline.
            stroke.color = None;
            stroke.paint_server = None;
        },
        None => {
            // Unparseable — keep existing color/server, treat as "none" color
            stroke.color = None;
            stroke.paint_server = None;
        },
    }

    // stroke-width
    if let Some(v) = read_attr("stroke-width") {
        stroke.width = v
            .trim_end_matches("px")
            .parse::<f32>()
            .unwrap_or(1.0)
            .max(0.0);
    }

    // stroke-opacity
    if let Some(v) = read_attr("stroke-opacity") {
        stroke.opacity = v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0);
    }

    // stroke-linecap
    if let Some(v) = read_attr("stroke-linecap") {
        stroke.line_cap = match v.trim() {
            "round" => LineCap::Round,
            "square" => LineCap::Square,
            _ => LineCap::Butt,
        };
    }

    // stroke-linejoin
    if let Some(v) = read_attr("stroke-linejoin") {
        stroke.line_join = match v.trim() {
            "round" => LineJoin::Round,
            "bevel" => LineJoin::Bevel,
            _ => LineJoin::Miter,
        };
    }

    // stroke-miterlimit
    if let Some(v) = read_attr("stroke-miterlimit") {
        if let Ok(ml) = v.parse::<f32>() {
            stroke.miter_limit = ml;
        }
    }

    // stroke-dasharray
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

    // stroke-dashoffset
    if let Some(v) = read_attr("stroke-dashoffset") {
        stroke.dash_offset = v.trim_end_matches("px").parse::<f32>().unwrap_or(0.0);
    }
}

/// Apply fill-related SVG presentation attributes to an existing style,
/// field by field.
fn apply_fill_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let style_attr = get_attr(element, "style");

    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    let fill_value = read_attr("fill");
    if fill_value.is_none() {
        return; // No fill attribute — computed style untouched.
    }
    let fill_value = fill_value.unwrap();
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

// ======================= Style Construction =======================

pub(crate) fn build_style(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &CssClassRules,
) -> NodeStyle {
    let element = node.as_element().unwrap();

    let (mut style, css_transform) = if element.style_data().is_some() {
        let computed = node.style(&context.style_context);
        let style = NodeStyle::from_computed_values(&computed).unwrap_or_default();
        let css_transform = css_transform_from_computed(&computed);
        (style, css_transform)
    } else {
        (NodeStyle::default(), Vec::new())
    };

    let attr_ops = parse_transform_str(&get_attr(&element, "transform").unwrap_or_default());
    style.transform = [css_transform, attr_ops].concat();

    // Apply CSS class rules from inline `<style>` elements.
    apply_css_class_rules(&element, css_rules, &mut style);

    // `filter` is not available via Stylo computed values in Servo builds
    // (the Filter type uses `Impossible` for the URL type parameter), so
    // read it directly from the DOM attribute.
    let filter_ref = get_attr(&element, "filter")
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

    style
}

fn css_transform_from_computed(values: &style::properties::ComputedValues) -> Vec<TransformOp> {
    let list = &values.get_box().transform;
    if list.0.is_empty() {
        return Vec::new();
    }
    convert_transform_operations(&list.0)
}

fn convert_transform_operations(
    ops: &[style::values::computed::transform::TransformOperation],
) -> Vec<TransformOp> {
    use style::values::generics::transform::GenericTransformOperation::*;
    use style::values::generics::transform::ToAbsoluteLength;

    let mut result = Vec::new();
    for op in ops {
        match op {
            Rotate(angle) => {
                result.push(TransformOp::Rotate(angle.degrees(), 0.0, 0.0));
            },
            Translate(tx, ty) => {
                let px = ToAbsoluteLength::to_pixel_length(tx, None).unwrap_or(0.0);
                let py = ToAbsoluteLength::to_pixel_length(ty, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(px, py));
            },
            TranslateX(t) => {
                let px = ToAbsoluteLength::to_pixel_length(t, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(px, 0.0));
            },
            TranslateY(t) => {
                let py = ToAbsoluteLength::to_pixel_length(t, None).unwrap_or(0.0);
                result.push(TransformOp::Translate(0.0, py));
            },
            Scale(sx, sy) => {
                result.push(TransformOp::Scale(*sx, *sy));
            },
            ScaleX(s) => {
                result.push(TransformOp::Scale(*s, 1.0));
            },
            ScaleY(s) => {
                result.push(TransformOp::Scale(1.0, *s));
            },
            SkewX(a) => {
                result.push(TransformOp::SkewX(a.degrees()));
            },
            SkewY(a) => {
                result.push(TransformOp::SkewY(a.degrees()));
            },
            Matrix(m) => {
                result.push(TransformOp::Matrix([m.a, m.b, m.c, m.d, m.e, m.f]));
            },
            _ => {},
        }
    }
    result
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

/// Build a NodeStyle for elements inside `<pattern>` and `<mask>` definitions.
///
/// These elements are in the DOM tree and have resolved [`ComputedValues`] via
/// the CSS cascade (including presentation attributes from
/// [`SVGElement::synthesize_presentational_hints`]), so this function uses the
/// same cascade path as [`build_style`].
///
/// Falls back to reading DOM attributes directly only when the element has no
/// style data (e.g. unstyled foreign elements).
pub(crate) fn build_style_from_attrs(
    node: ServoLayoutNode,
    context: &LayoutContext,
) -> NodeStyle {
    let element = node.as_element().unwrap();

    if element.style_data().is_some() {
        // Use the cascade path — presentation attributes flow through
        // synthesize_presentational_hints → ComputedValues → FromComputedValues.
        let computed = node.style(&context.style_context);
        NodeStyle::from_computed_values(&computed).unwrap_or_default()
    } else {
        // Fallback: no computed values available — read DOM attributes directly.
        let mut style = NodeStyle::default();

        apply_stroke_presentation_attrs(&element, &mut style);
        apply_fill_presentation_attrs(&element, &mut style);

        let read_attr_or_inline = |name: &str| -> Option<String> {
            get_attr(&element, name).or_else(|| {
                get_attr(&element, "style")
                    .and_then(|s| parse_inline_style_prop(&s, name))
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
