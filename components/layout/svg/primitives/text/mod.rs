/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Text layout primitives: per-character positioning/rotation lists, span/chunk
//! collection, and textPath resolution.
//!
//! These are the building blocks used by [`crate::svg::usvg_builder::build_text`]
//! to assemble a [`usvg::Text`] node. Font resolution lives in [`font`];
//! whitespace trimming in [`whitespace`].

mod font;
mod whitespace;

pub(crate) use font::{SvgFonts, convert_font};
pub(crate) use whitespace::{TrimmedTexts, trim_text_tree};

use std::collections::HashMap;
use std::sync::Arc;

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType, LayoutNode};
use resvg::usvg;
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use style::dom::NodeInfo;
use style::properties::ComputedValues;
use svgtypes::LengthUnit;

use crate::context::LayoutContext;
use crate::svg::primitives::paint::{Gradients, build_fill, build_stroke};
use crate::svg::primitives::attrs::{
    element_id, element_layout_type, length_attr_opt, parse_number_list, parse_transform,
};
use crate::svg::primitives::shape::resolve_shape_path;

/// A text character position. _Character_ is a Unicode codepoint, per SVG 2.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CharacterPosition {
    /// An absolute X axis position.
    pub(crate) x: Option<f32>,
    /// An absolute Y axis position.
    pub(crate) y: Option<f32>,
    /// A relative X axis offset.
    pub(crate) dx: Option<f32>,
    /// A relative Y axis offset.
    pub(crate) dy: Option<f32>,
}

/// State threaded through the chunk-collection walk.
struct IterState {
    chars_count: usize,
    chunk_bytes_count: usize,
    split_chunk: bool,
    text_flow: usvg::TextFlow,
    chunks: Vec<usvg::TextChunk>,
}

/// Counts the Unicode codepoints across all text descendants of `node`, using
/// the whitespace-trimmed text.
fn count_chars(node: ServoLayoutNode<'_>, texts: &TrimmedTexts) -> usize {
    node.dom_children()
        .map(|child| {
            if child.as_element().is_some() {
                count_chars(child, texts)
            } else {
                texts
                    .get(&child.opaque())
                    .map(|s| s.chars().count())
                    .unwrap_or(0)
            }
        })
        .sum()
}

/// Resolves per-character `x`/`y`/`dx`/`dy` positions, ported from usvg's
/// `resolve_positions_list`.
pub(crate) fn resolve_positions_list(
    node: ServoLayoutNode<'_>,
    texts: &TrimmedTexts,
) -> Vec<CharacterPosition> {
    let mut list = vec![
        CharacterPosition {
            x: None,
            y: None,
            dx: None,
            dy: None,
        };
        count_chars(node, texts)
    ];
    let mut offset = 0usize;
    resolve_positions_impl(node, &mut list, &mut offset, texts);
    list
}

fn resolve_positions_impl(
    node: ServoLayoutNode<'_>,
    list: &mut [CharacterPosition],
    offset: &mut usize,
    texts: &TrimmedTexts,
) {
    if let Some(element) = node.as_element() {
        let ty = element_layout_type(&element);
        if matches!(
            ty,
            LayoutElementType::SVGTextElement | LayoutElementType::SVGTSpanElement
        ) {
            let child_chars = count_chars(node, texts);
            macro_rules! push_list {
                ($attr:literal, $field:ident) => {
                    if let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from($attr)) {
                        let nums = parse_number_list(value);
                        let len = nums.len().min(child_chars);
                        for i in 0..len {
                            list[*offset + i].$field = Some(nums[i]);
                        }
                    }
                };
            }
            push_list!("x", x);
            push_list!("y", y);
            push_list!("dx", dx);
            push_list!("dy", dy);
        }
        for child in node.dom_children() {
            resolve_positions_impl(child, list, offset, texts);
        }
    } else if node.is_text_node() {
        *offset += texts
            .get(&node.opaque())
            .map(|s| s.chars().count())
            .unwrap_or(0);
    }
}

/// Resolves per-character rotation, ported from usvg's `resolve_rotate_list`.
pub(crate) fn resolve_rotate_list(node: ServoLayoutNode<'_>, texts: &TrimmedTexts) -> Vec<f32> {
    let mut list = vec![0.0; count_chars(node, texts)];
    let mut last = 0.0;
    let mut offset = 0usize;
    resolve_rotate_impl(node, &mut list, &mut offset, &mut last, texts);
    list
}

