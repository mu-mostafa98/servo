/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG render tree construction — assembles an [`SvgRenderTree`] from DOM nodes.
//!
//! Uses the **Builder pattern**: [`SvgRenderTreeBuilder`] accumulates state
//! (CSS rules, definition maps) through chained methods, then produces the
//! final tree via [`build`](SvgRenderTreeBuilder::build).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use svg_engine::render_tree::*;
use svg_engine::style::gradient::GradientDef;
use svg_engine::visitor::PaintServerFixupVisitor;
use web_atoms::ns;

use super::css::collect_svg_css_rules;
use super::defines::{
    ClipPathParser, DefinitionCollector, FilterParser, GradientParser, MaskParser, PatternParser,
};
use super::geometry::{build_shape, build_text};
use super::style::build_style;
use super::viewport::extract_viewport_info;
use crate::context::LayoutContext;

// ======================= Builder =======================

/// Builds an [`SvgRenderTree`] from a DOM SVG element.
pub(crate) struct SvgRenderTreeBuilder<'dom, 'a> {
    root_node: ServoLayoutNode<'dom>,
    context: &'a LayoutContext<'a>,
    css_rules: HashMap<String, HashMap<String, String>>,
}

impl<'dom, 'a> SvgRenderTreeBuilder<'dom, 'a> {
    /// Start building from an SVG DOM element node.
    pub(crate) fn new(node: ServoLayoutNode<'dom>, context: &'a LayoutContext<'a>) -> Self {
        let css_rules = collect_svg_css_rules(node);
        SvgRenderTreeBuilder {
            root_node: node,
            context,
            css_rules,
        }
    }

    /// Build the complete [`SvgRenderTree`].
    pub(crate) fn build(self) -> Option<Arc<SvgRenderTree>> {
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new())?;
        let viewport = extract_viewport_info(self.root_node);
        let definitions = collect_definitions(self.root_node, self.context);

        let mut tree = SvgRenderTree {
            root,
            viewport,
            gradients: definitions.gradients,
            clip_paths: definitions.clip_paths,
            patterns: definitions.patterns,
            masks: definitions.masks,
            filters: definitions.filters,
        };

        // Post-process: convert PaintServer::Gradient → PaintServer::Pattern
        // when the referenced ID is actually a pattern definition.
        let patterns = tree.patterns.clone();
        let mut visitor = PaintServerFixupVisitor {
            pattern_ids: &patterns,
        };
        tree.visit_mut(&mut visitor);

        Some(Arc::new(tree))
    }

    /// Recursively build a render node from a DOM node.
    fn build_render_node(
        &self,
        node: ServoLayoutNode<'dom>,
        root_node: ServoLayoutNode<'dom>,
        resolving: &mut HashSet<String>,
    ) -> Option<SvgRenderNode> {
        let element = node.as_element()?;
        let tag_name = element.local_name().as_ref().to_owned();

        // Text / tspan — extract text content from DOM children.
        if tag_name == "text" || tag_name == "tspan" {
            return build_text_node(node, self.context, &self.css_rules);
        }

        let computed = element
            .style_data()
            .is_some()
            .then(|| node.style(&self.context.style_context));
        let tag = build_tag(&element, computed.as_ref().map(|v| &**v))?;
        let (style, transforms) = build_style(node, self.context, &self.css_rules);
        let id = extract_id(&element);
        let children = resolve_children(node, &tag, root_node, self, resolving);

        Some(SvgRenderNode {
            id,
            tag,
            style,
            transforms,
            children,
        })
    }
}

// ======================= Text / Tspan Node =======================

