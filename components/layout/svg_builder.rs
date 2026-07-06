/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — bridges Servo's DOM and style system
//! with the SVG engine's render tree types.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use style::values::computed::basic_shape::ClipPath;
use style::values::computed::svg::{SVGOpacity, SVGStrokeDashArray, SVGPaint, SVGPaintKind};
use style::values::computed::svg::VectorEffect as StyloVectorEffect;
use style::values::generics::svg::SVGLength;
use style::values::specified::box_ as stylo_box;
use style::color::ColorSpace;

use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use crate::context::LayoutContext;

use svg_engine::render_tree::*;
use svg_engine::shapes::*;
use svg_engine::shapes::attr_parsers::{parse_length, parse_points};
use svg_engine::style::*;
use svg_engine::style::gradient::{GradientDef, PaintServer, parse_gradient_element};
use svg_engine::style::transform_ops::{parse_transform_str, TransformOp};
use svg_engine::render_tree::extract_viewbox;

use svgtypes::Color as SvgColor;
use web_atoms::ns;

// ======================= FromComputedValues Trait =======================

pub trait FromComputedValues: Sized {
    type Input;
    fn from_computed_values(values: &Self::Input) -> Option<Self>;
}

// ======================= ResolvedPaint =======================

enum ResolvedPaint {
    Color(SvgColor),
    PaintServer(String),
    None,
}

