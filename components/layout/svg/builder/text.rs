/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<text>`/`<tspan>` render-node assembly and font shaping for the SVG builder.

use std::collections::HashMap;

use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::element::text::{TextAnchor, TextSpan};
use svg_engine::element::{Container, SvgNode, SvgTag};

use super::{extract_id, font_key_to_resource};
use crate::context::LayoutContext;
use crate::svg::primitives::attrs::get_attr;
use crate::svg::primitives::text::{build_text, build_text_run};
use crate::svg::style::build_style;

/// Build a [`SvgNode`] for `<text>` or `<tspan>`.
///
/// For `<tspan>` (or a standalone `<text>` with no element children), the
/// node is a single [`SvgTag::Text`] span shaped with the node's own font.
///
/// For `<text>` with mixed bare-text / `<tspan>` children, the node is a
/// [`SvgTag::Container`](`Container::Text`) whose children are one
/// [`SvgTag::Text`] run per bare text node / `<tspan>`. Each run keeps its
/// own style (so per-tspan `fill` and `font-size` apply) and is positioned
/// with a cumulative `advance_offset` so runs flow left-to-right on one line.
pub(crate) fn build_text_node(
    node: ServoLayoutNode,
    context: &LayoutContext,
    css_rules: &HashMap<String, HashMap<String, String>>,
) -> Option<SvgNode> {
    let element = node.as_element()?;
    let fs: f32 = 16.0;
    let get = |name: &str| get_attr(&element, name);

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
        return Some(SvgNode {
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
        return Some(SvgNode {
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
        children.push(SvgNode {
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
    Some(SvgNode {
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
    let parent_elem = node.as_element().unwrap();
    // The <text>'s x/y is the line origin. Every run inherits it as the base
    // position; a <tspan> may override x/y explicitly. Horizontal flow between
    // runs is handled separately by advance_offset (cumulative advance +
    // anchor shift), so all runs share the same x/y base.
    let parent_x = get_attr(&parent_elem, "x")
        .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(0.0);
    let parent_y = get_attr(&parent_elem, "y")
        .and_then(|s| s.trim_end_matches("px").parse::<f32>().ok())
        .unwrap_or(0.0);
    let children: Vec<_> = node.dom_children().collect();
    let mut runs = Vec::new();
    for (i, child) in children.iter().enumerate() {
        if let Some(child_elem) = child.as_element() {
            if child_elem.local_name().as_ref() == "tspan" {
                let get = |n: &str| get_attr(&child_elem, n);
                if let Some(mut span) = build_text(*child, &get, fs) {
                    // Inherit the <text>'s baseline/origin for any axis the
                    // <tspan> does not set explicitly.
                    if get_attr(&child_elem, "x").is_none() {
                        span.x = parent_x;
                    }
                    if get_attr(&child_elem, "y").is_none() {
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
            let get = |n: &str| get_attr(&parent_elem, n);
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
    use svg_engine::element::text::{DominantBaseline, ShapedGlyph};
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

    // Record the resolved font size on the span so the renderer can size the
    // glyph clip rect's ascent/descent (the fallback estimate is too small for
    // large font sizes, clipping the top of tall glyphs).
    span.font_size = font_size;

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
            alternates: Default::default(),
            flags: if span.rtl {
                ShapingFlags::RTL_FLAG
            } else {
                ShapingFlags::empty()
            },
        };

        let key = font_key_to_resource(font.key(context.painter_id, &*context.font_context));
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
