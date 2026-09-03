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
use svg_engine::style::NodeStyle;
use svg_engine::style::gradient::GradientDef;
use svg_engine::text::TextAnchor;
use svg_engine::visitor::PaintServerFixupVisitor;
use web_atoms::ns;

use super::css::collect_svg_css_rules;
use super::defines::{
    ClipPathParser, DefinitionCollector, FilterParser, GradientParser, MarkerParser, MaskParser,
    PatternParser,
};
use super::geometry::{build_shape, build_text};
use super::style::build_style;
use super::viewport::{extract_nested_viewport, extract_viewport_info};
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
        let root = self.build_render_node(self.root_node, self.root_node, &mut HashSet::new(), None)?;
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
            markers: definitions.markers,
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
        inherited: Option<&NodeStyle>,
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
        let tag = build_tag(&element, computed.as_ref().map(|v| &**v), node, self.context)?;
        let (style, transforms) = build_style(node, self.context, &self.css_rules, inherited);
        let id = extract_id(&element);
        let children = resolve_children(
            node,
            &tag,
            root_node,
            self,
            resolving,
            &style,
            inherited.is_some(),
        );

        // A nested `<svg>` (any `<svg>` except the root) establishes its own
        // viewport. The root's viewport is handled via `SvgRenderTree::viewport`.
        let viewport = if tag_name == "svg" && node != root_node {
            extract_nested_viewport(node)
        } else {
            None
        };

        Some(SvgRenderNode {
            id,
            tag,
            style,
            transforms,
            viewport,
            children,
        })
    }
}

// ======================= Text / Tspan Node =======================

/// Build a [`SvgRenderNode`] for `<text>` or `<tspan>`.
///
/// For `<tspan>` (or a standalone `<text>` with no element children), the
/// node is a single [`SvgTag::Text`] span shaped with the node's own font.
///
/// For `<text>` with mixed bare-text / `<tspan>` children, the node is a
/// [`SvgTag::Container`](`Container::Text`) whose children are one
/// [`SvgTag::Text`] run per bare text node / `<tspan>`. Each run keeps its
/// own style (so per-tspan `fill` and `font-size` apply) and is positioned
/// with a cumulative `advance_offset` so runs flow left-to-right on one line.
fn build_text_node(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &HashMap<String, HashMap<String, String>>,
) -> Option<SvgRenderNode> {
    let element = node.as_element()?;
    let fs: f32 = 16.0;
    let get = |name: &str| super::style::get_attr(&element, name);

    // Collect the ordered inline runs of this element.
    let runs = collect_text_runs(node, fs);

    // No runs → maybe a bare single-span (e.g. <tspan> with only text, or
    // a <text> with no element children). Fall back to the legacy single-span
    // path so existing simple <text> usage keeps working.
    if runs.is_empty() {
        let mut span = build_text(node, &get, fs)?;
        shape_text_span(&mut span, node, context);
        let (style, transforms) = build_style(node, context, css_rules, None);
        let id = extract_id(&element);
        return Some(SvgRenderNode {
            id,
            tag: SvgTag::Text(span),
            style,
            transforms,
            viewport: None,
            children: vec![],
        });
    }

    // Single run → emit as a direct Text node (no container needed).
    if runs.len() == 1 {
        let (mut span, run_node) = runs.into_iter().next().unwrap();
        // Shape with the run's own node (the <tspan> for tspan runs, the
        // <text> itself for bare-text runs) so the run's font-size applies.
        shape_text_span(&mut span, run_node, context);
        let (style, transforms) = build_style(node, context, css_rules, None);
        let id = extract_id(&element);
        return Some(SvgRenderNode {
            id,
            tag: SvgTag::Text(span),
            style,
            transforms,
            viewport: None,
            children: vec![],
        });
    }

    // Multiple runs → a Container::Text with one Text child per run.
    // Shape each run first (so total_advance reflects real glyph widths),
    // then compute cumulative advance_offset and apply the <text>'s
    // text-anchor as a single shift on the first run.
    let shaped = runs
        .into_iter()
        .map(|(mut span, run_node)| {
            shape_text_span(&mut span, run_node, context);
            (span, run_node)
        })
        .collect::<Vec<_>>();
    let total_advance: f32 = shaped.iter().map(|(s, _)| s.total_advance()).sum();
    let anchor_shift = get("text-anchor")
        .as_deref()
        .map(|v| match v.trim() {
            "middle" => -0.5,
            "end" => -1.0,
            _ => 0.0,
        })
        .unwrap_or(0.0)
        * total_advance;

    let mut pen = anchor_shift;
    // `dy` shifts the *current* text position, so it accumulates across runs
    // (a later tspan's `dy` is relative to the position after earlier ones).
    let mut dy_pen = 0.0f32;
    let mut children = Vec::with_capacity(shaped.len());
    for (mut span, run_node) in shaped {
        span.advance_offset = pen;
        // The whole-line anchor shift is already folded into `advance_offset`,
        // so clear each run's own text-anchor to avoid double-applying it.
        span.text_anchor = TextAnchor::Start;
        // Offset this run by the accumulated vertical shift from preceding runs.
        span.y += dy_pen;
        dy_pen += span.dy.iter().sum::<f32>();
        pen += span.total_advance();
        let (run_style, run_transforms) = build_style(run_node, context, css_rules, None);
        let run_id = extract_id(&run_node.as_element()?);
        children.push(SvgRenderNode {
            id: run_id,
            tag: SvgTag::Text(span),
            style: run_style,
            transforms: run_transforms,
            viewport: None,
            children: vec![],
        });
    }

    let (style, transforms) = build_style(node, context, css_rules, None);
    let id = extract_id(&element);
    Some(SvgRenderNode {
        id,
        tag: SvgTag::Container(Container::Text),
        style,
        transforms,
        viewport: None,
        children,
    })
}