fn resolve_rotate_impl(
    node: ServoLayoutNode<'_>,
    list: &mut [f32],
    offset: &mut usize,
    last: &mut f32,
    texts: &TrimmedTexts,
) {
    if let Some(element) = node.as_element() {
        if let Some(value) = element.attribute_as_str(&ns!(), &LocalName::from("rotate")) {
            let rotate = parse_number_list(value);
            let child_chars = count_chars(node, texts);
            for i in 0..child_chars {
                if let Some(a) = rotate.get(i).copied() {
                    list[*offset + i] = a;
                    *last = a;
                } else {
                    list[*offset + i] = *last;
                }
            }
        }
        for child in node.dom_children() {
            resolve_rotate_impl(child, list, offset, last, texts);
        }
    } else if node.is_text_node() {
        *offset += texts
            .get(&node.opaque())
            .map(|s| s.chars().count())
            .unwrap_or(0);
    }
}

fn text_anchor(computed: &ComputedValues) -> usvg::TextAnchor {
    match computed.get_inherited_svg().text_anchor {
        style::computed_values::text_anchor::T::Middle => usvg::TextAnchor::Middle,
        style::computed_values::text_anchor::T::End => usvg::TextAnchor::End,
        _ => usvg::TextAnchor::Start,
    }
}

fn dominant_baseline(computed: &ComputedValues) -> usvg::DominantBaseline {
    use style::values::computed::DominantBaseline;
    match computed.get_inherited_box().dominant_baseline {
        DominantBaseline::Alphabetic => usvg::DominantBaseline::Alphabetic,
        DominantBaseline::Ideographic => usvg::DominantBaseline::Ideographic,
        DominantBaseline::Hanging => usvg::DominantBaseline::Hanging,
        DominantBaseline::Mathematical => usvg::DominantBaseline::Mathematical,
        DominantBaseline::Central => usvg::DominantBaseline::Central,
        DominantBaseline::Middle => usvg::DominantBaseline::Middle,
        DominantBaseline::TextTop => usvg::DominantBaseline::TextBeforeEdge,
        DominantBaseline::TextBottom => usvg::DominantBaseline::TextAfterEdge,
        _ => usvg::DominantBaseline::Auto,
    }
}

fn alignment_baseline(computed: &ComputedValues) -> usvg::AlignmentBaseline {
    use style::values::computed::AlignmentBaseline;
    match computed.get_box().alignment_baseline {
        AlignmentBaseline::Baseline => usvg::AlignmentBaseline::Baseline,
        AlignmentBaseline::TextBottom => usvg::AlignmentBaseline::TextAfterEdge,
        AlignmentBaseline::Middle => usvg::AlignmentBaseline::Middle,
        AlignmentBaseline::TextTop => usvg::AlignmentBaseline::TextBeforeEdge,
    }
}

fn baseline_shift(computed: &ComputedValues) -> Vec<usvg::BaselineShift> {
    use style::values::computed::BaselineShift as ServoBaselineShift;
    match &computed.get_box().baseline_shift {
        ServoBaselineShift::Keyword(kw) => match kw {
            style::values::generics::box_::BaselineShiftKeyword::Sub => {
                vec![usvg::BaselineShift::Subscript]
            },
            style::values::generics::box_::BaselineShiftKeyword::Super => {
                vec![usvg::BaselineShift::Superscript]
            },
            _ => Vec::new(),
        },
        ServoBaselineShift::Length(lp) => match lp.to_length() {
            Some(length) if length.px() != 0.0 => vec![usvg::BaselineShift::Number(length.px())],
            _ => Vec::new(),
        },
    }
}

fn letter_spacing(computed: &ComputedValues) -> f32 {
    computed
        .get_inherited_text()
        .letter_spacing
        .0
        .to_length()
        .map(|l| l.px())
        .unwrap_or(0.0)
}

fn word_spacing(computed: &ComputedValues) -> f32 {
    computed
        .get_inherited_text()
        .word_spacing
        .to_length()
        .map(|l| l.px())
        .unwrap_or(0.0)
}

fn length_adjust(element: &ServoLayoutElement<'_>) -> usvg::LengthAdjust {
    match element.attribute_as_str(&ns!(), &LocalName::from("lengthAdjust")) {
        Some("spacingAndGlyphs") => usvg::LengthAdjust::SpacingAndGlyphs,
        _ => usvg::LengthAdjust::Spacing,
    }
}

pub(crate) fn convert_writing_mode(element: &ServoLayoutElement<'_>) -> usvg::WritingMode {
    match element.attribute_as_str(&ns!(), &LocalName::from("writing-mode")) {
        Some("tb") | Some("tb-rl") | Some("vertical-rl") | Some("vertical-lr") => {
            usvg::WritingMode::TopToBottom
        },
        _ => usvg::WritingMode::LeftToRight,
    }
}

