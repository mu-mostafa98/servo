/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Programmatic construction of a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! This is the Phase 2 bridge: instead of serializing the SVG subtree back to
//! XML (which discards the CSS cascade) and letting usvg re-parse it, we walk
//! the DOM on the layout thread — where computed styles are available — and
//! build the usvg tree directly via usvg's public constructors.
//!
//! Reading [`ComputedValues`] means `fill`, `stroke`, `opacity`, and the shape
//! geometry properties (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are taken from the
//! post-cascade result, so stylesheets and presentation attributes both apply.

use std::collections::HashMap;
use std::sync::Arc;

use std::hash::{Hash, Hasher};

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutNode};
use net_traits::image_cache::ImageCache;
use resvg::usvg::{self, tiny_skia_path};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::color::ColorSpace;
use style::dom::OpaqueNode;
use style::properties::ComputedValues;
use style::values::computed::{LengthPercentage, NonNegativeLengthPercentageOrAuto};
use style::values::computed::svg::{SVGOpacity, SVGPaint, SVGPaintKind, SVGStrokeDashArray};
use style::values::generics::length::GenericLengthPercentageOrAuto;
use style::values::generics::svg::SVGLength;
use svgtypes::{PathParser, PathSegment, TransformListParser, TransformListToken};
use webrender_api::units::DeviceIntSize;
use webrender_api::ImageKey;

use crate::context::LayoutContext;

/// Paint servers referenced by `url(#id)` and collected from `<linearGradient>`
/// and `<radialGradient>` elements before the main tree walk.
#[derive(Default)]
struct Gradients {
    linear: HashMap<String, Arc<usvg::LinearGradient>>,
    radial: HashMap<String, Arc<usvg::RadialGradient>>,
}

/// Builds a [`usvg::Tree`] from the `<svg>` element at `node`.
///
/// Returns `None` when `node` is not an `<svg>` element or the tree would be
/// empty/invalid.
pub(crate) fn build_usvg_tree(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
) -> Option<usvg::Tree> {
    let element = node.as_element()?;
    if element.local_name() != &LocalName::from("svg") {
        return None;
    }

    let (size, view_box) = resolve_size_and_view_box(&element)?;

    // Collect gradient definitions up front so that `fill`/`stroke` referencing
    // them can be resolved during the main walk.
    let mut gradients = Gradients::default();
    for child in node.dom_children() {
        collect_gradients(child, context, &mut gradients);
    }

    let root_ts = view_box
        .map(|vb| vb.to_transform(size))
        .unwrap_or_else(usvg::Transform::identity);

    // Mirror usvg's XML parser: when a viewBox transform is present, wrap the
    // children in a group carrying that transform; otherwise build straight into
    // the root.
    let mut root = usvg::Group::empty();
    let mut content = usvg::Group::empty();
    content.transform = root_ts;
    content.abs_transform = root_ts;

    for child in node.dom_children() {
        if let Some(child_node) = convert_node(child, context, &gradients, root_ts) {
            content.push_child(child_node);
        }
    }

    if root_ts.is_identity() {
        root = content;
    } else {
        root.push_child(usvg::Node::Group(Box::new(content)));
    }

    let mut tree = usvg::Tree::new(size, root);
    tree.finalize();
    Some(tree)
}

/// Determines the image size and view box from the root `<svg>` element.
fn resolve_size_and_view_box(
    element: &ServoLayoutElement<'_>,
) -> Option<(usvg::Size, Option<usvg::ViewBox>)> {
    let view_box = parse_view_box(element);

    let width = element
        .attribute_as_str(&ns!(), &LocalName::from("width"))
        .and_then(parse_length_attr);
    let height = element
        .attribute_as_str(&ns!(), &LocalName::from("height"))
        .and_then(parse_length_attr);

    let size = match (width, height, view_box.map(|vb| vb.rect)) {
        (Some(w), Some(h), _) => usvg::Size::from_wh(w, h),
        (Some(w), None, Some(vb)) => usvg::Size::from_wh(w, vb.height() * w / vb.width()),
        (None, Some(h), Some(vb)) => usvg::Size::from_wh(vb.width() * h / vb.height(), h),
        (None, None, Some(vb)) => usvg::Size::from_wh(vb.width(), vb.height()),
        _ => usvg::Size::from_wh(100.0, 100.0),
    }?;

    Some((size, view_box))
}

