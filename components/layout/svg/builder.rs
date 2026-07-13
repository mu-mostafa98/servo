use std::sync::Arc;

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use svg_engine::shapes::{AttrAccessor, BuildFromElement, *};
use web_atoms::ns;

use super::style::build_style;
use crate::context::LayoutContext;
use crate::dom::NodeExt;

/// Wraps a DOM element reference to implement `AttrAccessor`.
struct ElementAccessor<'a> {
    element: &'a ServoLayoutElement<'a>,
}

impl<'a> AttrAccessor for ElementAccessor<'a> {
    fn get_attr(&self, name: &str) -> Option<String> {
        self.element
            .attribute_as_str(&ns!(), &LocalName::from(name))
            .map(|s| s.to_string())
    }
}

pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        SvgRenderTreeBuilder {
            root_node: node,
            context,
        }
    }

    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node)?;
        let viewport = extract_viewport_info(self.root_node);

        Some(Arc::new(SvgRenderTree { root, viewport }))
    }

    fn build_render_node(&self, node: ServoLayoutNode<'dom>) -> Option<SvgRenderNode> {
        let element = node.as_element()?;
        let accessor = ElementAccessor { element: &element };
        let tag = build_tag(&accessor, &element)?;

        let style = match element.style_data() {
            Some(_) => {
                let computed = element.style(&self.context.style_context);
                build_style(&computed)
            },
            None => {
                let style_str = element
                    .attribute_as_str(&ns!(), &LocalName::from("style"))
                    .unwrap_or("");
                build_style_from_attr(style_str)
            },
        };

        let id = element
            .attribute_as_str(&ns!(), &html5ever::local_name!("id"))
            .map(|s| s.to_string());

        let children = node
            .dom_children()
            .filter_map(|child| self.build_render_node(child))
            .collect();

        Some(SvgRenderNode {
            id,
            tag,
            style,
            children,
        })
    }
}

fn build_tag(accessor: &ElementAccessor, _element: &ServoLayoutElement) -> Option<SvgTag> {
    let name = _element.local_name().as_ref();
    match name {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "rect" => {
            let rect = Rectangle::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Rect(rect)))
        },
        "circle" => {
            let circle = Circle::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Circle(circle)))
        },
        "ellipse" => {
            let ellipse = Ellipse::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Ellipse(ellipse)))
        },
        "line" => {
            let line = Line::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Line(line)))
        },
        "polyline" => {
            let polyline = Polyline::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Polyline(polyline)))
        },
        "polygon" => {
            let polygon = Polygon::from_attrs(16.0, accessor)?;
            Some(SvgTag::Shape(Shape::Polygon(polygon)))
        },
        _ => None,
    }
}

fn build_style_from_attr(style_str: &str) -> svg_engine::style::NodeStyle {
    use svg_engine::style::color::parse_css_color;
    use svg_engine::style::*;
    use svgtypes::Color as SvgColor;

    let mut fill_color: Option<SvgColor> = None;
    let mut fill_opacity: f32 = 1.0;
    let mut fill_rule = FillRule::NonZero;
    let mut stroke_color: Option<SvgColor> = None;
    let mut stroke_opacity: f32 = 1.0;
    let mut stroke_width: f32 = 1.0;
    let mut has_stroke_width = false;

    for decl in style_str.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let parts: Vec<&str> = decl.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        let prop = parts[0].trim();
        let val = parts[1].trim();

        match prop {
            "fill" => {
                fill_color = parse_css_color(val);
            },
            "fill-opacity" => {
                if let Ok(v) = val.parse::<f32>() {
                    fill_opacity = v.clamp(0.0, 1.0);
                }
            },
            "fill-rule" => {
                fill_rule = if val == "evenodd" {
                    FillRule::EvenOdd
                } else {
                    FillRule::NonZero
                };
            },
            "stroke" => {
                stroke_color = parse_css_color(val);
            },
            "stroke-width" => {
                let v = val.trim_end_matches("px").trim();
                if let Ok(w) = v.parse::<f32>() {
                    stroke_width = w.max(0.0);
                    has_stroke_width = true;
                }
            },
            "stroke-opacity" => {
                if let Ok(v) = val.parse::<f32>() {
                    stroke_opacity = v.clamp(0.0, 1.0);
                }
            },
            "opacity" => {
                if let Ok(v) = val.parse::<f32>() {
                    fill_opacity *= v;
                    stroke_opacity *= v;
                }
            },
            _ => {},
        }
    }

    NodeStyle {
        fill: fill_color.map(|c| FillParams {
            color: Some(c),
            opacity: fill_opacity,
            fill_rule,
        }),
        stroke: stroke_color.map(|c| StrokeParams {
            color: Some(c),
            opacity: stroke_opacity,
            width: if has_stroke_width { stroke_width } else { 1.0 },
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            dash_array: None,
            dash_offset: 0.0,
        }),
        ..Default::default()
    }
}

fn extract_viewport_info(node: ServoLayoutNode<'_>) -> ViewportInfo {
    let element = match node.as_element() {
        Some(e) => e,
        None => {
            return ViewportInfo {
                width: 300.0,
                height: 150.0,
            };
        },
    };

    let get_attr = |attr: &str| {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .map(|s| s.to_string())
    };

    let svg_width = get_attr("width")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(300.0);
    let svg_height = get_attr("height")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(150.0);

    ViewportInfo {
        width: svg_width,
        height: svg_height,
    }
}
