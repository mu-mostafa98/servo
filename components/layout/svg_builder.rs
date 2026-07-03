/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — bridges Servo's DOM and style system
//! with the SVG engine's render tree types.

use std::collections::HashMap;
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use style::values::computed::svg::{SVGOpacity, SVGStrokeDashArray, SVGPaint, SVGPaintKind};
use style::values::generics::svg::SVGLength;
use style::color::ColorSpace;
use webrender_api::ColorF;

use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use crate::context::LayoutContext;

use svg_engine::render_tree::*;
use svg_engine::shapes::*;
use svg_engine::shapes::attr_parsers::{parse_length, parse_points};
use svg_engine::style::*;
use svg_engine::style::gradient::{GradientDef, PaintServer, parse_gradient_element};
use svg_engine::style::transform_ops::parse_transform_str;
use svg_engine::render_tree::extract_viewbox;

use web_atoms::ns;

// ======================= FromComputedValues Trait =======================

pub trait FromComputedValues: Sized {
    type Input;
    fn from_computed_values(values: &Self::Input) -> Option<Self>;
}

// ======================= ResolvedPaint =======================

enum ResolvedPaint {
    Color(ColorF),
    PaintServer(String),
    None,
}

fn resolve_svg_paint(svg_paint: &SVGPaint, computed_values: &style::properties::ComputedValues) -> ResolvedPaint {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed_values.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            ResolvedPaint::Color(ColorF::new(
                srgb.components.0.clamp(0.0, 1.0),
                srgb.components.1.clamp(0.0, 1.0),
                srgb.components.2.clamp(0.0, 1.0),
                srgb.alpha,
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
    type Input = style::properties::ComputedValues;

    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.fill, values);
        let opacity = match inherited_svg.fill_opacity {
            SVGOpacity::Opacity(opacity) => opacity, _ => 1.0,
        };
        let fill_rule = match inherited_svg.fill_rule {
            style::computed_values::fill_rule::T::Nonzero => FillRule::NonZero,
            style::computed_values::fill_rule::T::Evenodd => FillRule::EvenOdd,
        };
        match paint {
            ResolvedPaint::Color(color) => Some(FillParams { color: Some(color), paint_server: None, opacity, fill_rule }),
            ResolvedPaint::PaintServer(id) => Some(FillParams { color: None, paint_server: Some(PaintServer::Gradient(id)), opacity, fill_rule }),
            ResolvedPaint::None => None,
        }
    }
}

impl FromComputedValues for StrokeParams {
    type Input = style::properties::ComputedValues;

    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        let inherited_svg = values.get_inherited_svg();
        let paint = resolve_svg_paint(&inherited_svg.stroke, values);
        let opacity = match inherited_svg.stroke_opacity {
            SVGOpacity::Opacity(opacity) => opacity, _ => 1.0,
        };
        let width = match &inherited_svg.stroke_width {
            SVGLength::LengthPercentage(nn_lp) => nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0),
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
                if vs.is_empty() { None } else { Some(vs.iter().map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0)).collect()) }
            },
            _ => None,
        };
        let dash_offset = match &inherited_svg.stroke_dashoffset {
            SVGLength::LengthPercentage(lp) => lp.to_length().map(|l| l.px()).unwrap_or(0.0),
            _ => 0.0,
        };
        if width <= 0.0 { return None; }
        match paint {
            ResolvedPaint::Color(color) => Some(StrokeParams { color: Some(color), paint_server: None, opacity, width, line_cap, line_join, miter_limit, dash_array, dash_offset }),
            ResolvedPaint::PaintServer(id) => Some(StrokeParams { color: None, paint_server: Some(PaintServer::Gradient(id)), opacity, width, line_cap, line_join, miter_limit, dash_array, dash_offset }),
            ResolvedPaint::None => None,
        }
    }
}

impl FromComputedValues for NodeStyle {
    type Input = style::properties::ComputedValues;

    fn from_computed_values(values: &style::properties::ComputedValues) -> Option<Self> {
        Some(NodeStyle {
            visibility: Visibility::Visible, display: Display::Inline, transform: Vec::new(),
            fill: FillParams::from_computed_values(values),
            stroke: StrokeParams::from_computed_values(values),
            render_hints: None, effects: None,
        })
    }
}

// ======================= Element helpers =======================

fn get_attr(element: &ServoLayoutElement, attr: &str) -> Option<String> {
    element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string())
}

// ======================= Style Construction =======================

fn build_style(
    node: ServoLayoutNode,
    context: &LayoutContext,
) -> NodeStyle {
    let element = node.as_element().unwrap();
    let mut style = match element.style_data() {
        Some(_) => {
            let computed = node.style(&context.style_context);
            NodeStyle::from_computed_values(&computed).unwrap_or_default()
        },
        None => NodeStyle::default(),
    };
    style.transform = parse_transform_str(&get_attr(&element, "transform").unwrap_or_default());
    style
}

// ======================= Shape Construction =======================