fn parse_view_box(element: &ServoLayoutElement<'_>) -> Option<usvg::ViewBox> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from("viewBox"))?;
    let mut nums = value.split(|c: char| c.is_ascii_whitespace() || c == ',');
    let x = nums.next()?.trim().parse::<f32>().ok()?;
    let y = nums.next()?.trim().parse::<f32>().ok()?;
    let w = nums.next()?.trim().parse::<f32>().ok()?;
    let h = nums.next()?.trim().parse::<f32>().ok()?;
    let rect = usvg::NonZeroRect::from_xywh(x, y, w, h)?;

    let aspect = element
        .attribute_as_str(&ns!(), &LocalName::from("preserveAspectRatio"))
        .and_then(|s| s.parse::<svgtypes::AspectRatio>().ok())
        .unwrap_or_default();

    Some(usvg::ViewBox { rect, aspect })
}

/// Collects `<linearGradient>`/`<radialGradient>` definitions (and their
/// `<stop>` children) into `gradients`.
fn collect_gradients(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &mut Gradients,
) {
    let Some(element) = node.as_element() else {
        return;
    };
    let name = element.local_name().clone();
    if name == LocalName::from("linearGradient") {
        if let (Some(id), Some(grad)) = (element_id(&element), build_linear_gradient(&element)) {
            gradients.linear.insert(id, Arc::new(grad));
        }
        return;
    }
    if name == LocalName::from("radialGradient") {
        if let (Some(id), Some(grad)) = (element_id(&element), build_radial_gradient(&element)) {
            gradients.radial.insert(id, Arc::new(grad));
        }
        return;
    }

    for child in node.dom_children() {
        collect_gradients(child, context, gradients);
    }
}

fn element_id(element: &ServoLayoutElement<'_>) -> Option<String> {
    let id = element.attribute_as_str(&ns!(), &LocalName::from("id"))?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn build_linear_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::LinearGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let x1 = length_attr(element, "x1", 0.0);
    let y1 = length_attr(element, "y1", 0.0);
    let x2 = length_attr(element, "x2", 1.0);
    let y2 = length_attr(element, "y2", 0.0);
    let base = build_base_gradient(element, id)?;
    Some(usvg::LinearGradient::new(base, x1, y1, x2, y2))
}

fn build_radial_gradient(element: &ServoLayoutElement<'_>) -> Option<usvg::RadialGradient> {
    let id = usvg::NonEmptyString::new(element_id(element)?)?;
    let cx = length_attr(element, "cx", 0.5);
    let cy = length_attr(element, "cy", 0.5);
    let r = usvg::PositiveF32::new(length_attr(element, "r", 0.5))?;
    let fx = length_attr(element, "fx", cx);
    let fy = length_attr(element, "fy", cy);
    let fr = usvg::PositiveF32::new(length_attr(element, "fr", 0.0))?;
    let base = build_base_gradient(element, id)?;
    Some(usvg::RadialGradient::new(base, cx, cy, r, fx, fy, fr))
}