fn resolve_svg_paint(svg_paint: &SVGPaint, computed_values: &style::properties::ComputedValues) -> ResolvedPaint {
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
        // Map CSS visibility → svg_engine visibility
        let svg_visibility = match values.get_inherited_box().visibility {
            style::computed_values::visibility::T::Visible => Visibility::Visible,
            style::computed_values::visibility::T::Hidden => Visibility::Hidden,
            style::computed_values::visibility::T::Collapse => Visibility::Collapse,
        };

        // Map CSS display → svg_engine display
        let display = values.get_box().display;
        let svg_display = if display.outside() == stylo_box::DisplayOutside::None ||
                            display.inside() == stylo_box::DisplayInside::None
        {
            Display::None
        } else {
            Display::Inline
        };

        // Map vector-effect → svg_engine vector_effect hint
        let ve = values.get_svg().vector_effect;
        let vector_effect_hint = if ve.intersects(StyloVectorEffect::NON_SCALING_STROKE) {
            Some(VectorEffect::NonScalingStroke)
        } else {
            None
        };

        // Map clip-path → svg_engine effects
        let clip_path_ref = match &values.get_svg().clip_path {
            ClipPath::Url(style::url::ComputedUrl::Valid(u)) => u.fragment().map(|s| s.to_owned()),
            ClipPath::Url(style::url::ComputedUrl::Invalid(s)) => {
                let trimmed = s.trim_start_matches('#');
                if !trimmed.is_empty() { Some(trimmed.to_owned()) } else { None }
            },
            _ => None,
        };
        let effects = clip_path_ref.map(|ref_id| NodeEffects {
            clip_path: Some(ref_id.clone()),
            mask: None,
            filter: None,
        });

        // Map shape-rendering → svg_engine shape_rendering hint
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
            _ => None, // Auto → default behavior
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

/// Parse a CSS inline style attribute and extract specific CSS property values.
/// Handles formats like `"stroke: white; stroke-width: 4"` and `"fill: red"`.
fn parse_inline_style_prop(style_value: &str, prop_name: &str) -> Option<String> {
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

/// Build a NodeStyle by parsing SVG presentation attributes directly from the DOM,
/// falling back to inline CSS `style` attribute if needed.
/// Used for elements inside `<pattern>` where Servo's style system may not
/// compute styles.
fn build_style_from_attrs(element: &ServoLayoutElement) -> NodeStyle {
    // Read from presentation attributes first, then fall back to `style=""` attribute.
    let style_attr = get_attr(element, "style");

    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr.as_ref().and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    let fill_attr = read_attr("fill");
    let stroke_attr = read_attr("stroke");
    let fill_opacity = read_attr("fill-opacity")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let stroke_opacity = read_attr("stroke-opacity")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let stroke_width = read_attr("stroke-width")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(1.0);

    let fill = fill_attr.and_then(|v| {
        let ps = PaintServer::from_attr(&v);
        match ps {
            Some(PaintServer::Solid(c)) => Some(FillParams {
                color: Some(c),
                paint_server: None,
                opacity: fill_opacity,
                fill_rule: FillRule::NonZero,
            }),
            Some(PaintServer::Gradient(id)) => Some(FillParams {
                color: None,
                paint_server: Some(PaintServer::Gradient(id)),
                opacity: fill_opacity,
                fill_rule: FillRule::NonZero,
            }),
            Some(PaintServer::Pattern(_)) | None => None,
        }
    });

    let stroke = stroke_attr.and_then(|v| {
        let ps = PaintServer::from_attr(&v);
        match ps {
            Some(PaintServer::Solid(c)) => Some(StrokeParams {
                color: Some(c),
                paint_server: None,
                opacity: stroke_opacity,
                width: stroke_width,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                miter_limit: 4.0,
                dash_array: None,
                dash_offset: 0.0,
            }),
            Some(PaintServer::Gradient(id)) => Some(StrokeParams {
                color: None,
                paint_server: Some(PaintServer::Gradient(id)),
                opacity: stroke_opacity,
                width: stroke_width,
                line_cap: LineCap::Butt,
                line_join: LineJoin::Miter,
                miter_limit: 4.0,
                dash_array: None,
                dash_offset: 0.0,
            }),
            Some(PaintServer::Pattern(_)) | None => None,
        }
    });

    NodeStyle {
        visibility: Visibility::Visible,
        display: Display::Inline,
        transform: Vec::new(),
        fill,
        stroke,
        render_hints: None,
        effects: None,
        opacity: 1.0,
    }
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
                                        if let Some(op) = stop_elem.attribute_as_str(&ns!(), &local_name!("stop-opacity")) {
                                            attrs.push(("stop-opacity".to_owned(), op.to_string()));
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

// ======================= Clip Path Collection =======================

fn collect_clip_paths(node: ServoLayoutNode) -> HashMap<String, ClipPathDef> {
    let mut clip_paths = HashMap::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                for cp_child in defs_child.dom_children() {
                    if let Some(cp_elem) = cp_child.as_element() {
                        if cp_elem.local_name() == &local_name!("clipPath") {
                            let id = cp_elem.attribute_as_str(&ns!(), &local_name!("id"))
                                .map(|s| s.to_string());
                            let units = cp_elem.attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
                                .and_then(|s| match s.trim() {
                                    "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                                    _ => None,
                                })
                                .unwrap_or(ClipPathUnits::UserSpaceOnUse);
                            if let Some(ref id) = id {
                                let mut shapes = Vec::new();
                                for child_node in cp_child.dom_children() {
                                    if let Some(child_elem) = child_node.as_element() {
                                        let tag_name = child_elem.local_name().as_ref().to_owned();
                                        if let Some(shape) = build_shape(&child_elem, &tag_name) {
                                            shapes.push(shape);
                                        }
                                    }
                                }
                                if !shapes.is_empty() {
                                    clip_paths.insert(id.clone(), ClipPathDef { shapes, clip_path_units: units });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    clip_paths
}

// ======================= Pattern Collection =======================

fn collect_patterns<'dom>(node: ServoLayoutNode<'dom>, _context: &LayoutContext) -> HashMap<String, PatternDef> {
    let mut patterns = HashMap::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                for pat_child in defs_child.dom_children() {
                    if let Some(pat_elem) = pat_child.as_element() {
                        if pat_elem.local_name() == &local_name!("pattern") {
                            let id = pat_elem.attribute_as_str(&ns!(), &local_name!("id"))
                                .map(|s| s.to_string());
                            if let Some(ref id) = id {
                                let parse_attr = |attr: &str, default: f32| -> f32 {
                                    pat_elem.attribute_as_str(&ns!(), &LocalName::from(attr))
                                        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
                                        .unwrap_or(default)
                                };
                                let width = parse_attr("width", 0.0);
                                let height = parse_attr("height", 0.0);
                                let x = parse_attr("x", 0.0);
                                let y = parse_attr("y", 0.0);
                                let pattern_units = pat_elem.attribute_as_str(&ns!(), &local_name!("patternUnits"))
                                    .and_then(|s| match s.trim() {
                                        "objectBoundingBox" => Some(PatternUnits::ObjectBoundingBox),
                                        _ => None,
                                    })
                                    .unwrap_or(PatternUnits::UserSpaceOnUse);
                                let pattern_content_units = pat_elem.attribute_as_str(&ns!(), &local_name!("patternContentUnits"))
                                    .and_then(|s| match s.trim() {
                                        "objectBoundingBox" => Some(PatternContentUnits::ObjectBoundingBox),
                                        _ => None,
                                    })
                                    .unwrap_or(PatternContentUnits::UserSpaceOnUse);
                                if width > 0.0 && height > 0.0 {
                                    let mut shapes = Vec::new();
                                    for child_node in pat_child.dom_children() {
                                        if let Some(child_elem) = child_node.as_element() {
                                            let tag_name = child_elem.local_name().as_ref().to_owned();
                                            if let Some(shape) = build_shape(&child_elem, &tag_name) {
                                                let style = build_style_from_attrs(&child_elem);
                                                shapes.push((shape, style));
                                            }
                                        }
                                    }
                                    if !shapes.is_empty() {
                                        patterns.insert(id.clone(), PatternDef {
                                            width, height, x, y,
                                            pattern_units, pattern_content_units,
                                            shapes,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    patterns
}

// ======================= Mask Collection =======================

fn collect_masks(node: ServoLayoutNode) -> HashMap<String, MaskDef> {
    let mut masks = HashMap::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                for m_child in defs_child.dom_children() {
                    if let Some(m_elem) = m_child.as_element() {
                        if m_elem.local_name() == &local_name!("mask") {
                            let id = m_elem.attribute_as_str(&ns!(), &local_name!("id"))
                                .map(|s| s.to_string());
                            if let Some(ref id) = id {
                                let mut shapes = Vec::new();
                                for child_node in m_child.dom_children() {
                                    if let Some(child_elem) = child_node.as_element() {
                                        let tag_name = child_elem.local_name().as_ref().to_owned();
                                        if let Some(shape) = build_shape(&child_elem, &tag_name) {
                                            let style = build_style_from_attrs(&child_elem);
                                            shapes.push((shape, style));
                                        }
                                    }
                                }
                                if !shapes.is_empty() {
                                    masks.insert(id.clone(), MaskDef { shapes });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    masks
}

// ======================= Filter Collection =======================

fn collect_filters(node: ServoLayoutNode) -> HashMap<String, FilterDef> {
    let mut filters = HashMap::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                for f_child in defs_child.dom_children() {
                    if let Some(f_elem) = f_child.as_element() {
                        if f_elem.local_name() == &local_name!("filter") {
                            let id = f_elem.attribute_as_str(&ns!(), &local_name!("id"))
                                .map(|s| s.to_string());
                            if let Some(ref id) = id {
                                let get_attr = |name: &str| -> f32 {
                                    f_elem.attribute_as_str(&ns!(), &LocalName::from(name))
                                        .and_then(|v| v.parse::<f32>().ok())
                                        .unwrap_or(0.0)
                                };
                                let x = get_attr("x");
                                let y = get_attr("y");
                                let width = get_attr("width");
                                let height = get_attr("height");

                                let mut primitives = Vec::new();
                                for prim_child in f_child.dom_children() {
                                    if let Some(prim_elem) = prim_child.as_element() {
                                        let prim_name = prim_elem.local_name().as_ref().to_owned();
                                        match prim_name.as_str() {
                                            "feGaussianBlur" => {
                                                let sd = prim_elem.attribute_as_str(&ns!(), &LocalName::from("stdDeviation"))
                                                    .and_then(|v| v.parse::<f32>().ok())
                                                    .unwrap_or(0.0);
                                                primitives.push(FilterPrimitive::GaussianBlur(sd, sd));
                                            },
                                            "feDropShadow" => {
                                                let dx = prim_elem.attribute_as_str(&ns!(), &LocalName::from("dx"))
                                                    .and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                                                let dy = prim_elem.attribute_as_str(&ns!(), &LocalName::from("dy"))
                                                    .and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                                                let sd = prim_elem.attribute_as_str(&ns!(), &LocalName::from("stdDeviation"))
                                                    .and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                                                primitives.push(FilterPrimitive::DropShadow(dx, dy, sd, 0.0, 0.0, 0.0, 0.5));
                                            },
                                            "feColorMatrix" => {
                                                // Only handle type="matrix" with 20 values.
                                                if let Some(type_val) = prim_elem.attribute_as_str(&ns!(), &LocalName::from("type")) {
                                                    if type_val.trim() == "matrix" {
                                                        if let Some(val_str) = prim_elem.attribute_as_str(&ns!(), &LocalName::from("values")) {
                                                            let mut matrix = [0.0f32; 20];
                                                            let vals: Vec<f32> = val_str.split_whitespace()
                                                                .filter_map(|v| v.parse::<f32>().ok()).collect();
                                                            for (i, v) in vals.iter().enumerate().take(20) {
                                                                matrix[i] = *v;
                                                            }
                                                            primitives.push(FilterPrimitive::ColorMatrix(matrix));
                                                        }
                                                    }
                                                }
                                            },
                                            _ => {},
                                        }
                                    }
                                }
                                if !primitives.is_empty() {
                                    filters.insert(id.clone(), FilterDef { primitives, x, y, width, height });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    filters
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

/// Recursively search the SVG DOM subtree for an element by its `id`.
fn find_element_by_id<'dom>(node: ServoLayoutNode<'dom>, target_id: &str) -> Option<ServoLayoutNode<'dom>> {
    if let Some(element) = node.as_element() {
        if let Some(id) = element.attribute_as_str(&ns!(), &local_name!("id")) {
            if id == target_id {
                return Some(node);
            }
        }
    }
    for child in node.dom_children() {
        if child.as_element().is_some() {
            if let Some(found) = find_element_by_id(child, target_id) {
                return Some(found);
            }
        }
    }
    None
}

/// Apply `<use>`'s computed style to the root of a cloned subtree.
///
/// For the root render node, if its corresponding DOM element did NOT have an
/// explicit presentation attribute for `fill` or `stroke`, override it with
/// the `<use>` element's computed value (per SVG 2 shadow tree spec).
///
/// Children are NOT overridden — they get their styles from their own DOM
/// position via Stylo's cascade (e.g. a child of `<g fill="crimson">`
/// already inherits crimson correctly).
fn apply_use_style_to_root<'dom>(
    render_node: &mut SvgRenderNode,
    dom_node: ServoLayoutNode<'dom>,
    use_style: &NodeStyle,
) {
    if let Some(element) = dom_node.as_element() {
        if let Some(use_fill) = &use_style.fill {
            if get_attr(&element, "fill").is_none() {
                render_node.style.fill = Some(use_fill.clone());
            }
        }
        if let Some(use_stroke) = &use_style.stroke {
            if get_attr(&element, "stroke").is_none() {
                render_node.style.stroke = Some(use_stroke.clone());
            }
        }
    }
}

/// Build children for a `<use>` element — resolves the href, finds the target
/// element in the SVG DOM, and builds its render subtree.
///
/// Applies x/y offset as a `translate` transform on the cloned content.
/// Tracks `resolving` ids to detect and break circular references.
fn build_use_children<'dom>(
    element: &ServoLayoutElement<'dom>,
    context: &LayoutContext,
    root_node: ServoLayoutNode<'dom>,
    use_style: &NodeStyle,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    // Read href / xlink:href attribute.
    let href_id = get_attr(element, "href")
        .or_else(|| get_attr(element, "xlink:href"))
        .and_then(|v| {
            let trimmed = v.trim_start_matches('#');
            if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
        });
    let Some(ref_id) = href_id else { return vec![] };

    // Cycle detection — skip if we're already resolving this id.
    if !resolving.insert(ref_id.clone()) {
        return vec![]; // circular reference, skip
    }

    // Find the target element in the full SVG DOM tree.
    let mut result = match find_element_by_id(root_node, &ref_id) {
        Some(target) => {
            // Build the target's render subtree.
            let Some(mut node) = build_svg_render_node(target, context, root_node, resolving) else {
                resolving.remove(&ref_id);
                return vec![];
            };

            // Apply <use>'s computed style to the cloned root node
            apply_use_style_to_root(&mut node, target, use_style);

            vec![node]
        },
        None => vec![],
    };

    resolving.remove(&ref_id);

    // Apply x/y offset as a translate transform on the cloned content.
    let x = get_attr(element, "x").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(0.0);
    let y = get_attr(element, "y").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(0.0);
    if x != 0.0 || y != 0.0 {
        for child in &mut result {
            if child.style.transform.is_empty() {
                child.style.transform = vec![TransformOp::Translate(x, y)];
            } else {
                child.style.transform.insert(0, TransformOp::Translate(x, y));
            }
        }
    }

    result
}

fn build_svg_render_node<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
    root_node: ServoLayoutNode<'dom>,
    resolving: &mut HashSet<String>,
) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let tag = build_tag(&element)?;
    let style = build_style(node, context);
    let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string());
    let children = match &tag {
        SvgTag::Container(Container::Use) => {
            build_use_children(&element, context, root_node, &style, resolving)
        },
        _ => {
            node.dom_children()
                .filter_map(|child| build_svg_render_node(child, context, root_node, resolving))
                .collect()
        },
    };
    // Check for SVG `mask` and `filter` attributes, store references in effects.
    let mut style = style;
    if let Some(mask_val) = get_attr(&element, "mask") {
        let trimmed = mask_val.trim_start_matches("url(#").trim_end_matches(')');
        if !trimmed.is_empty() {
            match &mut style.effects {
                Some(e) => e.mask = Some(trimmed.to_owned()),
                None => style.effects = Some(NodeEffects { clip_path: None, mask: Some(trimmed.to_owned()), filter: None }),
            }
        }
    }
    if let Some(filter_val) = get_attr(&element, "filter") {
        let trimmed = filter_val.trim_start_matches("url(#").trim_end_matches(')');
        if !trimmed.is_empty() {
            match &mut style.effects {
                Some(e) => e.filter = Some(trimmed.to_owned()),
                None => style.effects = Some(NodeEffects { clip_path: None, mask: None, filter: Some(trimmed.to_owned()) }),
            }
        }
    }
    Some(SvgRenderNode { id, tag, style, children })
}

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(node: ServoLayoutNode<'dom>, context: &LayoutContext) -> Option<Arc<SvgRenderTree>> {
    let root = build_svg_render_node(node, context, node, &mut HashSet::new())?;
    let viewport = extract_viewport_info(node);
    let gradients = collect_gradients(node);
    let clip_paths = collect_clip_paths(node);
    let patterns = collect_patterns(node, context);
    let masks = collect_masks(node);
    let filters = collect_filters(node);
    let mut tree = SvgRenderTree { root, viewport, gradients, clip_paths, patterns, masks, filters };
    // Post-process: convert PaintServer::Gradient references to PaintServer::Pattern
    // when the referenced ID is actually a pattern definition.
    fixup_paint_servers(&mut tree.root, &tree.patterns);
    Some(Arc::new(tree))
}

/// Walk the render tree and convert gradient paint server references to pattern
/// references where the ID matches a collected pattern definition.
fn fixup_paint_servers(node: &mut SvgRenderNode, patterns: &HashMap<String, PatternDef>) {
    if let Some(ref mut fill) = node.style.fill {
        if let Some(PaintServer::Gradient(id)) = &fill.paint_server {
            if patterns.contains_key(id) {
                fill.paint_server = Some(PaintServer::Pattern(id.clone()));
            }
        }
    }
    if let Some(ref mut stroke) = node.style.stroke {
        if let Some(PaintServer::Gradient(id)) = &stroke.paint_server {
            if patterns.contains_key(id) {
                stroke.paint_server = Some(PaintServer::Pattern(id.clone()));
            }
        }
    }
    for child in &mut node.children {
        fixup_paint_servers(child, patterns);
    }
}
