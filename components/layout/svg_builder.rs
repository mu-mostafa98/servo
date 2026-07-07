/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — bridges Servo's DOM and style system
//! with the SVG engine's render tree types.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode, LayoutNodeType};
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
            ResolvedPaint::None => {
                // SVG 2: fill defaults to currentColor when not explicitly set.
                // Explicit fill="none" (SVGPaintKind::None) → no fill.
                // Anything else (unset, unknown paint kind) → inherit currentColor.
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
            transform: Vec::new(), // populated by build_style
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

// ======================= SVG Inline CSS Support =======================

/// A simple mapping from class name to (property → value) parsed from
/// `<style>` elements inside an SVG subtree. Only supports class selectors
/// (`.foo { ... }`) with simple property:value declarations — exactly the
/// pattern used in the SVG test suite and most inline-SVG demos.
type CssClassRules = HashMap<String, HashMap<String, String>>;

/// Collect CSS class rules from all `<style>` elements inside the SVG DOM subtree.
/// Servo's CSS engine ordinarily processes only HTML-namespaced `<style>` elements;
/// `<style>` inside `<svg>` is created in the SVG namespace and its rules never
/// reach the stylesheet.  This function fills the gap for the SVG engine path.
fn collect_svg_css_rules<'dom>(root_node: ServoLayoutNode<'dom>) -> CssClassRules {
    let mut all_rules: CssClassRules = HashMap::new();
    // Walk the subtree looking for <style> elements (any namespace).
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

/// Extract the raw CSS text content from a `<style>` DOM element.
/// Iterates children to find Text nodes (the style element itself is
/// an Element, so calling text_content() on it directly would panic).
fn extract_style_text_content<'dom>(node: ServoLayoutNode<'dom>) -> Option<String> {
    let mut text = String::new();
    for child in node.dom_children() {
        if let Some(LayoutNodeType::Text) = child.type_id() {
            text.push_str(&child.text_content());
        }
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

/// Parse simple CSS class rules like:
///   `.fill-red { fill: #E74C3C; }`
///   `.my-class { fill: red; stroke: blue; opacity: 0.5; }`
///
/// Returns a map from class name to (property → value).
/// Only handles class selectors (`.name { ... }`). Other selectors are ignored.
fn parse_svg_class_rules(css_text: &str) -> CssClassRules {
    let mut rules: CssClassRules = HashMap::new();
    // Split on `}` to get individual rule blocks, then process each.
    for block in css_text.split('}') {
        let block = block.trim();
        if block.is_empty() { continue; }
        // Split on the first `{` to get selector and declarations.
        let mut parts = block.splitn(2, '{');
        let selector = parts.next().unwrap_or("").trim();
        let declarations = parts.next().unwrap_or("").trim();
        if selector.is_empty() || declarations.is_empty() { continue; }
        // Only handle class selectors: ".className"
        if !selector.starts_with('.') { continue; }
        let class_name = selector[1..].trim();
        if class_name.is_empty() || class_name.contains(' ') { continue; }
        let props = parse_svg_declarations(declarations);
        rules.insert(class_name.to_owned(), props);
    }
    rules
}

/// Parse a CSS declaration block string like `"fill: #E74C3C; stroke: blue"`
/// into a property→value map.
fn parse_svg_declarations(block: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for decl in block.split(';') {
        let decl = decl.trim();
        if decl.is_empty() { continue; }
        let mut parts = decl.splitn(2, ':');
        let name = parts.next().unwrap_or("").trim().to_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if name.is_empty() || value.is_empty() { continue; }
        props.insert(name, value.to_owned());
    }
    props
}

/// Apply collected CSS class rules to a NodeStyle based on the element's `class` attribute.
fn apply_css_class_rules(
    element: &ServoLayoutElement,
    css_rules: &CssClassRules,
    style: &mut NodeStyle,
) {
    let Some(class_attr) = get_attr(element, "class") else { return };
    for class_name in class_attr.split_whitespace() {
        let Some(props) = css_rules.get(class_name) else { continue };
        for (prop, value) in props {
            apply_css_property(style, prop, value);
        }
    }
}

/// Apply a single CSS property:value pair to a NodeStyle.
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
                            fill_rule: style.fill.as_ref().map(|f| f.fill_rule).unwrap_or(FillRule::NonZero),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.fill = Some(FillParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
                            fill_rule: style.fill.as_ref().map(|f| f.fill_rule).unwrap_or(FillRule::NonZero),
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
                            line_cap: style.stroke.as_ref().map(|s| s.line_cap).unwrap_or(LineCap::Butt),
                            line_join: style.stroke.as_ref().map(|s| s.line_join).unwrap_or(LineJoin::Miter),
                            miter_limit: style.stroke.as_ref().map(|s| s.miter_limit).unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style.stroke.as_ref().map(|s| s.dash_offset).unwrap_or(0.0),
                        });
                    },
                    PaintServer::Gradient(id) => {
                        style.stroke = Some(StrokeParams {
                            color: None,
                            paint_server: Some(PaintServer::Gradient(id)),
                            opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                            width: style.stroke.as_ref().map(|s| s.width).unwrap_or(1.0),
                            line_cap: style.stroke.as_ref().map(|s| s.line_cap).unwrap_or(LineCap::Butt),
                            line_join: style.stroke.as_ref().map(|s| s.line_join).unwrap_or(LineJoin::Miter),
                            miter_limit: style.stroke.as_ref().map(|s| s.miter_limit).unwrap_or(4.0),
                            dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
                            dash_offset: style.stroke.as_ref().map(|s| s.dash_offset).unwrap_or(0.0),
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
                if let Some(ref mut s) = style.stroke { s.width = w.max(0.0); }
            }
        },
        "stroke-opacity" => {
            if let Ok(op) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke { s.opacity = op.clamp(0.0, 1.0); }
            }
        },
        "stroke-linecap" => {
            let lc = match value {
                "round" => LineCap::Round,
                "square" => LineCap::Square,
                _ => LineCap::Butt,
            };
            if let Some(ref mut s) = style.stroke { s.line_cap = lc; }
        },
        "stroke-linejoin" => {
            let lj = match value {
                "round" => LineJoin::Round,
                "bevel" => LineJoin::Bevel,
                _ => LineJoin::Miter,
            };
            if let Some(ref mut s) = style.stroke { s.line_join = lj; }
        },
        "stroke-dasharray" => {
            if value != "none" {
                let dashes: Vec<f32> = value.split(',')
                    .filter_map(|v| v.trim().parse::<f32>().ok())
                    .collect();
                if !dashes.is_empty() {
                    if let Some(ref mut s) = style.stroke { s.dash_array = Some(dashes); }
                }
            } else {
                if let Some(ref mut s) = style.stroke { s.dash_array = None; }
            }
        },
        "stroke-dashoffset" => {
            if let Ok(off) = value.parse::<f32>() {
                if let Some(ref mut s) = style.stroke { s.dash_offset = off; }
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

// ======================= Style Construction =======================

fn build_style(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &CssClassRules,
) -> NodeStyle {
    let element = node.as_element().unwrap();

    // Get values from Servo's style system (handles CSS cascade, inheritance,
    // and class selectors).  When style_data() is None (e.g. SVG child elements
    // that never created a layout box), node.style() falls back to
    // default_computed_values which supplies the SVG property defaults.
    let computed = node.style(&context.style_context);
    let mut style = NodeStyle::from_computed_values(&computed).unwrap_or_default();

    // Read CSS 'transform' from computed style and merge with
    // the SVG 'transform' attribute.  CSS comes first (applied
    // before the attribute transform in the pipeline).
    let css_ops = css_transform_from_computed(&computed);
    let attr_ops = parse_transform_str(
        &get_attr(&element, "transform").unwrap_or_default(),
    );
    style.transform = [css_ops, attr_ops].concat();

    // Overlay presentation attributes onto the computed style.
    // Presentation attributes (e.g. fill="red") take precedence over
    // CSS cascade defaults but are overridden by inline style="" and
    // CSS class rules that the style system resolved.
    let attr_style = build_style_from_attrs(&element);
    if attr_style.fill.is_some() {
        style.fill = attr_style.fill;
    }
    if attr_style.stroke.is_some() {
        style.stroke = attr_style.stroke;
    }
    match attr_style.visibility {
        Visibility::Visible => {},
        _ => style.visibility = attr_style.visibility,
    }
    if (attr_style.opacity - 1.0).abs() > f32::EPSILON {
        style.opacity = attr_style.opacity;
    }

    // Apply CSS class rules from <style> elements inside the SVG subtree.
    // Servo's CSS engine does not process SVG-namespaced <style> elements,
    // so we handle common class selectors here as a fallback.
    apply_css_class_rules(&element, css_rules, &mut style);

    style
}

/// Extract the CSS `transform` property from computed values as
/// [`TransformOp`]s.  Returns an empty vec when the transform is identity.
fn css_transform_from_computed(
    values: &style::properties::ComputedValues,
) -> Vec<TransformOp> {
    let list = &values.get_box().transform;
    if list.0.is_empty() {
        return Vec::new();
    }
    convert_transform_operations(&list.0)
}

/// Convert stylo computed transform operations to SVG-style [`TransformOp`]s.
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
            // 3D or complex operations — skip (SVG doesn't support them).
            _ => {},
        }
    }
    result
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

    let fill = match fill_attr {
        Some(v) => {
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
                Some(PaintServer::Pattern(_)) => None,
                None => {
                    // fill="none" (or unparseable) — explicit none, keeps
                    // fill visible for inheritance but produces no draw
                    // commands (renderer checks fill.color / paint_server).
                    Some(FillParams {
                        color: None,
                        paint_server: None,
                        opacity: fill_opacity,
                        fill_rule: FillRule::NonZero,
                    })
                },
            }
        },
        None => None, // no fill attribute — inherit from parent
    };

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

    let svg_visibility = read_attr("visibility").map_or(Visibility::Visible, |v| match v.trim() {
        "hidden" | "collapse" => Visibility::Hidden,
        _ => Visibility::Visible,
    });

    NodeStyle {
        visibility: svg_visibility,
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

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

fn build_shape(element: &ServoLayoutElement, tag_name: &str) -> Option<Shape> {
    let fs = SVG_DEFAULT_FONT_SIZE;
    match tag_name {
        "rect" => {
            let w = parse_length("width", &|a| get_attr(element, a), fs).ok()?;
            let h = parse_length("height", &|a| get_attr(element, a), fs).ok()?;
            if w < 0.0 || h < 0.0 { return None; }
            Some(Shape::Rect(Rectangle { x: parse_length("x", &|a| get_attr(element, a), fs).unwrap_or(0.0), y: parse_length("y", &|a| get_attr(element, a), fs).unwrap_or(0.0), width: w, height: h, rx: parse_length("rx", &|a| get_attr(element, a), fs).ok(), ry: parse_length("ry", &|a| get_attr(element, a), fs).ok() }))
        },
        "circle" => {
            let r = parse_length("r", &|a| get_attr(element, a), fs).ok()?;
            Some(Shape::Circle(Circle { cx: parse_length("cx", &|a| get_attr(element, a), fs).unwrap_or(0.0), cy: parse_length("cy", &|a| get_attr(element, a), fs).unwrap_or(0.0), r }))
        },
        "ellipse" => {
            let rx = parse_length("rx", &|a| get_attr(element, a), fs).ok()?;
            let ry = parse_length("ry", &|a| get_attr(element, a), fs).ok()?;
            Some(Shape::Ellipse(Ellipse { cx: parse_length("cx", &|a| get_attr(element, a), fs).unwrap_or(0.0), cy: parse_length("cy", &|a| get_attr(element, a), fs).unwrap_or(0.0), rx, ry }))
        },
        "line" => Some(Shape::Line(Line {
            x1: parse_length("x1", &|a| get_attr(element, a), fs).unwrap_or(0.0),
            y1: parse_length("y1", &|a| get_attr(element, a), fs).unwrap_or(0.0),
            x2: parse_length("x2", &|a| get_attr(element, a), fs).unwrap_or(0.0),
            y2: parse_length("y2", &|a| get_attr(element, a), fs).unwrap_or(0.0),
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

/// Recursively search a DOM subtree for SVG elements with the given
/// local name.  Handles nested groups inside <defs>.
fn find_elements_by_tag<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &str,
    result: &mut Vec<ServoLayoutNode<'dom>>,
) {
    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == tag {
                result.push(child);
            }
            // Recurse into containers to handle nested <g> inside <defs>.
            let name = elem.local_name().as_ref();
            if name == "g" || name == "defs" || name == "svg" || name == "a" || name == "switch" {
                find_elements_by_tag(child, tag, result);
            }
        }
    }
}

fn collect_gradients(node: ServoLayoutNode) -> HashMap<String, GradientDef> {
    let mut gradients = HashMap::new();
    let mut all_grads = Vec::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                find_elements_by_tag(defs_child, "linearGradient", &mut all_grads);
                find_elements_by_tag(defs_child, "radialGradient", &mut all_grads);
            }
        }
    }
    for grad_node in all_grads {
        if let Some(grad_elem) = grad_node.as_element() {
            let grad_name = grad_elem.local_name().as_ref().to_owned();
            let mut stop_attrs: Vec<Vec<(String, String)>> = Vec::new();
            for stop_child in grad_node.dom_children() {
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
    gradients
}

fn collect_clip_paths(node: ServoLayoutNode) -> HashMap<String, ClipPathDef> {
    let mut clip_paths = HashMap::new();
    let mut all_cp = Vec::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                find_elements_by_tag(defs_child, "clipPath", &mut all_cp);
            }
        }
    }
    for cp_node in all_cp {
        if let Some(cp_elem) = cp_node.as_element() {
            let id = cp_elem.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string());
            let units = cp_elem.attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
                .and_then(|s| match s.trim() {
                    "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                    _ => None,
                })
                .unwrap_or(ClipPathUnits::UserSpaceOnUse);
            if let Some(ref id) = id {
                let mut shapes = Vec::new();
                for child_node in cp_node.dom_children() {
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
    clip_paths
}


// ======================= Pattern Collection =======================

fn collect_patterns<'dom>(node: ServoLayoutNode<'dom>, _context: &LayoutContext) -> HashMap<String, PatternDef> {
    let mut patterns = HashMap::new();
    let mut all_pat = Vec::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                find_elements_by_tag(defs_child, "pattern", &mut all_pat);
            }
        }
    }
    for pat_node in all_pat {
        if let Some(pat_elem) = pat_node.as_element() {
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
                    for child_node in pat_node.dom_children() {
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
    patterns
}

// ======================= Mask Collection =======================

fn collect_masks(node: ServoLayoutNode) -> HashMap<String, MaskDef> {
    let mut masks = HashMap::new();
    let mut all_mask = Vec::new();
    for defs_child in node.dom_children() {
        if let Some(defs_elem) = defs_child.as_element() {
            if defs_elem.local_name() == &local_name!("defs") {
                find_elements_by_tag(defs_child, "mask", &mut all_mask);
            }
        }
    }
    for mask_node in all_mask {
        if let Some(m_elem) = mask_node.as_element() {
            let id = m_elem.attribute_as_str(&ns!(), &local_name!("id"))
                .map(|s| s.to_string());
            if let Some(ref id) = id {
                let mut shapes = Vec::new();
                for child_node in mask_node.dom_children() {
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
    masks
}

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
                                let get = |attr: &str| f_elem.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
                                let get_float = |attr: &str, default: f32| -> f32 {
                                    get(attr).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default)
                                };
                                // Filter bounds defaults (-10%, -10%, 120%, 120%).
                                let x = get_float("x", -0.1);
                                let y = get_float("y", -0.1);
                                let width = get_float("width", 1.2);
                                let height = get_float("height", 1.2);

                                let mut primitives = Vec::new();
                                for prim_child in f_child.dom_children() {
                                    if let Some(prim_elem) = prim_child.as_element() {
                                        let pname = prim_elem.local_name().as_ref().to_owned();
                                        match pname.as_str() {
                                            "feGaussianBlur" => {
                                                let std_dev = get_float("stdDeviation", 0.0);
                                                primitives.push(FilterPrimitive::GaussianBlur(std_dev, std_dev));
                                            },
                                            "feDropShadow" => {
                                                let dx = get_float("dx", 2.0);
                                                let dy = get_float("dy", 2.0);
                                                let std_dev = get_float("stdDeviation", 2.0);
                                                primitives.push(FilterPrimitive::DropShadow(dx, dy, std_dev, 0.0, 0.0, 0.0, 0.5));
                                            },
                                            "feColorMatrix" => {
                                                // Simple grayscale via color matrix.
                                                let v = 1.0 / 3.0;
                                                primitives.push(FilterPrimitive::ColorMatrix([
                                                    v, v, v, 0.0, 0.0,
                                                    v, v, v, 0.0, 0.0,
                                                    v, v, v, 0.0, 0.0,
                                                    0.0, 0.0, 0.0, 1.0, 0.0,
                                                ]));
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

fn extract_viewport_info<'dom>(node: ServoLayoutNode<'dom>, _context: &LayoutContext) -> ViewportInfo {
    let element = node.as_element().unwrap();
    let get = |attr: &str| element.attribute_as_str(&ns!(), &LocalName::from(attr)).map(|s| s.to_string());
    let svg_width = get("width").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(300.0);
    let svg_height = get("height").and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).unwrap_or(150.0);
    let view_box = get("viewBox").as_deref().and_then(extract_viewbox);

    // Check if overflow was explicitly set to "visible".
    let overflow_visible = get("overflow")
        .or_else(|| {
            get("style").and_then(|s| parse_inline_style_prop(&s, "overflow"))
        })
        .map_or(false, |v| v.trim().eq_ignore_ascii_case("visible"));

    // preserveAspectRatio: only apply when explicitly set (backward compat).
    let aspect_ratio = get("preserveAspectRatio")
        .map(|v| parse_aspect_ratio(&v));

    ViewportInfo { width: svg_width, height: svg_height, view_box, overflow_visible, aspect_ratio }
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
        if let Some(found) = find_element_by_id(child, target_id) {
            return Some(found);
        }
    }
    None
}

fn build_svg_render_node<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
    root_node: ServoLayoutNode<'dom>,
    resolving: &mut HashSet<String>,
    css_rules: &CssClassRules,
) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let tag = build_tag(&element)?;
    let style = build_style(node, context, css_rules);
    let id = element.attribute_as_str(&ns!(), &local_name!("id")).map(|s| s.to_string());

    // Resolve children, handling <use> element references.
    let children = match &tag {
        SvgTag::Container(Container::Use) => {
            let ref_id = element.attribute_as_str(&ns!(), &local_name!("href"))
                .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
                .and_then(|href| {
                    let trimmed = href.trim_start_matches('#');
                    if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
                });
            match ref_id {
                Some(ref_id) if !resolving.contains(&ref_id) => {
                    resolving.insert(ref_id.clone());
                    let result = find_element_by_id(root_node, &ref_id)
                        .and_then(|target| build_svg_render_node(target, context, root_node, resolving, css_rules))
                        .map(|target_node| target_node.children)
                        .unwrap_or_default();
                    resolving.remove(&ref_id);
                    result
                },
                _ => vec![],
            }
        },
        _ => {
            node.dom_children()
                .filter_map(|child| build_svg_render_node(child, context, root_node, resolving, css_rules))
                .collect()
        },
    };

    Some(SvgRenderNode { id, tag, style, children })
}

/// Post-process the tree: convert PaintServer::Gradient refs to
/// PaintServer::Pattern when the referenced ID is a pattern definition.
fn fixup_paint_servers(node: &mut SvgRenderNode, patterns: &HashMap<String, PatternDef>) {
    // Collect IDs to fix up without holding borrows across mutation.
    let fix_fill = node.style.fill.as_ref()
        .and_then(|f| match f.paint_server {
            Some(PaintServer::Gradient(ref id)) if patterns.contains_key(id) => Some(id.clone()),
            _ => None,
        });
    if let Some(id) = fix_fill {
        if let Some(ref mut fill) = node.style.fill {
            fill.paint_server = Some(PaintServer::Pattern(id));
        }
    }
    let fix_stroke = node.style.stroke.as_ref()
        .and_then(|s| match s.paint_server {
            Some(PaintServer::Gradient(ref id)) if patterns.contains_key(id) => Some(id.clone()),
            _ => None,
        });
    if let Some(id) = fix_stroke {
        if let Some(ref mut stroke) = node.style.stroke {
            stroke.paint_server = Some(PaintServer::Pattern(id));
        }
    }
    for child in &mut node.children {
        fixup_paint_servers(child, patterns);
    }
}

/// Main entry point — builds a complete `SvgRenderTree` from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(node: ServoLayoutNode<'dom>, context: &LayoutContext) -> Option<Arc<SvgRenderTree>> {
    // Collect CSS class rules from <style> elements inside the SVG subtree
    // before building the render tree, so build_style can apply them.
    let css_rules = collect_svg_css_rules(node);
    let root = build_svg_render_node(node, context, node, &mut HashSet::new(), &css_rules)?;
    let viewport = extract_viewport_info(node, context);
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