fn build_base_gradient(
    element: &ServoLayoutElement<'_>,
    id: usvg::NonEmptyString,
) -> Option<usvg::BaseGradient> {
    let units = match element
        .attribute_as_str(&ns!(), &LocalName::from("gradientUnits"))
        .unwrap_or("objectBoundingBox")
    {
        "userSpaceOnUse" => usvg::Units::UserSpaceOnUse,
        _ => usvg::Units::ObjectBoundingBox,
    };
    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("gradientTransform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let spread_method = match element
        .attribute_as_str(&ns!(), &LocalName::from("spreadMethod"))
        .unwrap_or("pad")
    {
        "reflect" => usvg::SpreadMethod::Reflect,
        "repeat" => usvg::SpreadMethod::Repeat,
        _ => usvg::SpreadMethod::Pad,
    };

    let mut stops = Vec::new();
    for child in element.as_node().dom_children() {
        if let Some(stop) = child.as_element().and_then(|e| build_stop(&e)) {
            stops.push(stop);
        }
    }

    Some(usvg::BaseGradient::new(
        id,
        units,
        transform,
        spread_method,
        stops,
    ))
}

fn build_stop(element: &ServoLayoutElement<'_>) -> Option<usvg::Stop> {
    if element.local_name() != &LocalName::from("stop") {
        return None;
    }
    let offset_raw = element.attribute_as_str(&ns!(), &LocalName::from("offset"))?;
    let offset = if let Some(pct) = offset_raw.strip_suffix('%') {
        pct.parse::<f32>().ok()? / 100.0
    } else {
        offset_raw.parse::<f32>().ok()?
    };
    let offset = usvg::StopOffset::new(offset.clamp(0.0, 1.0))?;

    let color = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-color"))
        .map(parse_color)
        .unwrap_or_else(usvg::Color::black);
    let opacity = element
        .attribute_as_str(&ns!(), &LocalName::from("stop-opacity"))
        .and_then(|s| s.parse::<f32>().ok())
        .and_then(usvg::Opacity::new)
        .unwrap_or(usvg::Opacity::ONE);

    Some(usvg::Stop::new(offset, color, opacity))
}

/// Converts a DOM node (and its subtree) into a [`usvg::Node`].
fn convert_node(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let name = element.local_name().clone();

    if is_group_element(&name) {
        return convert_group(node, context, gradients, parent_abs_transform);
    }

    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    if let Some(shape) = build_shape_node(
        &element,
        &name,
        computed.as_deref(),
        gradients,
        parent_abs_transform,
    ) {
        return Some(shape);
    }

    // Elements we don't handle yet (text, image, use, …) are silently skipped.
    None
}

fn is_group_element(name: &LocalName) -> bool {
    matches!(
        name.as_ref(),
        "svg" | "g" | "defs" | "symbol" | "a" | "clipPath" | "mask" | "pattern"
    )
}

fn convert_group(
    node: ServoLayoutNode<'_>,
    context: &LayoutContext,
    gradients: &Gradients,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let element = node.as_element()?;
    let computed = element
        .style_data()
        .is_some()
        .then(|| node.style(&context.style_context));

    let mut group = usvg::Group::empty();
    group.id = element_id(&element).unwrap_or_default();

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    group.transform = transform;
    let abs_transform = parent_abs_transform.pre_concat(transform);
    group.abs_transform = abs_transform;

    if let Some(computed) = computed.as_deref() {
        group.opacity = usvg::Opacity::new(computed.get_effects().opacity)
            .unwrap_or(usvg::Opacity::ONE);
    }

    for child in node.dom_children() {
        if let Some(child_node) = convert_node(child, context, gradients, abs_transform) {
            group.push_child(child_node);
        }
    }

    Some(usvg::Node::Group(Box::new(group)))
}

fn build_shape_node(
    element: &ServoLayoutElement<'_>,
    name: &LocalName,
    computed: Option<&ComputedValues>,
    gradients: &Gradients,
    parent_abs_transform: usvg::Transform,
) -> Option<usvg::Node> {
    let data = build_shape_path(element, name, computed)?;

    let transform = element
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    let abs_transform = parent_abs_transform.pre_concat(transform);

    let id = element_id(element).unwrap_or_default();
    let visible = computed
        .map(|c| {
            !matches!(
                c.get_inherited_box().visibility,
                style::computed_values::visibility::T::Hidden |
                    style::computed_values::visibility::T::Collapse
            )
        })
        .unwrap_or(true);

    let fill = computed.and_then(|c| build_fill(c, gradients));
    let stroke = computed.and_then(|c| build_stroke(c, gradients));

    usvg::Path::new(
        id,
        visible,
        fill,
        stroke,
        usvg::PaintOrder::default(),
        usvg::ShapeRendering::default(),
        Arc::new(data),
        abs_transform,
    )
    .map(|p| usvg::Node::Path(Box::new(p)))
}

/// Builds the path geometry for a shape element. Geometry properties that are
/// CSS longhands (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are read from `computed`;
/// the rest (`width`/`height`/`x1`/`y1`/`x2`/`y2`/`points`/`d`) fall back to
/// attributes.
fn build_shape_path(
    element: &ServoLayoutElement<'_>,
    name: &LocalName,
    computed: Option<&ComputedValues>,
) -> Option<tiny_skia_path::Path> {
    match name.as_ref() {
        "rect" => {
            let (x, y, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_x()),
                        lp_to_f32(&svg.clone_y()),
                        lp_or_auto_to_f32(&svg.clone_rx()),
                        lp_or_auto_to_f32(&svg.clone_ry()),
                    )
                },
                None => (
                    length_attr(element, "x", 0.0),
                    length_attr(element, "y", 0.0),
                    length_attr_opt(element, "rx"),
                    length_attr_opt(element, "ry"),
                ),
            };
            let w = length_attr(element, "width", 0.0);
            let h = length_attr(element, "height", 0.0);
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            let (rx, ry) = match (rx, ry) {
                (Some(rx), Some(ry)) => (rx.min(w / 2.0), ry.min(h / 2.0)),
                (Some(rx), None) => {
                    let r = rx.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, Some(ry)) => {
                    let r = ry.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, None) => (0.0, 0.0),
            };
            rounded_rect(&mut pb, x, y, w, h, rx, ry);
            pb.finish()
        },
        "circle" => {
            let (cx, cy, r) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_to_f32(&svg.clone_r().0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "r", 0.0),
                ),
            };
            if r <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            ellipse(&mut pb, cx, cy, r, r);
            pb.finish()
        },
        "ellipse" => {
            let (cx, cy, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_or_auto_to_f32(&svg.clone_rx()).unwrap_or(0.0),
                        lp_or_auto_to_f32(&svg.clone_ry()).unwrap_or(0.0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "rx", 0.0),
                    length_attr(element, "ry", 0.0),
                ),
            };
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            ellipse(&mut pb, cx, cy, rx, ry);
            pb.finish()
        },
        "line" => {
            let x1 = length_attr(element, "x1", 0.0);
            let y1 = length_attr(element, "y1", 0.0);
            let x2 = length_attr(element, "x2", 0.0);
            let y2 = length_attr(element, "y2", 0.0);
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            pb.finish()
        },
        "polyline" => polygon_points(element, "points", false),
        "polygon" => polygon_points(element, "points", true),
        "path" => element
            .attribute_as_str(&ns!(), &LocalName::from("d"))
            .and_then(parse_path_d),
        _ => None,
    }
}

