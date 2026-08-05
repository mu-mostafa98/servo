/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — bridges Servo DOM with usvg types.
//!
//! The main entry point is [`build_svg_render_tree`], called from
//! [`crate::replaced`].

pub(crate) mod builder;
pub(crate) mod css;
pub(crate) mod defines;
pub(crate) mod geometry;
pub(crate) mod style;
pub(crate) mod transforms;
pub(crate) mod viewport;

use std::sync::Arc;

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use crate::style_ext::ComputedValuesExt;
use usvg::*;
use web_atoms::ns;

use crate::context::LayoutContext;

/// Gradient store — holds gradient definitions parsed from <defs> for
/// resolution of `url(#id)` references during node construction.
struct GradientStore {
    linear: Vec<Arc<LinearGradient>>,
    radial: Vec<Arc<RadialGradient>>,
}

impl GradientStore {
    fn lookup_linear(&self, id: &str) -> Option<&Arc<LinearGradient>> {
        self.linear.iter().find(|g| g.id() == id)
    }

    fn lookup_radial(&self, id: &str) -> Option<&Arc<RadialGradient>> {
        self.radial.iter().find(|g| g.id() == id)
    }
}

/// Build gradient definitions directly from the DOM — no XML serialization.
/// Iterates `<defs>` children looking for `<linearGradient>` and `<radialGradient>`
/// elements, reads their attributes, and constructs usvg types via public constructors.
fn build_gradients_from_dom<'dom>(root_node: ServoLayoutNode<'dom>) -> GradientStore {
    let mut linear = Vec::new();
    let mut radial = Vec::new();

    for child in root_node.dom_children() {
        let Some(elem) = child.as_element() else { continue };
        if elem.local_name().as_ref() != "defs" { continue }

        // Found a <defs> — iterate its children for gradient elements.
        for defs_child in child.dom_children() {
            let Some(grad_elem) = defs_child.as_element() else { continue };
            let tag = grad_elem.local_name().as_ref();
            match tag {
                "lineargradient" | "linearGradient" => {
                    if let Some(g) = build_linear_gradient_from_dom(&grad_elem, defs_child) {
                        linear.push(Arc::new(g));
                    }
                }
                "radialgradient" | "radialGradient" => {
                    if let Some(g) = build_radial_gradient_from_dom(&grad_elem, defs_child) {
                        radial.push(Arc::new(g));
                    }
                }
                _ => {}
            }
        }
    }

    GradientStore { linear, radial }
}

/// Read a `<linearGradient>` directly from the DOM.
fn build_linear_gradient_from_dom<'dom>(
    elem: &ServoLayoutElement<'dom>,
    node: ServoLayoutNode<'dom>,
) -> Option<LinearGradient> {
    let id = get_attr(elem, "id")?;
    let x1 = attr_f32(elem, "x1", 0.0);
    let y1 = attr_f32(elem, "y1", 0.0);
    let x2 = attr_f32(elem, "x2", 1.0); // SVG default: 100%
    let y2 = attr_f32(elem, "y2", 0.0);

    let units = match get_attr(elem, "gradientUnits").as_deref() {
        Some("userSpaceOnUse") => Units::UserSpaceOnUse,
        _ => Units::ObjectBoundingBox,
    };

    let spread_method = match get_attr(elem, "spreadMethod").as_deref() {
        Some("reflect") => SpreadMethod::Reflect,
        Some("repeat") => SpreadMethod::Repeat,
        _ => SpreadMethod::Pad,
    };

    let transform = parse_gradient_transform(
        &get_attr(elem, "gradientTransform").unwrap_or_default(),
    );

    let stops = build_stops_from_dom(node);

    LinearGradient::new(&id, units, transform, spread_method, stops, x1, y1, x2, y2)
}