fn build_shape(element: &ServoLayoutElement, tag_name: &str) -> Option<Shape> {
    match tag_name {
        "rect" => {
            let w = parse_length("width", &|a| get_attr(element, a)).ok()?;
            let h = parse_length("height", &|a| get_attr(element, a)).ok()?;
            if w < 0.0 || h < 0.0 { return None; }
            Some(Shape::Rect(Rectangle { x: parse_length("x", &|a| get_attr(element, a)).unwrap_or(0.0), y: parse_length("y", &|a| get_attr(element, a)).unwrap_or(0.0), width: w, height: h, rx: parse_length("rx", &|a| get_attr(element, a)).ok(), ry: parse_length("ry", &|a| get_attr(element, a)).ok() }))
        },
        "circle" => {
            let r = parse_length("r", &|a| get_attr(element, a)).ok()?;
            Some(Shape::Circle(Circle { cx: parse_length("cx", &|a| get_attr(element, a)).unwrap_or(0.0), cy: parse_length("cy", &|a| get_attr(element, a)).unwrap_or(0.0), r }))
        },
        "ellipse" => {
            let rx = parse_length("rx", &|a| get_attr(element, a)).ok()?;
            let ry = parse_length("ry", &|a| get_attr(element, a)).ok()?;
            Some(Shape::Ellipse(Ellipse { cx: parse_length("cx", &|a| get_attr(element, a)).unwrap_or(0.0), cy: parse_length("cy", &|a| get_attr(element, a)).unwrap_or(0.0), rx, ry }))
        },
        "line" => Some(Shape::Line(Line {
            x1: parse_length("x1", &|a| get_attr(element, a)).unwrap_or(0.0),
            y1: parse_length("y1", &|a| get_attr(element, a)).unwrap_or(0.0),
            x2: parse_length("x2", &|a| get_attr(element, a)).unwrap_or(0.0),
            y2: parse_length("y2", &|a| get_attr(element, a)).unwrap_or(0.0),
        })),
        "polyline" => parse_points(&|a| get_attr(element, a)).ok().map(|pts| Shape::Polyline(Polyline { points: pts })),
        "polygon" => parse_points(&|a| get_attr(element, a)).ok().map(|pts| Shape::Polygon(Polygon { points: pts })),
        "path" => {
            let value = get_attr(element, "d")?;
            use kurbo::BezPath;
            BezPath::from_svg(&value).ok().map(|path| Shape::Path(Path { path }))
        },
        _ => None,
    }
}

// ======================= Tag Dispatch =======================

fn build_tag(element: &ServoLayoutElement) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        _ => build_shape(element, tag).map(SvgTag::Shape),
    }
}

// ======================= Gradient Collection =======================

fn collect_gradients(node: ServoLayoutNode) -> HashMap<String, GradientDef> {
    let mut gradients = HashMap::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                for grad_child in defs_child.dom_children() {
                    if let Some(grad_elem) = grad_child.as_element() {
                        let grad_name = grad_elem.local_name().as_ref().to_owned();
                        if grad_name == "linearGradient" || grad_name == "radialGradient" {
                            let mut stop_attrs: Vec<Vec<(String, String)>> = Vec::new();
                            for stop_child in grad_child.dom_children() {
                                if let Some(stop_elem) = stop_child.as_element() {
                                    if stop_elem.local_name() == &local_name!("stop") {
                                        let mut attrs: Vec<(String, String)> = Vec::new();
                                        if let Some(offset) = stop_elem.attribute_as_str(&ns!(), &local_name!("offset")) {
                                            attrs.push(("offset".to_owned(), offset.to_string()));
                                        }
                                        if let Some(color) = stop_elem.attribute_as_str(&ns!(), &local_name!("stop-color")) {
                                            attrs.push(("stop-color".to_owned(), color.to_string()));
                                        }
                                        if !attrs.is_empty() { stop_attrs.push(attrs); }
                                    }
                                }
                            }
                            let grad_get = |attr: &str| {
                                grad_elem.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string())
                            };
                            if let Ok(def) = parse_gradient_element(&grad_name, &grad_get, &stop_attrs) {
                                match &def {
                                    GradientDef::Linear(lg) => { gradients.insert(lg.id.clone(), def); },
                                    GradientDef::Radial(rg) => { gradients.insert(rg.id.clone(), def); },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    gradients
}

// ======================= Viewport Extraction =======================

fn extract_viewport_info(node: ServoLayoutNode) -> ViewportInfo {
    let element = node.as_element().unwrap();
    let get = |attr: &str| element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
    let svg_width = get("width").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(300.0);
    let svg_height = get("height").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(150.0);
    let view_box = get("viewBox").as_deref().and_then(extract_viewbox);
    ViewportInfo { width: svg_width, height: svg_height, view_box }
}

// ======================= Render Node & Tree Construction =======================

fn build_svg_render_node(node: ServoLayoutNode, context: &LayoutContext) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let tag = build_tag(&element)?;
    let style = build_style(node, context);
    let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string());
    let children = node.dom_children()
        .filter_map(|child| build_svg_render_node(child, context))
        .collect();
    Some(SvgRenderNode { id, tag, style, children })
}

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
pub(crate) fn build_svg_render_tree(node: ServoLayoutNode, context: &LayoutContext) -> Option<Arc<SvgRenderTree>> {
    let root = build_svg_render_node(node, context)?;
    let viewport = extract_viewport_info(node);
    let gradients = collect_gradients(node);
    Some(Arc::new(SvgRenderTree { root, viewport, gradients }))
}