fn polygon_points(
    element: &ServoLayoutElement<'_>,
    attr: &str,
    close: bool,
) -> Option<tiny_skia_path::Path> {
    let value = element.attribute_as_str(&ns!(), &LocalName::from(attr))?;
    let mut nums = value.split(|c: char| c.is_ascii_whitespace() || c == ',');
    let mut pb = tiny_skia_path::PathBuilder::new();
    let mut first = true;
    let mut has_point = false;
    loop {
        let x = nums.next().and_then(|s| s.trim().parse::<f32>().ok());
        let y = nums.next().and_then(|s| s.trim().parse::<f32>().ok());
        match (x, y) {
            (Some(x), Some(y)) => {
                if first {
                    pb.move_to(x, y);
                    first = false;
                } else {
                    pb.line_to(x, y);
                }
                has_point = true;
            },
            _ => break,
        }
    }
    if !has_point {
        return None;
    }
    if close {
        pb.close();
    }
    pb.finish()
}

fn ellipse(pb: &mut tiny_skia_path::PathBuilder, cx: f32, cy: f32, rx: f32, ry: f32) {
    const K: f32 = 0.5522847498;
    let ox = rx * K;
    let oy = ry * K;
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.close();
}

fn rounded_rect(
    pb: &mut tiny_skia_path::PathBuilder,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rx: f32,
    ry: f32,
) {
    if rx <= 0.0 || ry <= 0.0 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return;
    }
    const K: f32 = 0.5522847498;
    pb.move_to(x + rx, y);
    pb.line_to(x + w - rx, y);
    pb.cubic_to(x + w - rx + rx * K, y, x + w, y + ry - ry * K, x + w, y + ry);
    pb.line_to(x + w, y + h - ry);
    pb.cubic_to(x + w, y + h - ry + ry * K, x + w - rx + rx * K, y + h, x + w - rx, y + h);
    pb.line_to(x + rx, y + h);
    pb.cubic_to(x + rx - rx * K, y + h, x, y + h - ry + ry * K, x, y + h - ry);
    pb.line_to(x, y + ry);
    pb.cubic_to(x, y + ry - ry * K, x + rx - rx * K, y, x + rx, y);
    pb.close();
}