/// Read a `<radialGradient>` directly from the DOM.
fn build_radial_gradient_from_dom<'dom>(
    elem: &ServoLayoutElement<'dom>,
    node: ServoLayoutNode<'dom>,
) -> Option<RadialGradient> {
    let id = get_attr(elem, "id")?;
    let cx = attr_f32(elem, "cx", 0.5); // SVG default: 50%
    let cy = attr_f32(elem, "cy", 0.5);
    // For ObjectBoundingBox: r defaults to 50%. For userSpaceOnUse: 50 (as a number).
    let r = attr_f32(elem, "r", 0.5);
    let fx = get_attr(elem, "fx").as_deref().and_then(parse_f32).unwrap_or(cx);
    let fy = get_attr(elem, "fy").as_deref().and_then(parse_f32).unwrap_or(cy);
    let fr = attr_f32(elem, "fr", 0.0);

    let units = match get_attr(elem, "gradientUnits").as_deref() {
        Some("userSpaceOnUse") => Units::UserSpaceOnUse,
        _ => Units::ObjectBoundingBox,
    };

    let spread_method = match get_attr(elem, "spreadMethod").as_deref() {
        Some("reflect") => SpreadMethod::Reflect,
        Some("repeat") => SpreadMethod::Repeat,
        _ => SpreadMethod::Pad,
    };

    let transform = parse_gradient_transform(
        &get_attr(elem, "gradientTransform").unwrap_or_default(),
    );

    let stops = build_stops_from_dom(node);

    RadialGradient::new(&id, units, transform, spread_method, stops, cx, cy, r, fx, fy, fr)
}

/// Read `<stop>` elements from a gradient element's DOM children.
fn build_stops_from_dom<'dom>(gradient_node: ServoLayoutNode<'dom>) -> Vec<Stop> {
    let mut stops = Vec::new();
    for child in gradient_node.dom_children() {
        let Some(elem) = child.as_element() else { continue };
        if elem.local_name().as_ref() != "stop" { continue }

        let offset_str = get_attr(&elem, "offset").unwrap_or_default();
        let offset = parse_stop_offset(&offset_str);

        let color = parse_stop_color(&elem);

        let opacity = get_attr(&elem, "stop-opacity")
            .as_deref()
            .and_then(parse_f32)
            .unwrap_or(1.0);

        if let (Some(o), Some(c)) = (offset, color) {
            if let Some(stop) = Stop::new(o, c, opacity) {
                stops.push(stop);
            }
        }
    }
    stops
}

/// Parse a stop offset which may be a percentage (0%-100%) or a number (0.0-1.0).
fn parse_stop_offset(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(percent) = s.strip_suffix('%') {
        percent.parse::<f32>().ok().map(|p| p / 100.0)
    } else {
        s.parse::<f32>().ok()
    }
}

/// Parse a stop-color value from a `<stop>` element.
fn parse_stop_color(elem: &ServoLayoutElement) -> Option<Color> {
    get_attr(elem, "stop-color")
        .as_deref()
        .and_then(parse_color)
        .or_else(|| Some(Color::black()))
}

/// Parse a `gradientTransform` attribute into a [`Transform`].
fn parse_gradient_transform(s: &str) -> Transform {
    if s.trim().is_empty() {
        return Transform::default();
    }
    // Simple transform parsing: translate, scale, rotate, matrix.
    // We parse basic SVG transform syntax manually.
    let s = s.trim();
    let mut t = Transform::default();
    for part in split_transforms(s) {
        t = t.pre_concat(parse_single_transform(&part));
    }
    t
}

/// Split a transform string by ')' boundaries.
fn split_transforms(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c == ')' {
            parts.push(s[start..=i].trim().to_string());
            start = i + 1;
        }
    }
    parts
}