/// An ordered inline run within a `<text>`: the span data plus the DOM node
/// it inherits style/font from (the `<tspan>` for tspan runs, the `<text>`
/// itself for bare-text runs). The `ServoLayoutNode` lifetime is elided to
/// match the enclosing function signatures.
type RunWithNode<'dom> = (TextSpan, ServoLayoutNode<'dom>);

/// Collect the ordered inline runs of a `<text>` (or `<tspan>`) element.
///
/// Each bare text node becomes a run that inherits the parent element's
/// attributes; each `<tspan>` child becomes a run carrying its own attributes
/// (`fill`, `font-size`, `x`/`y`, `dx`/`dy`, `text-anchor`). Pure-whitespace
/// text between tspans (indentation/newlines) is dropped so it does not render
/// as missing-glyph boxes.
fn collect_text_runs<'dom>(node: ServoLayoutNode<'dom>, fs: f32) -> Vec<RunWithNode<'dom>> {
    use super::geometry::build_text_run;
    let parent_elem = node.as_element().unwrap();
    // The <text>'s x/y is the line origin. Every run inherits it as the base
    // position; a <tspan> may override x/y explicitly. Horizontal flow between
    // runs is handled separately by advance_offset (cumulative advance +
    // anchor shift), so all runs share the same x/y base.
    let parent_x = super::style::get_attr(&parent_elem, "x")
        .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(0.0);
    let parent_y = super::style::get_attr(&parent_elem, "y")
        .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(0.0);
    let children: Vec<_> = node.dom_children().collect();
    let mut runs = Vec::new();
    for (i, child) in children.iter().enumerate() {
        if let Some(child_elem) = child.as_element() {
            if child_elem.local_name().as_ref() == "tspan" {
                let get = |n: &str| super::style::get_attr(&child_elem, n);
                if let Some(mut span) = build_text(*child, &get, fs) {
                    // Inherit the <text>'s baseline/origin for any axis the
                    // <tspan> does not set explicitly.
                    if super::style::get_attr(&child_elem, "x").is_none() {
                        span.x = parent_x;
                    }
                    if super::style::get_attr(&child_elem, "y").is_none() {
                        span.y = parent_y;
                    }
                    runs.push((span, *child));
                }
            }
        } else {
            let t = child.text_content();
            if t.trim().is_empty() {
                continue;
            }
            // Trim leading whitespace (the text node usually starts with the
            // newline + indentation that precedes the visible text).
            let text = t.trim_start();
            let trimmed = text.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            // Strip trailing whitespace (the indentation before `</text>`), but
            // keep a single separating space when this run is followed by more
            // inline content so adjacent runs stay separated. This also stops a
            // trailing newline from shaping into a `.notdef` box and from
            // inflating the RTL anchor offset.
            let followed_by_content = children[i + 1..].iter().any(|c| match c.as_element() {
                Some(e) => e.local_name().as_ref() == "tspan",
                None => !(*c).text_content().trim().is_empty(),
            });
            let text = if followed_by_content && trimmed.len() < text.len() {
                format!("{} ", trimmed)
            } else {
                trimmed.to_owned()
            };
            // Bare-text runs always use the <text>'s x/y (no own attributes).
            let get = |n: &str| super::style::get_attr(&parent_elem, n);
            if let Some(span) = build_text_run(text, &get, fs) {
                runs.push((span, node));
            }
        }
    }
    runs
}