/// Build a [`SvgRenderNode`] for `<text>` or `<tspan>` by extracting
/// text content from the DOM node and its `<tspan>` descendants, then
/// shaping the text using the font subsystem for glyph positioning.
fn build_text_node(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &HashMap<String, HashMap<String, String>>,
) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let fs: f32 = 16.0;
    let get = |name: &str| super::style::get_attr(&element, name);
    let mut span = build_text(node, &get, fs)?;
    // Shape text with the font subsystem for accurate glyph positions.
    shape_text_span(&mut span, node, context);
    let tag = SvgTag::Text(span);
    let (style, transforms) = build_style(node, context, css_rules);
    let id = extract_id(&element);
    Some(SvgRenderNode {
        id,
        tag,
        style,
        transforms,
        children: vec![],
    })
}

/// Shape a [`TextSpan`]'s text using the font subsystem.
/// Falls back gracefully to estimated widths if any step fails.
fn shape_text_span(span: &mut TextSpan, node: ServoLayoutNode, context: &LayoutContext) {
    use layout_api::LayoutNode;
    use svg_engine::text::ShapedGlyph;

    if span.text.is_empty() {
        return;
    }

    // Build a font group from the element's computed style.
    let Some(font_group) = (|| {
        let element = node.as_element()?;
        if !element.style_data().is_some() {
            return None;
        }
        let computed = node.style(&context.style_context);
        let font_style = computed.clone_font();
        let font_size = font_style.font_size.computed_size().px();
        if font_size <= 0.0 {
            return None;
        }
        Some(context.font_context.font_group(font_style))
    })() else {
        return;
    };

    let language: icu_locid::subtags::Language = "und".parse().unwrap();
    let mut glyphs = Vec::with_capacity(span.text.len());
    let mut x = 0.0f32;
    let chars: Vec<char> = span.text.chars().collect();
    let mut font_instance_key = None;

    for (i, &ch) in chars.iter().enumerate() {
        let next_ch = chars.get(i + 1).copied();
        if let Some(font) =
            font_group.find_by_codepoint(&*context.font_context, ch, next_ch, language)
        {
            if font_instance_key.is_none() {
                font_instance_key = Some(font.key(context.painter_id, &*context.font_context));
            }
            if let Some(glyph_id) = font.glyph_index(ch) {
                let advance = font.glyph_h_advance(glyph_id) as f32;
                glyphs.push(ShapedGlyph {
                    x,
                    y: 0.0,
                    advance,
                    glyph_id: glyph_id as u32,
                    character: ch,
                });
                x += advance + span.dx.get(i).copied().unwrap_or(0.0);
                continue;
            }
        }
        // Fallback: 8px per character.
        let advance = 8.0f32;
        glyphs.push(ShapedGlyph {
            x,
            y: 0.0,
            advance,
            glyph_id: 0,
            character: ch,
        });
        x += advance + span.dx.get(i).copied().unwrap_or(0.0);
    }
    span.glyphs = glyphs;
    span.font_instance_key = font_instance_key;
}

// ======================= Children Resolution =======================

/// Resolve children for a render node.
/// For `<use>`, clones the referenced element with x/y translation.
/// For all others, recursively builds children from DOM.
fn resolve_children<'dom>(
    node: ServoLayoutNode<'dom>,
    tag: &SvgTag,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgRenderTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    if let SvgTag::Container(Container::Use) = tag {
        resolve_use_children(node, root_node, builder, resolving)
    } else {
        node.dom_children()
            .filter_map(|child| builder.build_render_node(child, root_node, resolving))
            .collect()
    }
}