pub(crate) fn convert_direction(computed: &ComputedValues) -> usvg::TextDirection {
    match computed.get_inherited_box().direction {
        style::computed_values::direction::T::Rtl => usvg::TextDirection::RightToLeft,
        _ => usvg::TextDirection::LeftToRight,
    }
}

/// Builds the `text-decoration` for a span from the computed
/// `text-decoration-line` (wired into presentational hints in `svgelement.rs`).
fn text_decoration(
    computed: &ComputedValues,
    gradients: &Gradients,
    diagonal: f32,
) -> usvg::TextDecoration {
    let line = computed.get_text().text_decoration_line;
    let make_deco = |has: bool| -> Option<usvg::TextDecorationStyle> {
        if !has {
            return None;
        }
        Some(usvg::TextDecorationStyle {
            fill: build_fill(computed, gradients),
            stroke: build_stroke(computed, gradients, diagonal),
        })
    };

    usvg::TextDecoration {
        underline: make_deco(
            line.contains(style::values::specified::TextDecorationLine::UNDERLINE),
        ),
        overline: make_deco(line.contains(style::values::specified::TextDecorationLine::OVERLINE)),
        line_through: make_deco(
            line.contains(style::values::specified::TextDecorationLine::LINE_THROUGH),
        ),
    }
}

/// Builds a [`usvg::TextSpan`] from an element's computed style + text attributes.
fn build_text_span(
    element: &ServoLayoutElement<'_>,
    computed: &ComputedValues,
    font_size: usvg::NonZeroPositiveF32,
    gradients: &Gradients,
    diagonal: f32,
) -> usvg::TextSpan {
    let visible = !matches!(
        computed.get_inherited_box().visibility,
        style::computed_values::visibility::T::Hidden |
            style::computed_values::visibility::T::Collapse
    );

    let small_caps = computed.get_font().font_variant_caps ==
        style::computed_values::font_variant_caps::T::SmallCaps;

    usvg::TextSpan {
        start: 0,
        end: 0,
        fill: build_fill(computed, gradients),
        stroke: build_stroke(computed, gradients, diagonal),
        paint_order: usvg::PaintOrder::default(),
        font: convert_font(computed),
        font_size,
        small_caps,
        apply_kerning: true,
        font_optical_sizing: usvg::FontOpticalSizing::Auto,
        decoration: text_decoration(computed, gradients, diagonal),
        dominant_baseline: dominant_baseline(computed),
        alignment_baseline: alignment_baseline(computed),
        baseline_shift: baseline_shift(computed),
        visible,
        letter_spacing: letter_spacing(computed),
        word_spacing: word_spacing(computed),
        text_length: length_attr_opt(element, "textLength").filter(|v| *v >= 0.0),
        length_adjust: length_adjust(element),
    }
}

/// Collects the [`usvg::TextChunk`]s for a `<text>` element, ported from usvg's
/// `collect_text_chunks`.
pub(crate) fn collect_text_chunks(
    element: &ServoLayoutElement<'_>,
    pos_list: &[CharacterPosition],
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    texts: &TrimmedTexts,
) -> Vec<usvg::TextChunk> {
    let mut state = IterState {
        chars_count: 0,
        chunk_bytes_count: 0,
        split_chunk: false,
        text_flow: usvg::TextFlow::Linear,
        chunks: Vec::new(),
    };
    collect_chunks_impl(
        element, pos_list, context, gradients, defs, diagonal, &mut state, texts,
    );
    state.chunks
}