/// Shape a [`TextSpan`]'s text using the font subsystem (HarfBuzz), so cursive
/// scripts like Arabic get proper contextual joining. Text is grouped into runs
/// of consecutive characters that use the same fallback font, and each run is
/// shaped as a whole.
fn shape_text_span(span: &mut TextSpan, node: ServoLayoutNode, context: &LayoutContext) {
    use fonts::{ShapingFlags, ShapingOptions};
    use layout_api::LayoutNode;
    use style::computed_values::font_variant_position::T as FontVariantPosition;
    use style::values::computed::{
        FontFeatureSettings, FontVariantEastAsian, FontVariantLigatures, FontVariantNumeric,
    };
    use svg_engine::text::{DominantBaseline, ShapedGlyph};
    use unicode_script::Script;

    if span.text.is_empty() {
        return;
    }

    // Build a font group from the element's computed style, and capture the
    // resolved font size (needed for the dominant-baseline offset below).
    let Some((font_group, font_size)) = (|| {
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
        Some((context.font_context.font_group(font_style), font_size))
    })() else {
        return;
    };

    // Approximate vertical offset for `dominant-baseline` (relative to the
    // alphabetic baseline at `y`).
    let baseline_shift = match span.dominant_baseline {
        DominantBaseline::Auto => 0.0,
        DominantBaseline::Hanging => 0.8 * font_size,
        // `middle` = alphabetic + x-height/2; `central` = center of the em box
        // (= (ascent - descent) / 2), which sits a little lower than `middle`.
        DominantBaseline::Middle => 0.35 * font_size,
        DominantBaseline::Central => 0.45 * font_size,
    };

    let language: icu_locid::subtags::Language = "und".parse().unwrap();
    let mut glyphs = Vec::with_capacity(span.text.len());
    let mut pen_x = 0.0f32;
    let mut pen_y = 0.0f32;
    let chars: Vec<char> = span.text.chars().collect();
    let mut font_instance_key = None;

    let mut ci = 0;
    while ci < chars.len() {
        let Some(font) = font_group.find_by_codepoint(
            &*context.font_context,
            chars[ci],
            chars.get(ci + 1).copied(),
            language,
        ) else {
            // No font for this character — fallback. Whitespace is skipped.
            let ch = chars[ci];
            pen_x += span.dx.get(ci).copied().unwrap_or(0.0);
            pen_y += span.dy.get(ci).copied().unwrap_or(0.0);
            let advance = if ch.is_whitespace() { 4.0f32 } else { 8.0f32 };
            if !ch.is_whitespace() {
                glyphs.push(ShapedGlyph {
                    x: pen_x,
                    y: pen_y + baseline_shift,
                    advance,
                    glyph_id: 0,
                    character: ch,
                    font_instance_key: None,
                });
            }
            pen_x += advance;
            ci += 1;
            continue;
        };

        // Extend the run over consecutive characters that map to the same font.
        let mut cj = ci + 1;
        while cj < chars.len() {
            match font_group.find_by_codepoint(
                &*context.font_context,
                chars[cj],
                chars.get(cj + 1).copied(),
                language,
            ) {
                Some(next_font) if next_font == font => cj += 1,
                _ => break,
            }
        }

        // Shape the whole run (HarfBuzz handles Arabic joining, ligatures, …).
        let run_text: String = chars[ci..cj].iter().collect();
        let options = ShapingOptions {
            letter_spacing: None,
            word_spacing: None,
            script: Script::from(chars[ci]),
            language,
            ligatures: FontVariantLigatures::NORMAL,
            numeric: FontVariantNumeric::NORMAL,
            east_asian: FontVariantEastAsian::NORMAL,
            feature_settings: FontFeatureSettings::normal(),
            position: FontVariantPosition::Normal,
            flags: if span.rtl {
                ShapingFlags::RTL_FLAG
            } else {
                ShapingFlags::empty()
            },
        };

        let key = font.key(context.painter_id, &*context.font_context);
        if font_instance_key.is_none() {
            font_instance_key = Some(key);
        }

        let shaped = font.shape_text(&run_text, &options);

        // Map the shaped glyphs (already in visual order) to positions,
        // applying the per-character `dx`/`dy` (reversed for RTL).
        let mut run_char_index = 0;
        for glyph_info in shaped.glyphs() {
            let char_idx = (ci + run_char_index).min(chars.len() - 1);
            pen_x += span.dx.get(char_idx).copied().unwrap_or(0.0);
            pen_y += span.dy.get(char_idx).copied().unwrap_or(0.0);
            let advance = glyph_info.advance().to_f32_px();

            glyphs.push(ShapedGlyph {
                x: pen_x,
                y: pen_y + baseline_shift,
                advance,
                glyph_id: glyph_info.id() as u32,
                character: chars[char_idx],
                font_instance_key: Some(key),
            });
            pen_x += advance;
            run_char_index += glyph_info.character_count().max(1);
        }

        ci = cj;
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
    node_style: &NodeStyle,
    in_shadow: bool,
) -> Vec<SvgRenderNode> {
    if let SvgTag::Container(Container::Use) = tag {
        resolve_use_children(node, root_node, builder, resolving, node_style)
    } else {
        // Manual inheritance only applies inside a `<use>` shadow tree; for
        // normal content Stylo already resolves inherited properties along the
        // real DOM ancestry.
        let child_inherited = if in_shadow {
            Some(node_style)
        } else {
            None
        };
        node.dom_children()
            .filter_map(|child| builder.build_render_node(child, root_node, resolving, child_inherited))
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
    use_style: &NodeStyle,
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
    let target = find_element_by_id(root_node, &ref_id);
    let target_element = target.as_ref().and_then(|n| n.as_element());

    // The referenced element's viewport attributes, used when the target is a
    // <symbol> whose viewBox maps its internal coordinates onto the viewport
    // declared by the <use> (falling back to the symbol's own width/height).
    let parse_len = |e: &ServoLayoutElement, name: &str| -> Option<f32> {
        super::style::get_attr(e, name)
            .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
    };
    let sym_view_box = target_element
        .and_then(|e| super::style::get_attr(&e, "viewBox"))
        .as_deref()
        .and_then(extract_viewbox);
    let sym_aspect_ratio = target_element
        .and_then(|e| super::style::get_attr(&e, "preserveAspectRatio"))
        .as_deref()
        .map(parse_aspect_ratio);
    let sym_width = target_element.and_then(|e| parse_len(&e, "width"));
    let sym_height = target_element.and_then(|e| parse_len(&e, "height"));

    let result = target
        .and_then(|t| builder.build_render_node(t, root_node, resolving, Some(use_style)))
        .map(|target_node| {
            // Shared helper: apply <use> x/y offset as a translate transform.
            let apply_offset = |node: &mut SvgRenderNode| {
                if let (Some(dx), Some(dy)) = offset {
                    if dx != 0.0 || dy != 0.0 {
                        node.transforms.insert(
                            0,
                            svg_engine::style::transform_ops::TransformOp::Translate(dx, dy),
                        );
                    }
                }
            };

            // <symbol> is never rendered directly. When it carries a viewBox,
            // wrap its children in a viewport-carrying group so the traversal
            // maps the symbol's coordinates onto the <use> viewport (the same
            // viewBox → viewport machinery used for nested <svg> elements).
            if let SvgTag::Container(Container::Symbol) = &target_node.tag {
                if let Some(vb) = sym_view_box {
                    let width = parse_coord("width").or(sym_width).unwrap_or(vb.width);
                    let height = parse_coord("height").or(sym_height).unwrap_or(vb.height);
                    let wrapper = SvgRenderNode {
                        id: target_node.id,
                        tag: SvgTag::Container(Container::Group),
                        style: target_node.style,
                        transforms: Vec::new(),
                        viewport: Some(SvgViewport {
                            x: offset.0.unwrap_or(0.0),
                            y: offset.1.unwrap_or(0.0),
                            width,
                            height,
                            view_box: Some(vb),
                            aspect_ratio: sym_aspect_ratio,
                            overflow_visible: false,
                        }),
                        children: target_node.children,
                    };
                    return vec![wrapper];
                }

                // No viewBox — unwrap the children with the x/y offset applied.
                let mut children = target_node.children;
                for child in &mut children {
                    apply_offset(child);
                }
                return children;
            }

            let mut cloned = target_node;
            apply_offset(&mut cloned);
            vec![cloned]
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
    markers: HashMap<String, MarkerDef>,
}

/// Collect all definition types (gradients, clip-paths, patterns, masks,
/// filters, markers) from `<defs>` containers in the SVG subtree.
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
        markers: DefinitionCollector::collect::<MarkerParser>(node, context),
    }
}

// ======================= Tag Dispatch =======================

/// Map a DOM element's tag name to an [`SvgTag`].
fn build_tag<'dom>(
    element: &ServoLayoutElement<'dom>,
    computed: Option<&style::properties::ComputedValues>,
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
) -> Option<SvgTag> {
    let tag = element.local_name().as_ref();
    match tag {
        "svg" => Some(SvgTag::Container(Container::Svg)),
        "g" => Some(SvgTag::Container(Container::Group)),
        "defs" => Some(SvgTag::Container(Container::Defs)),
        "use" => Some(SvgTag::Container(Container::Use)),
        "symbol" => Some(SvgTag::Container(Container::Symbol)),
        "image" => build_image_tag(element, node, context).map(SvgTag::Image),
        _ => build_shape(element, tag, computed).map(SvgTag::Shape),
    }
}

/// Build an [`SvgImage`] from element attributes.
///
/// Resolves the `href`/`xlink:href` attribute to a WebRender [`ImageKey`] via
/// the layout image cache: the URL is resolved against the owner document's
/// base URL, then looked up (or requested) through `image_resolver`. When the
/// image is not yet loaded the key is `None` and the renderer draws a
/// placeholder; once it loads, a reflow re-runs this and yields `Some(key)`.
fn build_image_tag(
    element: &ServoLayoutElement,
    node: ServoLayoutNode,
    context: &LayoutContext,
) -> Option<SvgImage> {
    use layout_api::LayoutNode;
    use net_traits::request::InternalRequest;
    use net_traits::image_cache::Image;
    use layout_api::LayoutImageDestination;
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
    let get_xlink = |name: &str| {
        element.attribute_as_str(&ns!(xlink), &LocalName::from(name)).map(|s| s.to_string())
    };
    let href = get("href").or_else(|| get_xlink("href"));
    // Resolve href → ImageKey + natural dimensions. Relative URLs are resolved
    // against the owner document's base URL; data: URIs parse directly.  A
    // None/empty href, a pending load, or a decode failure all yield
    // `image_key = None`, in which case the renderer falls back to a
    // placeholder.
    let raster_data: Option<(Option<webrender_api::ImageKey>, u32, u32)> =
        href.as_deref().and_then(|href_str| {
            let base = node.base_url();
            let resolved = base.join(href_str.trim()).ok()?;
            context
                .image_resolver
                .get_cached_image_for_url(
                    node.opaque(),
                    resolved,
                    LayoutImageDestination::BoxTreeConstruction,
                    InternalRequest::No,
                )
                .ok()
                .and_then(|image| match image {
                    Image::Raster(raster) => {
                        Some((raster.id, raster.metadata.width, raster.metadata.height))
                    },
                    Image::Vector(..) => None, // vector images need rasterization; not handled here
                })
        });
    let (image_key, natural_width, natural_height) = match raster_data {
        Some((id, w, h)) => (id, Some(w), Some(h)),
        None => (None, None, None),
    };

    // Parse preserveAspectRatio — defaults to xMidYMid meet per SVG spec.
    let preserve_aspect_ratio = get("preserveAspectRatio")
        .map(|v| parse_aspect_ratio(&v))
        .unwrap_or_default();

    Some(SvgImage {
        x,
        y,
        width: w,
        height: h,
        href,
        image_key,
        natural_width,
        natural_height,
        preserve_aspect_ratio,
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