fn parse_path_d(d: &str) -> Option<tiny_skia_path::Path> {
    let mut pb = tiny_skia_path::PathBuilder::new();
    let mut cur_x = 0.0f64;
    let mut cur_y = 0.0f64;
    let mut start_x = 0.0f64;
    let mut start_y = 0.0f64;
    let mut last_cx = 0.0f64;
    let mut last_cy = 0.0f64;
    let mut last_qx = 0.0f64;
    let mut last_qy = 0.0f64;

    for seg in PathParser::from(d) {
        let seg = seg.ok()?;
        match seg {
            PathSegment::MoveTo { abs, x, y } => {
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                pb.move_to(x as f32, y as f32);
                cur_x = x;
                cur_y = y;
                start_x = x;
                start_y = y;
            },
            PathSegment::LineTo { abs, x, y } => {
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                pb.line_to(x as f32, y as f32);
                cur_x = x;
                cur_y = y;
            },
            PathSegment::HorizontalLineTo { abs, x } => {
                let x = if abs { x } else { cur_x + x };
                pb.line_to(x as f32, cur_y as f32);
                cur_x = x;
            },
            PathSegment::VerticalLineTo { abs, y } => {
                let y = if abs { y } else { cur_y + y };
                pb.line_to(cur_x as f32, y as f32);
                cur_y = y;
            },
            PathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (x1, y1) = abs_xy(abs, x1, y1, cur_x, cur_y);
                let (x2, y2) = abs_xy(abs, x2, y2, cur_x, cur_y);
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                pb.cubic_to(
                    x1 as f32,
                    y1 as f32,
                    x2 as f32,
                    y2 as f32,
                    x as f32,
                    y as f32,
                );
                last_cx = x2;
                last_cy = y2;
                cur_x = x;
                cur_y = y;
            },
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                let (x2, y2) = abs_xy(abs, x2, y2, cur_x, cur_y);
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                let x1 = cur_x + (cur_x - last_cx);
                let y1 = cur_y + (cur_y - last_cy);
                pb.cubic_to(
                    x1 as f32,
                    y1 as f32,
                    x2 as f32,
                    y2 as f32,
                    x as f32,
                    y as f32,
                );
                last_cx = x2;
                last_cy = y2;
                cur_x = x;
                cur_y = y;
            },
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                let (x1, y1) = abs_xy(abs, x1, y1, cur_x, cur_y);
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
                last_qx = x1;
                last_qy = y1;
                cur_x = x;
                cur_y = y;
            },
            PathSegment::SmoothQuadratic { abs, x, y } => {
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                let x1 = cur_x + (cur_x - last_qx);
                let y1 = cur_y + (cur_y - last_qy);
                pb.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
                last_qx = x1;
                last_qy = y1;
                cur_x = x;
                cur_y = y;
            },
            PathSegment::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                let (x, y) = abs_xy(abs, x, y, cur_x, cur_y);
                arc_to(
                    &mut pb,
                    cur_x,
                    cur_y,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                );
                cur_x = x;
                cur_y = y;
            },
            PathSegment::ClosePath { .. } => {
                pb.close();
                cur_x = start_x;
                cur_y = start_y;
            },
        }
    }
    pb.finish()
}