fn collect_chunks_impl(
    element: &ServoLayoutElement<'_>,
    pos_list: &[CharacterPosition],
    context: &LayoutContext,
    gradients: &Gradients,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
    diagonal: f32,
    state: &mut IterState,
    texts: &TrimmedTexts,
) {
    for child in element.as_node().dom_children() {
        if let Some(child_element) = child.as_element() {
            // `<textPath>` (text-on-path) must be a direct child of `<text>`; any
            // nested `<textPath>` is ignored. Resolve its referenced path and switch
            // the current text flow, then split the chunk on either side of it.
            let is_text_path = child_element.local_name() == &LocalName::from("textPath");
            if is_text_path {
                if element_layout_type(element) != LayoutElementType::SVGTextElement {
                    state.chars_count += count_chars(child, texts);
                    continue;
                }

                match resolve_text_flow(&child_element, context, defs) {
                    Some(flow) => state.text_flow = flow,
                    None => {
                        // Skip an invalid text path and all its children. We still
                        // advance the chars count because `pos_list` was built
                        // including this subtree.
                        state.chars_count += count_chars(child, texts);
                        continue;
                    },
                }

                state.split_chunk = true;
            }

            collect_chunks_impl(
                &child_element,
                pos_list,
                context,
                gradients,
                defs,
                diagonal,
                state,
                texts,
            );

            state.text_flow = usvg::TextFlow::Linear;

            // The next character after a `textPath` must start a new chunk too.
            if is_text_path {
                state.split_chunk = true;
            }

            continue;
        }

        if !child.is_text_node() {
            continue;
        }
        let Some(text) = texts.get(&child.opaque()).cloned() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }

        let computed = element.style(&context.style_context);

        let Some(font_size) =
            usvg::NonZeroPositiveF32::new(computed.get_font().font_size.computed_size().px())
        else {
            // A zero font size makes the span invalid; skip it.
            state.chars_count += text.chars().count();
            continue;
        };

        let span = build_text_span(element, &computed, font_size, gradients, diagonal);
        let anchor = text_anchor(&computed);

        let mut is_new_span = true;
        for c in text.chars() {
            let char_len = c.len_utf8();

            // A new chunk starts on the first span, whenever a character has an
            // absolute x/y coordinate, and after a `<textPath>` boundary.
            let is_new_chunk = pos_list[state.chars_count].x.is_some() ||
                pos_list[state.chars_count].y.is_some() ||
                state.split_chunk ||
                state.chunks.is_empty();

            state.split_chunk = false;

            if is_new_chunk {
                state.chunk_bytes_count = 0;
                let mut span2 = span.clone();
                span2.start = 0;
                span2.end = char_len;
                state.chunks.push(usvg::TextChunk {
                    x: pos_list[state.chars_count].x,
                    y: pos_list[state.chars_count].y,
                    anchor,
                    spans: vec![span2],
                    text_flow: state.text_flow.clone(),
                    text: c.to_string(),
                });
            } else if is_new_span {
                let mut span2 = span.clone();
                span2.start = state.chunk_bytes_count;
                span2.end = state.chunk_bytes_count + char_len;
                if let Some(chunk) = state.chunks.last_mut() {
                    chunk.text.push(c);
                    chunk.spans.push(span2);
                }
            } else if let Some(chunk) = state.chunks.last_mut() {
                chunk.text.push(c);
                if let Some(span) = chunk.spans.last_mut() {
                    span.end += char_len;
                }
            }

            is_new_span = false;
            state.chars_count += 1;
            state.chunk_bytes_count += char_len;
        }
    }
}

/// Resolves a `<textPath>` element into a [`usvg::TextFlow::Path`], ported from
/// usvg's `resolve_text_flow`: the `href` target is converted to a path outline,
/// its own `transform` applied, and `startOffset` (a percentage relative to the
/// whole path length, or an absolute length) resolved.
pub(crate) fn resolve_text_flow(
    element: &ServoLayoutElement<'_>,
    context: &LayoutContext,
    defs: &HashMap<String, ServoLayoutElement<'_>>,
) -> Option<usvg::TextFlow> {
    let href = element
        .attribute_as_str(&ns!(), &LocalName::from("href"))
        .or_else(|| element.attribute_as_str(&ns!(xlink), &LocalName::from("href")))?;
    let linked = defs.get(href.trim_start_matches('#'))?;

    let linked_ty = element_layout_type(linked);
    let linked_computed = linked
        .style_data()
        .is_some()
        .then(|| linked.as_node().style(&context.style_context));
    let mut path = resolve_shape_path(linked, linked_ty, linked_computed.as_deref())?;

    // The referenced path's own `transform` applies to its outline.
    let transform = linked
        .attribute_as_str(&ns!(), &LocalName::from("transform"))
        .map(parse_transform)
        .unwrap_or_else(usvg::Transform::identity);
    if !transform.is_identity() {
        path = path.transform(transform)?;
    }
    let path = Arc::new(path);

    let start_offset = match element.attribute_as_str(&ns!(), &LocalName::from("startOffset")) {
        Some(value) => match value.trim().parse::<svgtypes::Length>() {
            // 'If a percentage is given, then the `startOffset` represents a
            // percentage distance along the entire path.'
            Ok(length) if length.unit == LengthUnit::Percent => {
                usvg::path_length(&path) * (length.number as f32 / 100.0)
            },
            Ok(length) => length.number as f32,
            Err(_) => 0.0,
        },
        None => 0.0,
    };

    let id = usvg::NonEmptyString::new(element_id(linked)?)?;
    Some(usvg::TextFlow::Path(Arc::new(usvg::TextPath {
        id,
        start_offset,
        path,
    })))
}