/// Parse a single transform function like `rotate(45 0.5 0.5)` or `translate(10 20)`.
fn parse_single_transform(s: &str) -> Transform {
    let s = s.trim();
    if let Some(args) = s.strip_prefix("matrix(").and_then(|x| x.strip_suffix(')')) {
        let nums: Vec<f32> = args.split_whitespace().filter_map(|n| n.trim_end_matches(',').parse().ok()).collect();
        if nums.len() == 6 {
            return Transform::from_row(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]);
        }
    }
    if let Some(args) = s.strip_prefix("translate(").and_then(|x| x.strip_suffix(')')) {
        let nums: Vec<f32> = args.split_whitespace().filter_map(|n| n.trim_end_matches(',').parse().ok()).collect();
        if nums.len() == 2 {
            return Transform::from_translate(nums[0], nums[1]);
        } else if nums.len() == 1 {
            return Transform::from_translate(nums[0], 0.0);
        }
    }
    if let Some(args) = s.strip_prefix("scale(").and_then(|x| x.strip_suffix(')')) {
        let nums: Vec<f32> = args.split_whitespace().filter_map(|n| n.trim_end_matches(',').parse().ok()).collect();
        if nums.len() == 2 {
            return Transform::from_scale(nums[0], nums[1]);
        } else if nums.len() == 1 {
            return Transform::from_scale(nums[0], nums[0]);
        }
    }
    if let Some(args) = s.strip_prefix("rotate(").and_then(|x| x.strip_suffix(')')) {
        let nums: Vec<f32> = args.split_whitespace().filter_map(|n| n.trim_end_matches(',').parse().ok()).collect();
        if nums.len() == 1 {
            return Transform::from_rotate(nums[0]);
        } else if nums.len() == 3 {
            let t = Transform::from_translate(nums[1], nums[2]);
            let r = Transform::from_rotate(nums[0]);
            let t_inv = Transform::from_translate(-nums[1], -nums[2]);
            return t_inv.pre_concat(r).pre_concat(t);
        }
    }
    if let Some(args) = s.strip_prefix("skewX(").and_then(|x| x.strip_suffix(')')) {
        if let Ok(a) = args.trim().parse::<f32>() {
            return Transform::from_row(1.0, 0.0, a.to_radians().tan(), 1.0, 0.0, 0.0);
        }
    }
    if let Some(args) = s.strip_prefix("skewY(").and_then(|x| x.strip_suffix(')')) {
        if let Ok(a) = args.trim().parse::<f32>() {
            return Transform::from_row(1.0, a.to_radians().tan(), 0.0, 1.0, 0.0, 0.0);
        }
    }
    Transform::default()
}

/// Main entry point — builds a complete usvg::Tree from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(
    root_node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<usvg::Tree>> {
    let size = Size::from_wh(300.0, 150.0)?;
    let mut root = Group::new();

    // Extract gradient definitions directly from <defs> DOM children.
    // No XML serialization — we read attributes from the DOM and construct
    // usvg types using their public constructors.
    let gradients = build_gradients_from_dom(root_node);

    // Serialize <defs> for text element parsing (text layout requires usvg).
    let mut defs_xml = String::new();
    for child in root_node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == "defs" {
                defs_xml.push_str(&serialize_defs_subtree(child));
            }
        }
    }

    // Build the render tree (with gradient resolution available).
    for child in root_node.dom_children() {
        if let Some(node) = build_node(child, &defs_xml, &gradients, context) {
            root.push_child(node);
        }
    }

    let mut tree = Tree::new(size, root);

    // Push gradient definitions into the final tree.
    for lg in &gradients.linear { tree.push_linear_gradient(lg.clone()); }
    for rg in &gradients.radial { tree.push_radial_gradient(rg.clone()); }

    Some(Arc::new(tree))
}

fn build_node<'dom>(node: ServoLayoutNode<'dom>, defs_xml: &str, gradients: &GradientStore, context: &LayoutContext) -> Option<Node> {
    let element = node.as_element()?;
    let tag = element.local_name().as_ref().to_owned();

    let mut group = Group::new();

    match tag.as_str() {
        "svg" | "g" => {
            for child in node.dom_children() {
                if let Some(elem) = child.as_element() {
                    if elem.local_name().as_ref() == "defs" {
                        continue; // defs children not rendered
                    }
                }
                if let Some(n) = build_node(child, defs_xml, gradients, context) {
                    group.push_child(n);
                }
            }
            Some(Node::Group(Box::new(group)))
        }
        "defs" => None, // definitions collected at top level
        "rect" => build_rect(&element, gradients).map(|s| Node::SimpleShape(Box::new(s))),
        "circle" => build_circle(&element, gradients).map(|s| Node::SimpleShape(Box::new(s))),
        "ellipse" => build_ellipse(&element, gradients).map(|s| Node::SimpleShape(Box::new(s))),
        "line" => build_line(&element, gradients).map(|s| Node::SimpleShape(Box::new(s))),
        "path" => build_path_element(&element, gradients).map(|p| Node::Path(Box::new(p))),
        "polygon" => build_polygon_element(&element, gradients).map(|p| Node::Path(Box::new(p))),
        "polyline" => build_polyline_element(&element, gradients).map(|p| Node::Path(Box::new(p))),
        "text" => build_text_element(node, defs_xml),
        "image" | "img" => build_image_element(node),
        _ => None,
    }
}

// ======================= Helpers =======================