#[inline]
fn abs_xy(abs: bool, x: f64, y: f64, cur_x: f64, cur_y: f64) -> (f64, f64) {
    if abs {
        (x, y)
    } else {
        (cur_x + x, cur_y + y)
    }
}

/// Converts an SVG elliptical arc segment to cubic Bézier curves (per
/// <https://www.w3.org/TR/SVG11/implnote.html#ArcImplementationNotes>).
#[allow(clippy::too_many_arguments)]
fn arc_to(
    pb: &mut tiny_skia_path::PathBuilder,
    x0: f64,
    y0: f64,
    mut rx: f64,
    mut ry: f64,
    phi: f64,
    large_arc: bool,
    sweep: bool,
    x: f64,
    y: f64,
) {
    if rx == 0.0 || ry == 0.0 {
        pb.line_to(x as f32, y as f32);
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    let phi = phi.to_radians();

    let dx2 = (x0 - x) / 2.0;
    let dy2 = (y0 - y) / 2.0;
    let x1p = phi.cos() * dx2 + phi.sin() * dy2;
    let y1p = -phi.sin() * dx2 + phi.cos() * dy2;

    let mut rx2 = rx * rx;
    let mut ry2 = ry * ry;
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let lambda = x1p2 / rx2 + y1p2 / ry2;
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
        rx2 = rx * rx;
        ry2 = ry * ry;
    }

    let numerator = rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2;
    let denominator = rx2 * y1p2 + ry2 * x1p2;
    let mut radicand = if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    };
    if radicand < 0.0 {
        radicand = 0.0;
    }
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let coef = sign * radicand.sqrt();
    let cxp = coef * (rx * y1p / ry);
    let cyp = coef * (-(ry * x1p / rx));

    let cx = phi.cos() * cxp - phi.sin() * cyp + (x0 + x) / 2.0;
    let cy = phi.sin() * cxp + phi.cos() * cyp + (y0 + y) / 2.0;

    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };

    let theta1 = angle(
        1.0,
        0.0,
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
    );
    let mut dtheta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }

    let segments = (dtheta.abs() / (std::f64::consts::FRAC_PI_2)).ceil().max(1.0) as usize;
    let delta = dtheta / segments as f64;
    let t = (8.0 / 3.0) * (delta / 4.0).sin() * (delta / 4.0).sin() / delta.sin();

    let mut theta = theta1;
    let mut prev_x = x0;
    let mut prev_y = y0;
    for _ in 0..segments {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let cos_t2 = (theta + delta).cos();
        let sin_t2 = (theta + delta).sin();

        let p1x = cx + rx * cos_t;
        let p1y = cy + ry * sin_t;
        let p2x = cx + rx * cos_t2;
        let p2y = cy + ry * sin_t2;

        let c1x = p1x - t * (-rx * sin_t);
        let c1y = p1y - t * (ry * cos_t);
        let c2x = p2x + t * (-rx * sin_t2);
        let c2y = p2y + t * (ry * cos_t2);

        // Rotate back into user space.
        let rot = |px: f64, py: f64| -> (f64, f64) {
            (
                phi.cos() * px - phi.sin() * py,
                phi.sin() * px + phi.cos() * py,
            )
        };
        let (c1x, c1y) = rot(c1x, c1y);
        let (c2x, c2y) = rot(c2x, c2y);
        let (p2x, p2y) = rot(p2x, p2y);

        pb.cubic_to(
            c1x as f32,
            c1y as f32,
            c2x as f32,
            c2y as f32,
            p2x as f32,
            p2y as f32,
        );
        prev_x = p2x;
        prev_y = p2y;
        theta += delta;
    }
    let _ = (prev_x, prev_y);
}

fn build_fill(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Fill> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.fill, computed, gradients)?;

    let opacity = match inherited.fill_opacity {
        SVGOpacity::Opacity(op) => op,
        _ => 1.0,
    };
    let rule = match inherited.fill_rule {
        style::computed_values::fill_rule::T::Evenodd => usvg::FillRule::EvenOdd,
        _ => usvg::FillRule::NonZero,
    };

    let mut fill = usvg::Fill::new(paint);
    fill.opacity = usvg::Opacity::new(opacity).unwrap_or(usvg::Opacity::ONE);
    fill.rule = rule;
    Some(fill)
}

