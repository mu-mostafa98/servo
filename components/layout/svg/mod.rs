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
use usvg::*;
use web_atoms::ns;

use crate::context::LayoutContext;

/// Main entry point — builds a complete usvg::Tree from an SVG DOM element.
pub(crate) fn build_svg_render_tree<'dom>(
    root_node: ServoLayoutNode<'dom>,
    _context: &LayoutContext,
) -> Option<Arc<usvg::Tree>> {
    let size = Size::from_wh(300.0, 150.0)?;
    let mut root = Group::new();

    // Collect <defs> markup once for href="#id" references in textPath
    let mut defs_xml = String::new();
    for child in root_node.dom_children() {
        if let Some(elem) = child.as_element() {
            if elem.local_name().as_ref() == "defs" {
                defs_xml.push_str(&serialize_defs_subtree(child));
            }
        }
    }

    for child in root_node.dom_children() {
        if let Some(node) = build_node(child, &defs_xml) {
            root.push_child(node);
        }
    }

    Some(Arc::new(Tree::new(size, root)))
}

fn build_node<'dom>(node: ServoLayoutNode<'dom>, defs_xml: &str) -> Option<Node> {
    let element = node.as_element()?;
    let tag = element.local_name().as_ref().to_owned();

    let mut group = Group::new();

    match tag.as_str() {
        "svg" | "g" => {
            for child in node.dom_children() {
                if let Some(n) = build_node(child, defs_xml) {
                    group.push_child(n);
                }
            }
            Some(Node::Group(Box::new(group)))
        }
        "rect" => build_rect(&element).map(|s| Node::SimpleShape(Box::new(s))),
        "circle" => build_circle(&element).map(|s| Node::SimpleShape(Box::new(s))),
        "ellipse" => build_ellipse(&element).map(|s| Node::SimpleShape(Box::new(s))),
        "line" => build_line(&element).map(|s| Node::SimpleShape(Box::new(s))),
        "path" => build_path_element(&element).map(|p| Node::Path(Box::new(p))),
        "polygon" => build_polygon_element(&element).map(|p| Node::Path(Box::new(p))),
        "polyline" => build_polyline_element(&element).map(|p| Node::Path(Box::new(p))),
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

fn build_fill(element: &ServoLayoutElement) -> Option<Fill> {
    let fill_str = get_attr(element, "fill")?;
    if fill_str == "none" {
        return None;
    }
    let color = parse_color(&fill_str)?;
    let opacity = get_attr(element, "fill-opacity")
        .as_deref()
        .and_then(parse_f32)
        .and_then(Opacity::new)
        .unwrap_or(Opacity::ONE);
    Some(Fill::new(Paint::Color(color), opacity, FillRule::NonZero))
}

fn build_stroke(element: &ServoLayoutElement) -> Option<Stroke> {
    let stroke_str = get_attr(element, "stroke")?;
    if stroke_str == "none" {
        return None;
    }
    let color = parse_color(&stroke_str)?;
    let width = attr_f32(element, "stroke-width", 1.0);
    let sw = StrokeWidth::new(width.max(0.01))?;
    let opacity = get_attr(element, "stroke-opacity")
        .as_deref()
        .and_then(parse_f32)
        .and_then(Opacity::new)
        .unwrap_or(Opacity::ONE);
    Some(Stroke::new(Paint::Color(color), sw))
}

// ======================= Shape Builders =======================

fn build_rect(element: &ServoLayoutElement) -> Option<SimpleShape> {
    use SimpleShapeKind::Rect;
    let x = attr_f32(element, "x", 0.0);
    let y = attr_f32(element, "y", 0.0);
    let w = attr_f32(element, "width", 0.0);
    let h = attr_f32(element, "height", 0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let rx = attr_opt_f32(element, "rx");
    let ry = attr_opt_f32(element, "ry");
    let fill = build_fill(element);
    let stroke = build_stroke(element);
    if fill.is_none() && stroke.is_none() {
        return None;
    }
    Some(SimpleShape::new(
        Rect { x, y, width: w, height: h, rx, ry },
        fill, stroke, Transform::default(),
    ))
}

fn build_circle(element: &ServoLayoutElement) -> Option<SimpleShape> {
    use SimpleShapeKind::Circle;
    let cx = attr_f32(element, "cx", 0.0);
    let cy = attr_f32(element, "cy", 0.0);
    let r = attr_f32(element, "r", 0.0);
    if r <= 0.0 {
        return None;
    }
    let fill = build_fill(element);
    let stroke = build_stroke(element);
    Some(SimpleShape::new(
        Circle { cx, cy, r },
        fill, stroke, Transform::default(),
    ))
}

fn build_ellipse(element: &ServoLayoutElement) -> Option<SimpleShape> {
    use SimpleShapeKind::Ellipse;
    let cx = attr_f32(element, "cx", 0.0);
    let cy = attr_f32(element, "cy", 0.0);
    let rx = attr_f32(element, "rx", 0.0);
    let ry = attr_f32(element, "ry", 0.0);
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let fill = build_fill(element);
    let stroke = build_stroke(element);
    Some(SimpleShape::new(
        Ellipse { cx, cy, rx, ry },
        fill, stroke, Transform::default(),
    ))
}

fn build_line(element: &ServoLayoutElement) -> Option<SimpleShape> {
    use SimpleShapeKind::Line;
    let x1 = attr_f32(element, "x1", 0.0);
    let y1 = attr_f32(element, "y1", 0.0);
    let x2 = attr_f32(element, "x2", 0.0);
    let y2 = attr_f32(element, "y2", 0.0);
    let stroke = build_stroke(element)?;
    Some(SimpleShape::new(
        Line { x1, y1, x2, y2 },
        None, Some(stroke), Transform::default(),
    ))
}

// ======================= Complex Shape Builders =======================

fn build_path_element(element: &ServoLayoutElement) -> Option<Path> {
    let d = get_attr(element, "d")?;
    let fill = build_fill(element);
    let stroke = build_stroke(element);
    Path::from_d(&d, fill, stroke, Transform::default())
}

fn build_polygon_element(element: &ServoLayoutElement) -> Option<Path> {
    let points = get_attr(element, "points")?;
    let fill = build_fill(element);
    let stroke = build_stroke(element);
    Path::from_points(&points, true, fill, stroke, Transform::default())
}

fn build_polyline_element(element: &ServoLayoutElement) -> Option<Path> {
    let points = get_attr(element, "points")?;
    let fill = build_fill(element);
    let stroke = build_stroke(element);
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
    let tag = element.local_name().as_ref().to_owned();
    if tag != "text" && tag != "tspan" && tag != "textPath" {
        return String::new();
    }
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

fn serialize_defs_subtree<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let Some(element) = node.as_element() else {
        return node.text_content().to_string();
    };
    let tag = element.local_name().as_ref().to_owned();
    let mut attrs = String::new();
    for attr in &["id", "d", "fill", "stroke", "stroke-width"] {
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