fn get_attr(element: &ServoLayoutElement, name: &str) -> Option<String> {
    // SVG namespace
    element
        .attribute_as_str(&ns!(svg), &LocalName::from(name))
        .or_else(|| element.attribute_as_str(&ns!(), &LocalName::from(name)))
        .map(|s| s.to_string())
}

fn parse_f32(val: &str) -> Option<f32> {
    val.trim_end_matches("px").parse::<f32>().ok()
}

fn attr_f32(element: &ServoLayoutElement, name: &str, default: f32) -> f32 {
    get_attr(element, name)
        .as_deref()
        .and_then(parse_f32)
        .unwrap_or(default)
}

fn attr_opt_f32(element: &ServoLayoutElement, name: &str) -> Option<f32> {
    get_attr(element, name).as_deref().and_then(parse_f32)
}

fn parse_color(val: &str) -> Option<Color> {
    // Minimal named colors
    let val = val.trim().to_lowercase();
    match val.as_str() {
        "red" => Some(Color::new_rgb(255, 0, 0)),
        "green" => Some(Color::new_rgb(0, 128, 0)),
        "blue" => Some(Color::new_rgb(0, 0, 255)),
        "black" => Some(Color::new_rgb(0, 0, 0)),
        "white" => Some(Color::new_rgb(255, 255, 255)),
        "yellow" => Some(Color::new_rgb(255, 255, 0)),
        "orange" => Some(Color::new_rgb(255, 165, 0)),
        "purple" => Some(Color::new_rgb(128, 0, 128)),
        "cyan" | "aqua" => Some(Color::new_rgb(0, 255, 255)),
        "lime" => Some(Color::new_rgb(0, 255, 0)),
        "pink" => Some(Color::new_rgb(255, 192, 203)),
        "teal" => Some(Color::new_rgb(0, 128, 128)),
        "coral" => Some(Color::new_rgb(255, 127, 80)),
        "gold" => Some(Color::new_rgb(255, 215, 0)),
        "dodgerblue" => Some(Color::new_rgb(30, 144, 255)),
        "hotpink" => Some(Color::new_rgb(255, 105, 180)),
        "gray" | "grey" => Some(Color::new_rgb(128, 128, 128)),
        _ if val.starts_with('#') => {
            let hex = &val[1..];
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color::new_rgb(r, g, b))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn build_fill(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<Fill> {
    let fill_str = get_attr(element, "fill")?;
    if fill_str == "none" {
        return None;
    }
    let opacity = get_attr(element, "fill-opacity")
        .as_deref()
        .and_then(parse_f32)
        .and_then(Opacity::new)
        .unwrap_or(Opacity::ONE);
    // Handle url(#id) references — look up gradient definitions.
    if fill_str.starts_with("url(#") {
        if let Some(id) = extract_url_id(&fill_str) {
            if let Some(lg) = gradients.lookup_linear(&id) {
                return Some(Fill::new(Paint::LinearGradient(lg.clone()), opacity, FillRule::NonZero));
            }
            if let Some(rg) = gradients.lookup_radial(&id) {
                return Some(Fill::new(Paint::RadialGradient(rg.clone()), opacity, FillRule::NonZero));
            }
        }
        // Gradient not found — fall back to black.
        return Some(Fill::new(Paint::Color(Color::new_rgb(0, 0, 0)), opacity, FillRule::NonZero));
    }
    let color = parse_color(&fill_str)?;
    Some(Fill::new(Paint::Color(color), opacity, FillRule::NonZero))
}

fn build_stroke(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<Stroke> {
    let stroke_str = get_attr(element, "stroke")?;
    if stroke_str == "none" {
        return None;
    }
    let width = attr_f32(element, "stroke-width", 1.0);
    let sw = StrokeWidth::new(width.max(0.01))?;
    let _opacity = get_attr(element, "stroke-opacity")
        .as_deref()
        .and_then(parse_f32)
        .and_then(Opacity::new)
        .unwrap_or(Opacity::ONE);
    // Handle url(#id) references — look up gradient definitions.
    if stroke_str.starts_with("url(#") {
        if let Some(id) = extract_url_id(&stroke_str) {
            if let Some(lg) = gradients.lookup_linear(&id) {
                return Some(Stroke::new(Paint::LinearGradient(lg.clone()), sw));
            }
            if let Some(rg) = gradients.lookup_radial(&id) {
                return Some(Stroke::new(Paint::RadialGradient(rg.clone()), sw));
            }
        }
        // Gradient not found — fall back to black.
        return Some(Stroke::new(Paint::Color(Color::new_rgb(0, 0, 0)), sw));
    }
    let color = parse_color(&stroke_str)?;
    Some(Stroke::new(Paint::Color(color), sw))
}

/// Extract the ID from `url(#some-id)`.
fn extract_url_id(url_str: &str) -> Option<String> {
    let trimmed = url_str.trim();
    if trimmed.starts_with("url(#") && trimmed.ends_with(')') {
        Some(trimmed[5..trimmed.len()-1].to_string())
    } else {
        None
    }
}

// ======================= Shape Builders =======================

fn build_rect(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<SimpleShape> {
    use SimpleShapeKind::Rect;
    // Use DOM attrs (Stylo computed value integration is a follow-up)
    let x = attr_f32(element, "x", 0.0);
    let y = attr_f32(element, "y", 0.0);
    let w = attr_f32(element, "width", 0.0);
    let h = attr_f32(element, "height", 0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let rx = attr_opt_f32(element, "rx");
    let ry = attr_opt_f32(element, "ry");
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Some(SimpleShape::new(
        Rect { x, y, width: w, height: h, rx, ry },
        fill, stroke, Transform::default(),
    ))
}

fn build_circle(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<SimpleShape> {
    use SimpleShapeKind::Circle;
    let cx = attr_f32(element, "cx", 0.0);
    let cy = attr_f32(element, "cy", 0.0);
    let r = attr_f32(element, "r", 0.0);
    if r <= 0.0 {
        return None;
    }
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Some(SimpleShape::new(
        Circle { cx, cy, r },
        fill, stroke, Transform::default(),
    ))
}

fn build_ellipse(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<SimpleShape> {
    use SimpleShapeKind::Ellipse;
    let cx = attr_f32(element, "cx", 0.0);
    let cy = attr_f32(element, "cy", 0.0);
    let rx = attr_f32(element, "rx", 0.0);
    let ry = attr_f32(element, "ry", 0.0);
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Some(SimpleShape::new(
        Ellipse { cx, cy, rx, ry },
        fill, stroke, Transform::default(),
    ))
}

fn build_line(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<SimpleShape> {
    use SimpleShapeKind::Line;
    let x1 = attr_f32(element, "x1", 0.0);
    let y1 = attr_f32(element, "y1", 0.0);
    let x2 = attr_f32(element, "x2", 0.0);
    let y2 = attr_f32(element, "y2", 0.0);
    let stroke = build_stroke(element, gradients)?;
    Some(SimpleShape::new(
        Line { x1, y1, x2, y2 },
        None, Some(stroke), Transform::default(),
    ))
}

// ======================= Complex Shape Builders =======================

fn build_path_element(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<Path> {
    let d = get_attr(element, "d")?;
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Path::from_d(&d, fill, stroke, Transform::default())
}

fn build_polygon_element(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<Path> {
    let points = get_attr(element, "points")?;
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Path::from_points(&points, true, fill, stroke, Transform::default())
}

fn build_polyline_element(element: &ServoLayoutElement, gradients: &GradientStore) -> Option<Path> {
    let points = get_attr(element, "points")?;
    let fill = build_fill(element, gradients);
    let stroke = build_stroke(element, gradients);
    Path::from_points(&points, false, fill, stroke, Transform::default())
}

fn build_text_element<'dom>(node: ServoLayoutNode<'dom>, defs_xml: &str) -> Option<Node> {
    let element = node.as_element()?;
    let inner = serialize_text_subtree(node);
    if inner.is_empty() {
        return None;
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600">{}{}</svg>"#,
        defs_xml, inner
    );
    let mut opt = usvg::Options::default();
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    opt.fontdb = Arc::new(db);
    let tree = usvg::Tree::from_str(&svg, &opt).ok()?;
    let mut group = Group::new();
    for child in tree.root().children() {
        extract_flattened(child, &mut group);
    }
    if group.has_children() {
        Some(Node::Group(Box::new(group)))
    } else {
        None
    }
}

fn serialize_text_subtree<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let Some(element) = node.as_element() else {
        return escape_xml(&node.text_content());
    };
    let html_tag = element.local_name().as_ref().to_owned();
    // Accept both lowercase (HTML parser) and SVG-cased names.
    if html_tag != "text" && html_tag != "tspan"
        && html_tag != "textpath" && html_tag != "textPath"
    {
        return String::new();
    }
    let tag = svg_tag_name(&html_tag);
    let mut attrs = String::new();
    for attr in &["x", "y", "dx", "dy", "fill", "stroke", "stroke-width",
                  "font-size", "font-weight", "font-family", "font-style",
                  "text-anchor", "rotate", "writing-mode",
                  "startOffset", "href", "xlink:href"] {
        if let Some(v) = get_attr(&element, attr) {
            attrs.push_str(&format!(" {}=\"{}\"", attr, v));
        }
    }
    let mut children = String::new();
    for child in node.dom_children() {
        children.push_str(&serialize_text_subtree(child));
    }
    format!("<{} {}>{}</{}>", tag, attrs.trim(), children, tag)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Map HTML-lowercased SVG tag names back to proper SVG camelCase.
/// HTML parsing lowercases all element names (e.g. lineargradient instead of linearGradient),
/// but usvg expects correct SVG casing when parsing the serialized XML.
fn svg_tag_name(html_lower: &str) -> &str {
    match html_lower {
        "lineargradient" => "linearGradient",
        "radialgradient" => "radialGradient",
        "clippath" => "clipPath",
        "textpath" => "textPath",
        "fontface" => "fontFace",
        "fontfacesrc" => "fontFaceSrc",
        "fontfaceuri" => "fontFaceUri",
        "fontfaceformat" => "fontFaceFormat",
        "fontfacename" => "fontFaceName",
        "missingglyph" => "missingGlyph",
        "glyphref" => "glyphRef",
        "altglyph" => "altGlyph",
        "altglyphdef" => "altGlyphDef",
        "altglyphitem" => "altGlyphItem",
        "colorkey" => "colorKey",
        "colorprofile" => "colorProfile",
        "componenttransfer" => "componentTransfer",
        "fedistantlight" => "feDistantLight",
        "fepointlight" => "fePointLight",
        "fespotlight" => "feSpotLight",
        "fedropshadow" => "feDropShadow",
        "fecolormatrix" => "feColorMatrix",
        "fecomponenttransfer" => "feComponentTransfer",
        "fecomposite" => "feComposite",
        "feconvolvematrix" => "feConvolveMatrix",
        "fediffuselighting" => "feDiffuseLighting",
        "fedisplacementmap" => "feDisplacementMap",
        "feflood" => "feFlood",
        "fegaussianblur" => "feGaussianBlur",
        "feimage" => "feImage",
        "femerge" => "feMerge",
        "femergenode" => "feMergeNode",
        "femorphology" => "feMorphology",
        "feoffset" => "feOffset",
        "fespecularlighting" => "feSpecularLighting",
        "fetile" => "feTile",
        "feturbulence" => "feTurbulence",
        "foreignobject" => "foreignObject",
        "animate" | "animatetransform" | "animatemotion" | "animatecolor"
            | "set" | "mpath" | "switch" | "view" => html_lower,
        other => other,
    }
}

fn serialize_defs_subtree<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let Some(element) = node.as_element() else {
        return node.text_content().to_string();
    };
    let tag = svg_tag_name(element.local_name().as_ref()).to_owned();
    let mut attrs = String::new();
    // Collect all known SVG attributes — covers gradients, filters, masks,
    // patterns, clip paths, stops, and general shape attributes.
    for attr in &[
        // Core
        "id", "class", "style",
        // Linear gradient
        "x1", "y1", "x2", "y2",
        // Radial gradient
        "cx", "cy", "r", "fx", "fy", "fr",
        // Gradient common
        "gradientUnits", "gradientTransform", "spreadMethod",
        // Stop
        "offset", "stop-color", "stop-opacity",
        // Pattern
        "patternUnits", "patternContentUnits", "patternTransform",
        "x", "y", "width", "height",
        // Clip path & mask
        "clipPathUnits", "maskUnits", "maskContentUnits",
        // Filter
        "filterUnits", "primitiveUnits",
        // Path / shape
        "d", "points",
        // Paint
        "fill", "stroke", "stroke-width", "stroke-linecap",
        "stroke-linejoin", "stroke-dasharray", "stroke-dashoffset",
        // Filter primitives
        "in", "result", "stdDeviation", "dx", "dy", "tableValues",
        "mode", "type", "scale", "bias", "operator",
        // Generic
        "transform", "opacity",
    ] {
        if let Some(v) = get_attr(&element, attr) {
            attrs.push_str(&format!(" {}=\"{}\"", attr, v));
        }
    }
    let mut children = String::new();
    for child in node.dom_children() {
        children.push_str(&serialize_defs_subtree(child));
    }
    format!("<{} {}>{}</{}>", tag, attrs.trim(), children, tag)
}

fn extract_flattened(node: &usvg::Node, group: &mut Group) {
    match node {
        usvg::Node::Group(g) => {
            for child in g.children() {
                extract_flattened(child, group);
            }
        }
        usvg::Node::Path(path) => {
            group.push_child(usvg::Node::Path(path.clone()));
        }
        usvg::Node::Text(text) => {
            for child in text.flattened().children() {
                extract_flattened(child, group);
            }
        }
        usvg::Node::Image(img) => {
            if let usvg::ImageKind::SVG(tree) = img.kind() {
                extract_flattened(&usvg::Node::Group(Box::new(tree.root().clone())), group);
            }
            // Raster images (PNG etc.) — stored as Image node
        }
        _ => {}
    }
}

fn build_image_element<'dom>(node: ServoLayoutNode<'dom>) -> Option<Node> {
    let element = node.as_element()?;
    let w = attr_f32(&element, "width", 100.0).max(1.0);
    let h = attr_f32(&element, "height", 100.0).max(1.0);

    // SVG image via data URI — parse + extract nested shapes
    if let Some(href) = get_attr(&element, "href").or_else(|| get_attr(&element, "xlink:href")) {
        if href.starts_with("data:") {
            if let Some(node) = load_svg_data_uri(&href, w, h) {
                return Some(node);
            }
        } else {
            if let Some(kind) = load_external_image(&href) {
                let size = Size::from_wh(w, h).unwrap_or(Size::from_wh(100.0, 100.0).unwrap());
            if let Some(img) = build_image_success(kind, size) {
                    return Some(img);
                }
            }
        }
    }

    // Fallback placeholder — colored rect
    let fill = Fill::new(Paint::Color(Color::new_rgb(100, 150, 200)), Opacity::ONE, FillRule::NonZero);
    let shape = SimpleShape::new(
        SimpleShapeKind::Rect { x: 0.0, y: 0.0, width: w, height: h, rx: None, ry: None },
        Some(fill), None, Transform::default(),
    );
    Some(Node::SimpleShape(Box::new(shape)))
}

fn load_svg_data_uri(href: &str, w: f32, h: f32) -> Option<Node> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}"><image href="{}" x="0" y="0" width="{}" height="{}"/></svg>"#,
        w, h, escape_xml(href), w, h
    );
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opt).ok()?;
    let mut group = Group::new();
    for child in tree.root().children() {
        extract_flattened(child, &mut group);
    }
    if group.has_children() {
        Some(Node::Group(Box::new(group)))
    } else {
        None
    }
}

fn build_image_success(kind: ImageKind, size: Size) -> Option<Node> {
    Image::new(String::new(), true, size, ImageRendering::default(), kind, Transform::default())
        .map(|img| Node::Image(Box::new(img)))
}

fn load_external_image(href: &str) -> Option<ImageKind> {
    // Try multiple locations for the file
    let candidates: Vec<std::path::PathBuf> = vec![
        std::env::current_dir().ok()?.join(href),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(href),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join(href),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?.join(href),
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return detect_image_kind(data);
        }
    }
    None
}

fn detect_image_kind(data: Vec<u8>) -> Option<ImageKind> {
    if data.len() < 8 { return None; }
    match &data[0..4] {
        b"\x89PNG" => Some(ImageKind::PNG(Arc::new(data))),
        b"\xff\xd8\xff" => Some(ImageKind::JPEG(Arc::new(data))),
        b"GIF8" => Some(ImageKind::GIF(Arc::new(data))),
        b"RIFF" if data.len() > 8 && &data[8..12] == b"WEBP" => Some(ImageKind::WEBP(Arc::new(data))),
        _ => None,
    }
}