fn build_stroke(computed: &ComputedValues, gradients: &Gradients) -> Option<usvg::Stroke> {
    let inherited = computed.get_inherited_svg();
    let paint = resolve_paint(&inherited.stroke, computed, gradients)?;

    let width = match &inherited.stroke_width {
        SVGLength::LengthPercentage(nn_lp) => nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0),
        _ => 1.0,
    };
    if width <= 0.0 {
        return None;
    }

    let mut stroke = usvg::Stroke::new(paint);
    stroke.width = usvg::StrokeWidth::new(width).unwrap_or(usvg::StrokeWidth::new(1.0).unwrap());
    stroke.opacity = match inherited.stroke_opacity {
        SVGOpacity::Opacity(op) => usvg::Opacity::new(op).unwrap_or(usvg::Opacity::ONE),
        _ => usvg::Opacity::ONE,
    };
    stroke.linecap = match inherited.stroke_linecap {
        style::computed_values::stroke_linecap::T::Round => usvg::LineCap::Round,
        style::computed_values::stroke_linecap::T::Square => usvg::LineCap::Square,
        _ => usvg::LineCap::Butt,
    };
    stroke.linejoin = match inherited.stroke_linejoin {
        style::computed_values::stroke_linejoin::T::Round => usvg::LineJoin::Round,
        style::computed_values::stroke_linejoin::T::Bevel => usvg::LineJoin::Bevel,
        _ => usvg::LineJoin::Miter,
    };
    stroke.miterlimit = usvg::StrokeMiterlimit::new(inherited.stroke_miterlimit.0);
    if let SVGStrokeDashArray::Values(vs) = &inherited.stroke_dasharray {
        if !vs.is_empty() {
            stroke.dasharray = Some(
                vs.iter()
                    .map(|v| v.0.to_length().map(|l| l.px()).unwrap_or(0.0))
                    .collect(),
            );
        }
    }
    stroke.dashoffset = match &inherited.stroke_dashoffset {
        SVGLength::LengthPercentage(lp) => lp.to_length().map(|l| l.px()).unwrap_or(0.0),
        _ => 0.0,
    };

    Some(stroke)
}

fn resolve_paint(
    svg_paint: &SVGPaint,
    computed: &ComputedValues,
    gradients: &Gradients,
) -> Option<usvg::Paint> {
    match &svg_paint.kind {
        SVGPaintKind::Color(color) => {
            let current_color = computed.clone_color();
            let absolute = color.resolve_to_absolute(&current_color);
            let srgb = absolute.to_color_space(ColorSpace::Srgb);
            Some(usvg::Paint::Color(usvg::Color::new_rgb(
                (srgb.components.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                (srgb.components.2.clamp(0.0, 1.0) * 255.0).round() as u8,
            )))
        },
        SVGPaintKind::None => None,
        SVGPaintKind::PaintServer(url) => {
            let fragment = match url {
                style::url::ComputedUrl::Valid(u) => u.fragment().map(|s| s.to_string()),
                style::url::ComputedUrl::Invalid(s) => {
                    let trimmed = s.trim_start_matches('#');
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                },
            };
            let fragment = fragment?;
            if let Some(g) = gradients.linear.get(&fragment) {
                Some(usvg::Paint::LinearGradient(g.clone()))
            } else if let Some(g) = gradients.radial.get(&fragment) {
                Some(usvg::Paint::RadialGradient(g.clone()))
            } else {
                // Unresolved paint server: fall back to black, matching usvg's
                // behaviour when a referenced gradient is missing.
                Some(usvg::Paint::Color(usvg::Color::black()))
            }
        },
        _ => None,
    }
}

fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

fn lp_or_auto_to_f32(lp: &NonNegativeLengthPercentageOrAuto) -> Option<f32> {
    match lp {
        GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
            Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0))
        },
        GenericLengthPercentageOrAuto::Auto => None,
    }
}