/// Resolve children for a `<use>` element.
///
/// Looks up the referenced element by its `#id`, builds its render node,
/// clones it as a child, and applies x/y translation if specified.
fn resolve_use_children<'dom>(
    node: ServoLayoutNode<'dom>,
    root_node: ServoLayoutNode<'dom>,
    builder: &SvgRenderTreeBuilder<'dom, '_>,
    resolving: &mut HashSet<String>,
) -> Vec<SvgRenderNode> {
    let element = node.as_element().unwrap();

    // Extract href reference.
    let ref_id = element
        .attribute_as_str(&ns!(), &local_name!("href"))
        .or_else(|| element.attribute_as_str(&ns!(), &local_name!("xlink:href")))
        .and_then(|h| {
            let t = h.trim_start_matches('#');
            if t.is_empty() {
                None
            } else {
                Some(t.to_owned())
            }
        });

    let Some(ref_id) = ref_id else { return vec![] };
    if resolving.contains(&ref_id) {
        return vec![];
    }
    resolving.insert(ref_id.clone());

    // Parse x/y offset.
    let parse_coord = |attr: &str| -> Option<f32> {
        element
            .attribute_as_str(&ns!(), &LocalName::from(attr))
            .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
    };
    let offset = (parse_coord("x"), parse_coord("y"));

    // Build target and clone with optional translation.
    let result = find_element_by_id(root_node, &ref_id)
        .and_then(|target| builder.build_render_node(target, root_node, resolving))
        .map(|target_node| {
            if let (Some(dx), Some(dy)) = offset {
                if dx != 0.0 || dy != 0.0 {
                    let mut cloned = target_node;
                    cloned.transforms.insert(
                        0,
                        svg_engine::style::transform_ops::TransformOp::Translate(dx, dy),
                    );
                    return vec![cloned];
                }
            }
            vec![target_node]
        })
        .unwrap_or_default();

    resolving.remove(&ref_id);
    result
}

// ======================= Definitions =======================

/// Collected definition maps from `<defs>`.
struct DefinitionMaps {
    gradients: HashMap<String, GradientDef>,
    clip_paths: HashMap<String, ClipPathDef>,
    patterns: HashMap<String, PatternDef>,
    masks: HashMap<String, MaskDef>,
    filters: HashMap<String, FilterDef>,
}

/// Collect all definition types (gradients, clip-paths, patterns, masks,
/// filters) from `<defs>` containers in the SVG subtree.
fn collect_definitions<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> DefinitionMaps {
    DefinitionMaps {
        gradients: DefinitionCollector::collect::<GradientParser>(node, context),
        clip_paths: DefinitionCollector::collect::<ClipPathParser>(node, context),
        patterns: DefinitionCollector::collect::<PatternParser>(node, context),
        masks: DefinitionCollector::collect::<MaskParser>(node, context),
        filters: DefinitionCollector::collect::<FilterParser>(node, context),
    }
}

// ======================= Tag Dispatch =======================

/// Map a DOM element's tag name to an [`SvgTag`].
fn build_tag<'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "image" => build_image_tag(element).map(SvgTag::Image),
        _ => build_shape(element, tag, computed).map(SvgTag::Shape),
    }
}

/// Build an [`SvgImage`] from element attributes.
fn build_image_tag(element: &ServoLayoutElement) -> Option<SvgImage> {
    use svg_engine::attr_parsers::parse_length;
    let fs = 16.0;
    let get = |name: &str| super::style::get_attr(element, name);
    let x = parse_length("x", &get, fs).unwrap_or(0.0);
    let y = parse_length("y", &get, fs).unwrap_or(0.0);
    let w = parse_length("width", &get, fs).unwrap_or(0.0).max(0.0);
    let h = parse_length("height", &get, fs).unwrap_or(0.0).max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let href = get("href").or_else(|| get("xlink:href"));
    Some(SvgImage {
        x,
        y,
        width: w,
        height: h,
        href,
    })
}

// ======================= Helpers =======================

/// Extract the `id` attribute from an SVG DOM element.
fn extract_id(element: &ServoLayoutElement) -> Option<String> {
    element
        .attribute_as_str(&ns!(), &local_name!("id"))
        .map(|s| s.to_string())
}

/// Recursively search the SVG DOM subtree for an element by its `id`.
fn find_element_by_id<'dom>(
    node: ServoLayoutNode<'dom>,
    target_id: &str,
) -> Option<ServoLayoutNode<'dom>> {
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
