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

/// Gradient store — holds gradient definitions parsed from <defs> for
/// resolution of `url(#id)` references during node construction.
/// A reusable path definition from `<defs>`.
struct DefPath {
    d: String,
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<String>,
}

/// Definitions collected from `<defs>` — gradients and paths used by
/// `url(#id)` references.
struct GradientStore {
    linear: Vec<Arc<LinearGradient>>,
    radial: Vec<Arc<RadialGradient>>,
    /// Path definitions keyed by element `id`.
    paths: std::collections::HashMap<String, DefPath>,
}

impl GradientStore {
    fn lookup_linear(&self, id: &str) -> Option<&Arc<LinearGradient>> {
        self.linear.iter().find(|g| g.id() == id)
    }

    fn lookup_radial(&self, id: &str) -> Option<&Arc<RadialGradient>> {
        self.radial.iter().find(|g| g.id() == id)
    }

    fn lookup_path(&self, id: &str) -> Option<&DefPath> {
        self.paths.get(id)
    }
}

/// Build gradient definitions directly from the DOM — no XML serialization.
/// Iterates `<defs>` children looking for `<linearGradient>` and `<radialGradient>`
/// elements, reads their attributes, and constructs usvg types via public constructors.
fn build_gradients_from_dom<'dom>(root_node: ServoLayoutNode<'dom>) -> GradientStore {
    let mut linear = Vec::new();
    let mut radial = Vec::new();
    let mut paths = std::collections::HashMap::new();

    for child in root_node.dom_children() {
        let Some(elem) = child.as_element() else { continue };
        if elem.local_name().as_ref() != "defs" { continue }

        // Found a <defs> — iterate its children for gradient and path elements.
        for defs_child in child.dom_children() {
            let Some(defs_elem) = defs_child.as_element() else { continue };
            let tag = defs_elem.local_name().as_ref();
            match tag {
                "lineargradient" | "linearGradient" => {
                    if let Some(g) = build_linear_gradient_from_dom(&defs_elem, defs_child) {
                        linear.push(Arc::new(g));
                    }
                }
                "radialgradient" | "radialGradient" => {
                    if let Some(g) = build_radial_gradient_from_dom(&defs_elem, defs_child) {
                        radial.push(Arc::new(g));
                    }
                }
                "path" => {
                    if let Some(id) = get_attr(&defs_elem, "id") {
                        if let Some(d) = get_attr(&defs_elem, "d") {
                            paths.insert(id, DefPath {
                                d,
                                fill: get_attr(&defs_elem, "fill"),
                                stroke: get_attr(&defs_elem, "stroke"),
                                stroke_width: get_attr(&defs_elem, "stroke-width"),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    GradientStore { linear, radial, paths }
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
///
/// Returns an [`svg_engine::SvgRenderData`] bundle: the usvg tree plus the
/// font side-tables (a [`FontKeyRegistry`] mapping each text span's
/// `font_handle` to a WebRender `FontInstanceKey`, and a [`GlyphStore`] of
/// pre-shaped glyphs). usvg itself stays free of any WebRender dependency;
/// the handles on its text spans are only resolved via these tables.
pub(crate) fn build_svg_render_tree<'dom>(
    root_node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<Arc<svg_engine::SvgRenderData>> {
    let size = Size::from_wh(300.0, 150.0)?;
    let mut root = Group::new();

    // Font side-tables — filled by build_text_element as it resolves native
    // fonts via Servo's FontContext. Threads through build_node → text.
    let mut font_keys = svg_engine::FontKeyRegistry::new();
    let mut glyphs = svg_engine::GlyphStore::new();

    // Extract gradient definitions directly from <defs> DOM children.
    // No XML serialization — we read attributes from the DOM and construct
    // usvg types using their public constructors.
    let gradients = build_gradients_from_dom(root_node);

    // Build the render tree (with gradient resolution available).
    for child in root_node.dom_children() {
        if let Some(node) = build_node(child, &gradients, context, &mut font_keys, &mut glyphs) {
            root.push_child(node);
        }
    }

    let mut tree = Tree::new(size, root);

    // Push gradient definitions into the final tree.
    for lg in &gradients.linear { tree.push_linear_gradient(lg.clone()); }
    for rg in &gradients.radial { tree.push_radial_gradient(rg.clone()); }

    // Ensure the font database has system fonts loaded for usvg's own
    // layout/flattening of complex text (textPath, gradient-filled text).
    // Simple text is shaped with Servo's FontContext above, not this db.
    {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        tree.set_fontdb(Arc::new(db));
    }

    Some(Arc::new(svg_engine::SvgRenderData {
        tree: Arc::new(tree),
        font_keys: Arc::new(font_keys),
        glyphs: Arc::new(glyphs),
    }))
}

fn build_node<'dom>(
    node: ServoLayoutNode<'dom>,
    gradients: &GradientStore,
    context: &LayoutContext,
    font_keys: &mut svg_engine::FontKeyRegistry,
    glyphs: &mut svg_engine::GlyphStore,
) -> Option<Node> {
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
                if let Some(n) = build_node(child, gradients, context, font_keys, glyphs) {
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
        "text" => build_text_element(node, gradients, context, font_keys, glyphs),
        "use" => build_use_element(&element, gradients),
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
        "darkred" => Some(Color::new_rgb(139, 0, 0)),
        "crimson" => Some(Color::new_rgb(220, 20, 60)),
        "green" => Some(Color::new_rgb(0, 128, 0)),
        "darkgreen" => Some(Color::new_rgb(0, 100, 0)),
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
        "lightgray" | "lightgrey" => Some(Color::new_rgb(211, 211, 211)),
        _ if val.starts_with('#') => {
            let hex = &val[1..];
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color::new_rgb(r, g, b))
            } else if hex.len() == 3 {
                // #abc → #aabbcc
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(Color::new_rgb(r, g, b))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse an SVG `font-family` attribute value into a Stylo [`FontFamily`].
///
/// SVG font-family is a comma-separated list of family names or generic
/// keywords (serif, sans-serif, monospace, cursive, fantasy). The CSS
/// computed style may not reflect the SVG presentation attribute, so we
/// must construct the family list explicitly for native font resolution.
fn parse_svg_font_family(family_str: &str) -> ::style::values::computed::font::FontFamily {
    use ::style::values::computed::font::{
        FamilyName, FontFamilyList, FontFamilyNameSyntax,
        GenericFontFamily, SingleFontFamily,
    };
    use ::style::ArcSlice;
    use stylo_atoms::Atom;

    let families: Vec<SingleFontFamily> = family_str
        .split(',')
        .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\''))
        .filter(|s| !s.is_empty())
        .map(|name| match name.to_lowercase().as_str() {
            "serif" => SingleFontFamily::Generic(GenericFontFamily::Serif),
            "sans-serif" => SingleFontFamily::Generic(GenericFontFamily::SansSerif),
            "monospace" => SingleFontFamily::Generic(GenericFontFamily::Monospace),
            "cursive" => SingleFontFamily::Generic(GenericFontFamily::Cursive),
            "fantasy" => SingleFontFamily::Generic(GenericFontFamily::Fantasy),
            "system-ui" => SingleFontFamily::Generic(GenericFontFamily::SystemUi),
            _ => SingleFontFamily::FamilyName(FamilyName {
                name: Atom::from(name),
                syntax: FontFamilyNameSyntax::Quoted,
            }),
        })
        .collect();

    ::style::values::computed::font::FontFamily {
        families: FontFamilyList {
            list: ArcSlice::from_iter(families.into_iter()),
        },
        is_system_font: false,
        is_initial: false,
    }
}

/// Look up an SVG attribute on `elem`, falling back to `parent` if not set.
/// Used for SVG inheritance from `<text>` to `<tspan>` elements.
fn attr_or_inherit(
    elem: &ServoLayoutElement,
    parent: &ServoLayoutElement,
    name: &str,
    default: &str,
) -> String {
    get_attr(elem, name)
        .or_else(|| get_attr(parent, name))
        .unwrap_or_else(|| default.to_string())
}

/// Parse a comma/space-separated list of floats (e.g. `dx="0,5,10"`).
fn parse_f32_list(s: &str) -> Vec<f32> {
    s.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter_map(|part| part.trim().parse::<f32>().ok())
        .collect()
}

/// If the `<text>` element has a `<textPath>` child, resolve the referenced
/// path and build a [`TextPath`]. Returns `None` if there's no textPath child
/// or the referenced path can't be found/resolved.
fn build_text_path(
    _text_elem: &ServoLayoutElement,
    text_node: ServoLayoutNode,
    defs: &GradientStore,
) -> Option<usvg::TextPath> {
    for child in text_node.dom_children() {
        let Some(elem) = child.as_element() else { continue };
        if elem.local_name().as_ref() != "textPath" && elem.local_name().as_ref() != "textpath" {
            continue;
        }
        // Read href — either `href` or `xlink:href`.
        let href = get_attr(&elem, "href")
            .or_else(|| get_attr(&elem, "xlink:href"))?;
        let path_id = href.strip_prefix('#')?;

        // Look up the path data in the defs store.
        let def_path = defs.lookup_path(path_id)?;
        let usvg_path = usvg::Path::from_d(
            &def_path.d, None, None, Transform::default(),
        )?;
        let skia_path = usvg_path.data().clone();

        // Parse startOffset (percentage or length).
        // Percentages are resolved to absolute path distance (matching usvg's
        // own parser behavior), so we need the path length.
        let start_offset_str = get_attr(&elem, "startOffset").unwrap_or_else(|| "0%".into());
        let start_offset = if let Some(pct) = start_offset_str.strip_suffix('%') {
            let pct_val: f64 = pct.parse::<f32>().ok()? as f64;
            let path_len = compute_path_length(&skia_path);
            (path_len * pct_val / 100.0) as f32
        } else {
            start_offset_str.parse::<f32>().ok()?
        };

        return usvg::TextPath::new(
            path_id,
            start_offset,
            std::sync::Arc::new(skia_path),
        );
    }
    None
}

/// Compute the total arc length of a `tiny_skia_path::Path` using
/// `kurbo` for accurate curve length (same approach as usvg's internal
/// `path_length` function).
fn compute_path_length(path: &tiny_skia_path::Path) -> f64 {
    use kurbo::{ParamCurve, ParamCurveArclen};
    use tiny_skia_path::PathSegment;

    let mut prev_mx = path.points()[0].x;
    let mut prev_my = path.points()[0].y;
    let mut prev_x = prev_mx;
    let mut prev_y = prev_my;
    let mut length = 0.0f64;

    fn line_to_cubic(px: f32, py: f32, x: f32, y: f32) -> kurbo::CubicBez {
        let line = kurbo::Line::new(
            kurbo::Point::new(px as f64, py as f64),
            kurbo::Point::new(x as f64, y as f64),
        );
        let p1 = line.eval(0.33);
        let p2 = line.eval(0.66);
        kurbo::CubicBez::new(line.p0, p1, p2, line.p1)
    }

    for seg in path.segments() {
        let curve = match seg {
            PathSegment::MoveTo(p) => {
                prev_mx = p.x;
                prev_my = p.y;
                prev_x = p.x;
                prev_y = p.y;
                continue;
            }
            PathSegment::LineTo(p) => line_to_cubic(prev_x, prev_y, p.x, p.y),
            PathSegment::QuadTo(p1, p) => kurbo::QuadBez::new(
                kurbo::Point::new(prev_x as f64, prev_y as f64),
                kurbo::Point::new(p1.x as f64, p1.y as f64),
                kurbo::Point::new(p.x as f64, p.y as f64),
            ).raise(),
            PathSegment::CubicTo(p1, p2, p) => kurbo::CubicBez::new(
                kurbo::Point::new(prev_x as f64, prev_y as f64),
                kurbo::Point::new(p1.x as f64, p1.y as f64),
                kurbo::Point::new(p2.x as f64, p2.y as f64),
                kurbo::Point::new(p.x as f64, p.y as f64),
            ),
            PathSegment::Close => line_to_cubic(prev_x, prev_y, prev_mx, prev_my),
        };
        length += curve.arclen(0.5);
        prev_x = curve.p3.x as f32;
        prev_y = curve.p3.y as f32;
    }
    length
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

/// Build a `<use>` element — looks up the referenced element in `<defs>` and
/// renders a clone at the `use` element's position.
fn build_use_element(element: &ServoLayoutElement, defs: &GradientStore) -> Option<Node> {
    let href = get_attr(element, "href")
        .or_else(|| get_attr(element, "xlink:href"))?;
    let ref_id = href.strip_prefix('#')?;

    // Try to find a path with this id in defs.
    if let Some(def_path) = defs.lookup_path(ref_id) {
        let x = attr_f32(element, "x", 0.0);
        let y = attr_f32(element, "y", 0.0);
        let transform = if x != 0.0 || y != 0.0 {
            Transform::from_translate(x, y)
        } else {
            Transform::default()
        };
        // Inherit fill/stroke from the referenced path when <use> doesn't
        // specify its own.
        let fill = build_fill(element, defs)
            .or_else(|| build_fill_from_defs(def_path));
        let stroke = build_stroke(element, defs)
            .or_else(|| build_stroke_from_defs(def_path));
        return Path::from_d(&def_path.d, fill, stroke, transform)
            .map(|p| Node::Path(Box::new(p)));
    }

    // TODO: support other element types (rect, circle, etc.)

    None
}

/// Build a [`Fill`] from a [`DefPath`]'s stored attributes.
fn build_fill_from_defs(def: &DefPath) -> Option<Fill> {
    let fill_str = def.fill.as_deref()?;
    if fill_str == "none" { return None; }
    let color = parse_color(fill_str)?;
    Some(Fill::new(Paint::Color(color), Opacity::ONE, FillRule::NonZero))
}

/// Build a [`Stroke`] from a [`DefPath`]'s stored attributes.
fn build_stroke_from_defs(def: &DefPath) -> Option<Stroke> {
    let stroke_str = def.stroke.as_deref()?;
    if stroke_str == "none" { return None; }
    let color = parse_color(stroke_str)?;
    let width = def.stroke_width.as_deref()
        .and_then(|w| w.parse::<f32>().ok())
        .unwrap_or(1.0);
    StrokeWidth::new(width.max(0.01))
        .map(|sw| Stroke::new(Paint::Color(color), sw))
}

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

/// Build spans from DOM children. Each <tspan> gets its own TextSpan with
/// that element's fill. Direct text uses the parent element's fill.
/// Returns normalized (collapsed whitespace) text content and spans, with
/// each span's `font_handle` set when a native font could be resolved.
fn build_colored_spans<'dom>(
    node: ServoLayoutNode<'dom>,
    parent_elem: &ServoLayoutElement<'dom>,
    context: &LayoutContext,
    font_keys: &mut svg_engine::FontKeyRegistry,
    glyphs: &mut svg_engine::GlyphStore,
) -> Option<(String, Vec<TextSpan>)> {
    let mut full_text = String::new();
    let mut spans = Vec::new();

    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            let tag = elem.local_name().as_ref();
            if tag == "tspan" || tag == "textPath" || tag == "textpath" {
                // Normalize this child's text and build a span with its style.
                // Handles <tspan> and <textPath> children the same way — both
                // contribute text content to the chunk.
                let child_text: String = extract_text_content(child)
                    .split_whitespace().collect::<Vec<_>>().join(" ");
                if !child_text.is_empty() {
                    // Push an inter-word space before this span (except for the
                    // first). Include the space in the span's text range so it
                    // gets shaped as a glyph and produces visible spacing.
                    let space_len = if full_text.is_empty() { 0 } else {
                        full_text.push(' ');
                        1
                    };
                    let start = full_text.len() - space_len;
                    full_text.push_str(&child_text);
                    let end = full_text.len();
                    if let Some(mut span) = make_span(&elem, parent_elem, start, end) {
                        if let Some(h) = shape_span_with_servo_fonts(
                            &elem, parent_elem, start, end, &full_text,
                            span.font_size().get(), context, font_keys, glyphs,
                        ) {
                            span.set_font_handle(h);
                        }
                        spans.push(span);
                    }
                }
            }
        } else {
            // Direct text content — use parent's style.
            let text: String = child.text_content()
                .split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                let space_len = if full_text.is_empty() { 0 } else {
                    full_text.push(' ');
                    1
                };
                let start = full_text.len() - space_len;
                full_text.push_str(&text);
                let end = full_text.len();
                if let Some(mut span) = make_span(parent_elem, parent_elem, start, end) {
                    if let Some(h) = shape_span_with_servo_fonts(
                        parent_elem, parent_elem, start, end, &full_text,
                        span.font_size().get(), context, font_keys, glyphs,
                    ) {
                        span.set_font_handle(h);
                    }
                    spans.push(span);
                }
            }
        }
    }

    if spans.is_empty() { None } else { Some((full_text, spans)) }
}

/// Build a TextSpan for a DOM element using that element's style attributes.
/// Falls back to `parent_elem` for attributes that the element doesn't set
/// (SVG inheritance from `<text>` to `<tspan>`).
fn make_span(
    elem: &ServoLayoutElement,
    parent_elem: &ServoLayoutElement,
    start: usize,
    end: usize,
) -> Option<TextSpan> {
    let inherited_font_size = attr_f32(parent_elem, "font-size", 16.0);
    let font_size = attr_opt_f32(elem, "font-size")
        .unwrap_or(inherited_font_size)
        .max(1.0);
    let font_family_str = attr_or_inherit(elem, parent_elem, "font-family", "sans-serif");
    let font_weight = attr_or_inherit(elem, parent_elem, "font-weight", "400")
        .parse::<f32>().unwrap_or(400.0) as u16;
    let font_style_str = attr_or_inherit(elem, parent_elem, "font-style", "normal");
    let font_style = match font_style_str.as_str() {
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique,
        _ => FontStyle::Normal,
    };
    let font = Font::from_attrs(&font_family_str, font_weight, font_style);
    // Fill: try element first, then parent, then default to black.
    let empty_defs = GradientStore { linear: vec![], radial: vec![], paths: Default::default() };
    let fill = build_fill(elem, &empty_defs)
        .or_else(|| build_fill(parent_elem, &empty_defs))
        .unwrap_or_else(|| Fill::new(
            Paint::Color(Color::new_rgb(0, 0, 0)),
            Opacity::ONE, FillRule::NonZero,
        ));
    // Stroke: try element first, then parent. Capture stroke so the emitter
    // can route stroked text to the path fallback.
    let stroke = build_stroke(elem, &empty_defs)
        .or_else(|| build_stroke(parent_elem, &empty_defs));
    TextSpan::new(start, end, Some(fill), stroke, font, font_size)
}

fn build_text_element<'dom>(
    node: ServoLayoutNode<'dom>,
    defs: &GradientStore,
    context: &LayoutContext,
    font_keys: &mut svg_engine::FontKeyRegistry,
    glyphs: &mut svg_engine::GlyphStore,
) -> Option<Node> {
    let element = node.as_element()?;
    let x = attr_f32(&element, "x", 0.0);
    let y = attr_f32(&element, "y", 0.0);
    let text_anchor = match get_attr(&element, "text-anchor").as_deref() {
        Some("middle") => TextAnchor::Middle,
        Some("end") => TextAnchor::End,
        _ => TextAnchor::Start,
    };
    let rendering_mode = match get_attr(&element, "text-rendering").as_deref() {
        Some("optimizeSpeed") => TextRendering::OptimizeSpeed,
        Some("geometricPrecision") => TextRendering::GeometricPrecision,
        Some("optimizeLegibility") => TextRendering::OptimizeLegibility,
        _ => TextRendering::default(),
    };

    // Read per-glyph transform attributes (dx, dy, rotate).
    let dx = parse_f32_list(&get_attr(&element, "dx").unwrap_or_default());
    let dy = parse_f32_list(&get_attr(&element, "dy").unwrap_or_default());
    let rotate = parse_f32_list(&get_attr(&element, "rotate").unwrap_or_default());

    // Check for <textPath> child and build the referenced path.
    let text_path = build_text_path(&element, node, defs);

    // Build spans from DOM children — each <tspan> gets its own TextSpan
    // with that tspan's fill color. Text not in a <tspan> uses parent fill.
    // Shaping + font resolution happens inside so each span has access to its
    // DOM element (for computed font style) and its text.
    let (content, spans) = build_colored_spans(node, &element, context, font_keys, glyphs)?;
    if content.is_empty() {
        return None;
    }

    let mut chunk = TextChunk::new(x, y, text_anchor, spans, content);
    if let Some(tp) = text_path {
        chunk.set_text_path(tp);
    }

    let mut text = Text::new(String::new(), rendering_mode, Transform::default());
    if !dx.is_empty() { text.set_dx(dx); }
    if !dy.is_empty() { text.set_dy(dy); }
    if !rotate.is_empty() { text.set_rotate(rotate); }
    text.push_chunk(chunk);

    // Run usvg's layout + flattening so that complex text (textPath, gradient
    // fills, per-glyph transforms) has a flattened-path fallback that the
    // emitter can use. Simple text with a resolved font_handle bypasses this
    // and is rendered via native glyphs in the emitter.
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let fontdb = Arc::new(db);
    let resolver = FontResolver::default();
    let mut cache = usvg::Cache::new(fontdb.clone());
    // convert() is optional for simple text but required to populate
    // `flattened` for the complex fallback. Failure is non-fatal — simple
    // spans with a font_handle still render via native glyphs.
    let _ = usvg::convert(&mut text, &resolver, &mut cache);

    Some(Node::Text(Box::new(text)))
}

/// Resolve a native font for a text span via Servo's `FontContext` and shape
/// its glyphs using `font.glyph_index` / `font.glyph_h_advance`.
///
/// Returns the opaque handle (stored on the usvg span) if a font was resolved
/// and at least one glyph shaped. The `FontInstanceKey` and shaped glyphs are
/// recorded in `font_keys` / `glyphs` respectively, keyed by that handle.
///
/// This mirrors the old `svg-text` branch's per-codepoint font fallback
/// (`font_group.find_by_codepoint`), so mixed-script spans pick the correct
/// font per character — something the previous fontdb/rustybuzz shaper could
/// not do and which is required for correct multilingual rendering.
fn shape_span_with_servo_fonts(
    elem: &ServoLayoutElement,
    parent_elem: &ServoLayoutElement,
    span_start: usize,
    span_end: usize,
    full_text: &str,
    font_size: f32,
    context: &LayoutContext,
    font_keys: &mut svg_engine::FontKeyRegistry,
    glyphs: &mut svg_engine::GlyphStore,
) -> Option<usize> {
    use layout_api::LayoutElement;

    let span_text = &full_text[span_start..span_end];
    if span_text.is_empty() {
        return None;
    }

    // Build a font group from the element's computed font style, but override
    // font properties with SVG presentation attributes (with inheritance from
    // the parent element, e.g. `<text>` → `<tspan>`). SVG presentation
    // attributes may not be mapped to CSS computed style for SVG text
    // elements, so we must apply them explicitly.
    let element_style = elem.style(&context.style_context);
    let mut font_style = (*element_style.clone_font()).clone();

    // Override font-family from the SVG attribute (with parent fallback).
    let svg_font_family = attr_or_inherit(elem, parent_elem, "font-family", "sans-serif");
    font_style.set_font_family(parse_svg_font_family(&svg_font_family));

    // Override font-style from the SVG attribute (with parent fallback).
    let svg_font_style = match attr_or_inherit(elem, parent_elem, "font-style", "normal").as_str() {
        "italic" => ::style::values::computed::font::FontStyle::ITALIC,
        "oblique" => ::style::values::computed::font::FontStyle::OBLIQUE,
        _ => ::style::values::computed::font::FontStyle::NORMAL,
    };
    font_style.set_font_style(svg_font_style);

    // Override font-weight from the SVG attribute (with parent fallback).
    let svg_font_weight = match attr_or_inherit(elem, parent_elem, "font-weight", "normal").as_str() {
        "bold" => ::style::values::computed::font::FontWeight::BOLD,
        "normal" => ::style::values::computed::font::FontWeight::NORMAL,
        val => {
            val.parse::<f32>().ok()
                .map(|w| ::style::values::computed::font::FontWeight::from_float(w))
                .unwrap_or(::style::values::computed::font::FontWeight::NORMAL)
        }
    };
    font_style.set_font_weight(svg_font_weight);

    font_style.compute_font_hash();

    // Override font-size from the SVG attribute (in app units).
    let au_size = app_units::Au::from_f32_px(font_size);
    let font_group = context.font_context.font_group_with_size(
        servo_arc::Arc::new(font_style),
        au_size,
    );
    let language: icu_locid::subtags::Language = "und".parse().ok()?;

    // Shape per codepoint with font fallback. Track the first resolved font
    // so the whole span shares one FontInstanceKey (WebRender push_text takes
    // a single key per call); characters missing from that font fall back to
    // the notdef glyph, which is the same behaviour as the old branch.
    let mut first_key: Option<webrender_api::FontInstanceKey> = None;
    let mut shaped: Vec<svg_engine::ShapedGlyph> = Vec::with_capacity(span_text.len());
    let chars: Vec<char> = span_text.chars().collect();
    let mut x_cursor = 0.0f32;
    let mut total_advance = 0.0f32;

    for (i, ch) in chars.iter().enumerate() {
        let next_ch = chars.get(i + 1).copied();
        let font_ref = font_group.find_by_codepoint(
            &*context.font_context, *ch, next_ch, language,
        );

        let (glyph_id, advance) = match font_ref.as_ref().and_then(|fr| {
            let gid = fr.glyph_index(*ch)?;
            let adv = fr.glyph_h_advance(gid);
            Some((gid, adv))
        }) {
            Some(v) => v,
            // No glyph available — skip the character (matches old fallback).
            None => continue,
        };

        // Resolve the FontInstanceKey lazily from the first font we shape.
        if first_key.is_none() {
            if let Some(fr) = font_ref.as_ref() {
                first_key = Some(fr.key(context.painter_id, &*context.font_context));
            }
        }

        let advance_f32 = advance as f32;
        shaped.push(svg_engine::ShapedGlyph {
            glyph_id,
            x: x_cursor,
            y: 0.0,
            advance: advance_f32,
        });
        x_cursor += advance_f32;
        total_advance += advance_f32;
    }

    let key = first_key?;
    if shaped.is_empty() {
        return None;
    }

    let handle = font_keys.register(key);
    glyphs.insert(handle, svg_engine::ShapedSpan {
        glyphs: shaped,
        total_advance,
        font_size,
    });
    Some(handle)
}

/// Extract all text content from a DOM node and its descendants.
fn extract_text_content<'dom>(node: ServoLayoutNode<'dom>) -> String {
    let mut text = String::new();
    for child in node.dom_children() {
        if let Some(elem) = child.as_element() {
            let tag = elem.local_name().as_ref();
            if tag == "tspan" || tag == "textPath" || tag == "textpath" {
                text.push_str(&extract_text_content(child));
            }
        } else {
            text.push_str(&child.text_content());
        }
    }
    text
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
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