/// Parses a length attribute, returning `default` when missing or unparseable.
fn length_attr(element: &ServoLayoutElement<'_>, attr: &str, default: f32) -> f32 {
    length_attr_opt(element, attr).unwrap_or(default)
}

fn length_attr_opt(element: &ServoLayoutElement<'_>, attr: &str) -> Option<f32> {
    element
        .attribute_as_str(&ns!(), &LocalName::from(attr))
        .and_then(parse_length_attr)
}

fn parse_length_attr(value: &str) -> Option<f32> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value.parse::<f32>().ok()
}

fn parse_transform(value: &str) -> usvg::Transform {
    let mut transform = usvg::Transform::identity();
    for token in TransformListParser::from(value) {
        let Ok(token) = token else {
            continue;
        };
        let t = match token {
            TransformListToken::Matrix { a, b, c, d, e, f } => usvg::Transform::from_row(
                a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
            ),
            TransformListToken::Translate { tx, ty } => {
                usvg::Transform::from_translate(tx as f32, ty as f32)
            },
            TransformListToken::Scale { sx, sy } => usvg::Transform::from_scale(sx as f32, sy as f32),
            TransformListToken::Rotate { angle } => {
                usvg::Transform::from_rotate(angle as f32)
            },
            TransformListToken::SkewX { angle } => {
                usvg::Transform::from_row(1.0, 0.0, (angle as f32).to_radians().tan(), 1.0, 0.0, 0.0)
            },
            TransformListToken::SkewY { angle } => {
                usvg::Transform::from_row(1.0, (angle as f32).to_radians().tan(), 0.0, 1.0, 0.0, 0.0)
            },
        };
        transform = transform.pre_concat(t);
    }
    transform
}

fn parse_color(value: &str) -> usvg::Color {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            return usvg::Color::new_rgb(r, g, b);
        }
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            return usvg::Color::new_rgb(r, g, b);
        }
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let mut nums = inner.split(',');
        let r = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        let g = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        let b = nums.next().and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        return usvg::Color::new_rgb(r, g, b);
    }
    match value {
        "black" => usvg::Color::black(),
        "white" => usvg::Color::white(),
        "red" => usvg::Color::new_rgb(255, 0, 0),
        "green" => usvg::Color::new_rgb(0, 128, 0),
        "blue" => usvg::Color::new_rgb(0, 0, 255),
        "none" | "transparent" => usvg::Color::new_rgb(0, 0, 0),
        _ => usvg::Color::black(),
    }
}

/// Rasterizes `tree` into an RGBA8 pixmap at `raster_size`, uploads the raw pixels to
/// WebRender keyed by `(node, size)`, and returns the resulting [`ImageKey`].
///
/// This runs synchronously on the layout thread, replacing the asynchronous
/// vector-image cache rasterization path for inline SVGs built from the DOM.
pub(crate) fn rasterize_svg_tree(
    image_cache: &dyn ImageCache,
    tree: &usvg::Tree,
    node: OpaqueNode,
    raster_size: DeviceIntSize,
) -> Option<ImageKey> {
    const MAX_SVG_PIXMAP_DIMENSION: i32 = 5000;

    let natural_size = tree.size().to_int_size();
    if natural_size.width() == 0 || natural_size.height() == 0 {
        return None;
    }

    let width = raster_size.width.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;
    let height = raster_size.height.clamp(1, MAX_SVG_PIXMAP_DIMENSION) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let transform = usvg::Transform::from_scale(
        width as f32 / natural_size.width() as f32,
        height as f32 / natural_size.height() as f32,
    );
    resvg::render(tree, transform, &mut pixmap.as_mut());
    let bytes = pixmap.take();

    // Key by (DOM node, raster size) so a re-layout of the same SVG after a mutation
    // updates the pixels in place rather than leaking a new WebRender image key.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.id().hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    let hash = hasher.finish();

    image_cache.upload_raw_pixels(hash, bytes, width, height);
    image_cache.raw_pixel_image_key(hash)
}
