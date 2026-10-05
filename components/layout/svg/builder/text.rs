/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<text>`/`<tspan>` render-node assembly and font shaping for the SVG builder.

use std::collections::HashMap;

use html5ever::local_name;
use kurbo::{ParamCurve as _, ParamCurveArclen as _, ParamCurveDeriv as _};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_svg::element::text::{TextAnchor, TextSpan};
use servo_svg::element::{Container, SvgNode, SvgTag};
use web_atoms::ns;

use super::{extract_id, font_key_to_resource};
use crate::context::LayoutContext;
use crate::svg::primitives::attrs::{extract_url_fragment, get_attr, parse_length_token};
use crate::svg::primitives::text::{build_text, build_text_run, parse_length_list};
use crate::svg::style::build_style;

/// A `<textPath>`'s resolved placement: the reference path, the resolved
/// `startOffset` (distance along the path at which the text begins), and the
/// `text-anchor` that positions the text relative to that point.
struct TextPathPlacement {
    path: kurbo::BezPath,
    start_offset: f32,
    anchor: TextAnchor,
}

/// Arc-length integration accuracy for path parameterization (`kurbo` units).
const ARC_LEN_ACCURACY: f64 = 0.1;

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
///
/// A `<textPath>` child places its text along a referenced `<path>`: the run's
/// glyphs are re-positioned to points on the path (advancing by arc length) and
/// rotated to follow the path tangent. See [`place_on_path`].
pub(crate) fn build_text_node<'dom>(
    node: ServoLayoutNode<'dom>,
    context: &LayoutContext,
    css_rules: &HashMap<String, HashMap<String, String>>,
    element_ids: &HashMap<String, ServoLayoutNode<'dom>>,
) -> Option<SvgNode> {
    let element = node.as_element()?;
    let fs: f32 = 16.0;
    let get = |name: &str| get_attr(&element, name);

    // Collect the ordered inline runs of this element.
    let runs = collect_text_runs(node, fs, element_ids);

    // No runs → maybe a bare single-span (e.g. <tspan> with only text, or
    // a <text> with no element children). Fall back to the legacy single-span
    // path so existing simple <text> usage keeps working.
    if runs.is_empty() {
        let mut span = build_text(node, &get, fs)?;
        shape_text_span(&mut span, node, context);
        let (style, transforms, _color) = build_style(node, context, css_rules, None, None);
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
        let (mut span, run_node, placement) = runs.into_iter().next().unwrap();
        // Shape with the run's own node (the <tspan> for tspan runs, the
        // <text> itself for bare-text runs) so the run's font-size applies.
        shape_text_span(&mut span, run_node, context);
        if let Some(p) = &placement {
            place_on_path(&mut span, &p.path, p.start_offset, p.anchor);
        }
        let (style, transforms, _color) = build_style(node, context, css_rules, None, None);
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
        .map(|(mut span, run_node, placement)| {
            shape_text_span(&mut span, run_node, context);
            if let Some(p) = &placement {
                place_on_path(&mut span, &p.path, p.start_offset, p.anchor);
            }
            (span, run_node, placement.is_some())
        })
        .collect::<Vec<_>>();
    let total_advance: f32 = shaped
        .iter()
        .filter(|(_, _, is_text_path)| !is_text_path)
        .map(|(s, _, _)| s.total_advance())
        .sum();
    let anchor_shift = get("text-anchor")
        .as_deref()
        .map(|v| match v.trim() {
            "middle" => -0.5,
            "end" => -1.0,
            _ => 0.0,
        })
        .unwrap_or(0.0) *
        total_advance;

    let mut pen = anchor_shift;
    // `dy` shifts the *current* text position, so it accumulates across runs
    // (a later tspan's `dy` is relative to the position after earlier ones).
    let mut dy_pen = 0.0f32;
    let mut children = Vec::with_capacity(shaped.len());
    for (mut span, run_node, is_text_path) in shaped {
        if is_text_path {
            // A `<textPath>` run is already positioned on the path; it does not
            // take part in the horizontal pen / vertical dy flow.
            let (run_style, run_transforms, _color) =
                build_style(run_node, context, css_rules, None, None);
            let run_id = extract_id(&run_node.as_element()?);
            children.push(SvgNode {
                id: run_id,
                tag: SvgTag::Text(span),
                style: run_style,
                transforms: run_transforms,
                viewport: None,
                children: vec![],
            });
            continue;
        }
        span.advance_offset = pen;
        // The whole-line anchor shift is already folded into `advance_offset`,
        // so clear each run's own text-anchor to avoid double-applying it.
        span.text_anchor = TextAnchor::Start;
        // Offset this run by the accumulated vertical shift from preceding runs.
        if span.y.is_empty() {
            // No explicit y — make the accumulated dy the run's origin so the
            // vertical shift is not lost (matches the old single-value default).
            span.y.push(dy_pen);
        } else {
            for yv in &mut span.y {
                *yv += dy_pen;
            }
        }
        dy_pen += span.dy.iter().sum::<f32>();
        pen += span.total_advance();
        let (run_style, run_transforms, _color) =
            build_style(run_node, context, css_rules, None, None);
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

    let (style, transforms, _color) = build_style(node, context, css_rules, None, None);
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

/// An ordered inline run within a `<text>`: the span data, the DOM node it
/// inherits style/font from (the `<tspan>` for tspan runs, the `<text>` itself
/// for bare-text runs), and — for `<textPath>` runs — the resolved path
/// placement. The `ServoLayoutNode` lifetime is elided to match the enclosing
/// function signatures.
type RunWithNode<'dom> = (TextSpan, ServoLayoutNode<'dom>, Option<TextPathPlacement>);

/// Collect the ordered inline runs of a `<text>` (or `<tspan>`) element.
///
/// Each bare text node becomes a run that inherits the parent element's
/// attributes; each `<tspan>` child becomes a run carrying its own attributes
/// (`fill`, `font-size`, `x`/`y`, `dx`/`dy`, `text-anchor`). Pure-whitespace
/// text between tspans (indentation/newlines) is dropped so it does not render
/// as missing-glyph boxes.
fn collect_text_runs<'dom>(
    node: ServoLayoutNode<'dom>,
    fs: f32,
    element_ids: &HashMap<String, ServoLayoutNode<'dom>>,
) -> Vec<RunWithNode<'dom>> {
    let parent_elem = node.as_element().unwrap();
    // The <text>'s x/y is the line origin. Every run inherits it as the base
    // position; a <tspan> may override x/y explicitly. Horizontal flow between
    // runs is handled separately by advance_offset (cumulative advance +
    // anchor shift), so all runs share the same x/y base.
    let parent_x = parse_length_list("x", &|n: &str| get_attr(&parent_elem, n), fs);
    let parent_y = parse_length_list("y", &|n: &str| get_attr(&parent_elem, n), fs);
    let children: Vec<_> = node.dom_children().collect();
    let mut runs = Vec::new();
    for (i, child) in children.iter().enumerate() {
        if let Some(child_elem) = child.as_element() {
            if child_elem.local_name() == &local_name!("tspan") {
                let get = |n: &str| get_attr(&child_elem, n);
                if let Some(mut span) = build_text(*child, &get, fs) {
                    // Inherit the <text>'s baseline/origin for any axis the
                    // <tspan> does not set explicitly.
                    if get_attr(&child_elem, "x").is_none() {
                        span.x = parent_x.clone();
                    }
                    if get_attr(&child_elem, "y").is_none() {
                        span.y = parent_y.clone();
                    }
                    runs.push((span, *child, None));
                }
            } else if child_elem.local_name() == &local_name!("textPath") {
                // A `<textPath>` places its own text along a referenced path.
                // When the reference cannot be resolved (no href, non-`<path>`
                // target, unparseable `d`), the placement is `None` and the run
                // falls back to a normal horizontal run.
                let get = |n: &str| get_attr(&child_elem, n);
                if let Some(mut span) = build_text(*child, &get, fs) {
                    // The text is written on its own line with surrounding
                    // indentation/newlines; trim leading/trailing whitespace so
                    // it neither advances the pen nor shapes into `.notdef`
                    // boxes. textPath ignores x/y/dx/dy/rotate (positions come
                    // from the path), so drop the per-character lists to stay
                    // consistent with the trimmed text.
                    let trimmed = span.text.trim().to_owned();
                    if trimmed.is_empty() {
                        continue;
                    }
                    span.text = trimmed;
                    span.x.clear();
                    span.y.clear();
                    span.dx.clear();
                    span.dy.clear();
                    span.rotate.clear();
                    // `text-anchor`: the textPath's own value, else the one it
                    // inherits from the containing `<text>`.
                    let anchor = parse_text_anchor(
                        get_attr(&child_elem, "text-anchor")
                            .or_else(|| get_attr(&parent_elem, "text-anchor")),
                    );
                    let placement =
                        resolve_text_path_placement(&child_elem, fs, element_ids, anchor);
                    runs.push((span, *child, placement));
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
                Some(e) => {
                    e.local_name() == &local_name!("tspan") ||
                        e.local_name() == &local_name!("textPath")
                },
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
                runs.push((span, node, None));
            }
        }
    }
    runs
}

/// Resolve a `<textPath>`'s referenced `<path>` into a placement. Returns
/// `None` when the element has no resolvable `href`, the reference does not
/// point at a `<path>`, or the `d` attribute is unparseable — in which case the
/// caller falls back to a normal horizontal run.
fn resolve_text_path_placement(
    elem: &ServoLayoutElement,
    fs: f32,
    element_ids: &HashMap<String, ServoLayoutNode>,
    anchor: TextAnchor,
) -> Option<TextPathPlacement> {
    let href = elem
        .attribute_as_str(&ns!(), &local_name!("href"))
        .map(str::to_string)
        .or_else(|| {
            elem.attribute_as_str(&ns!(xlink), &local_name!("href"))
                .map(str::to_string)
        })?;
    let id = extract_url_fragment(&href)?;
    let target = *element_ids.get(&id)?;
    let target_elem = target.as_element()?;
    if target_elem.local_name() != &local_name!("path") {
        return None;
    }
    let d = get_attr(&target_elem, "d")?;
    let path = kurbo::BezPath::from_svg(&d).ok()?;
    // `startOffset` percentages resolve against the path's total arc length
    // (not the font size, which is what plain lengths resolve against).
    let path_length = bez_path_length(&path) as f32;
    let start_offset = elem
        .attribute_as_str(&ns!(), &local_name!("startOffset"))
        .map(|v| parse_start_offset(v.trim(), fs, path_length))
        .unwrap_or(0.0);
    Some(TextPathPlacement {
        path,
        start_offset,
        anchor,
    })
}

/// Parse a `startOffset` value: percentages resolve against `path_length`,
/// everything else (px, em, …) against the font size like any SVG length.
fn parse_start_offset(value: &str, font_size: f32, path_length: f32) -> f32 {
    if let Some(pct) = value.strip_suffix('%') {
        if let Ok(p) = pct.trim().parse::<f32>() {
            return p / 100.0 * path_length;
        }
    }
    parse_length_token(value, font_size).unwrap_or(0.0)
}

/// Total arc length of a [`kurbo::BezPath`], summed over its segments.
fn bez_path_length(path: &kurbo::BezPath) -> f64 {
    path.segments()
        .map(|seg| seg.arclen(ARC_LEN_ACCURACY))
        .sum()
}

/// Parse a `text-anchor` attribute value into a [`TextAnchor`].
fn parse_text_anchor(value: Option<String>) -> TextAnchor {
    match value.as_deref().map(str::trim) {
        Some("middle") => TextAnchor::Middle,
        Some("end") => TextAnchor::End,
        _ => TextAnchor::Start,
    }
}

/// Re-position a shaped text run's glyphs along a `<textPath>` reference path.
///
/// Each glyph advances along the path by its shaped advance width; its position
/// becomes the point on the path at the running arc length (starting at
/// `start_offset`, shifted by `anchor`) and its rotation follows the path
/// tangent. The horizontal origin is cleared so the renderer uses the absolute
/// path coordinates, and the `rotate` list is rewritten to the tangent angles so
/// the existing per-glyph rotation path in the renderer is driven from the path
/// geometry.
fn place_on_path(
    span: &mut TextSpan,
    path: &kurbo::BezPath,
    start_offset: f32,
    anchor: TextAnchor,
) {
    let segments: Vec<kurbo::PathSeg> = path.segments().collect();
    if segments.is_empty() || span.glyphs.is_empty() {
        return;
    }
    // `text-anchor` shifts the text's *starting* point along the path so that
    // `start`/`middle`/`end` align the beginning/middle/end of the text to
    // `start_offset` (§11.6.1.1, applied to textPath).
    let anchor_shift = anchor.alignment_offset() * span.total_advance();
    let mut pen = start_offset as f64 + anchor_shift as f64;
    let mut angles = Vec::with_capacity(span.glyphs.len());
    for glyph in &mut span.glyphs {
        let (x, y, angle) = point_tangent_at(&segments, pen);
        glyph.x = x as f32;
        glyph.y = y as f32;
        angles.push(angle as f32);
        pen += glyph.advance as f64;
    }
    span.x.clear();
    span.y.clear();
    span.rotate = angles;
    span.advance_offset = 0.0;
    span.text_anchor = TextAnchor::Start;
    // The glyphs are already in visual order; disable RTL anchor mirroring so
    // `anchor_offset` stays 0 and the path coordinates are used verbatim.
    span.rtl = false;
}

/// Sample a point and tangent angle (degrees, y-down clockwise) at arc length
/// `s` along a set of path segments, clamping `s` into `[0, total]`.
fn point_tangent_at(segments: &[kurbo::PathSeg], s: f64) -> (f64, f64, f64) {
    if segments.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut remaining = s.max(0.0);
    for (i, seg) in segments.iter().enumerate() {
        let len = seg.arclen(ARC_LEN_ACCURACY);
        let is_last = i == segments.len() - 1;
        if remaining <= len || is_last {
            let t = if len > 0.0 {
                seg.inv_arclen(remaining.min(len), ARC_LEN_ACCURACY)
            } else {
                0.0
            };
            let point = seg.eval(t);
            let (dx, dy) = tangent_at(seg, t);
            let angle = if dx * dx + dy * dy < 1e-12 {
                0.0
            } else {
                dy.atan2(dx).to_degrees()
            };
            return (point.x, point.y, angle);
        }
        remaining -= len;
    }
    // Unreachable (the last segment always matches above), but keep a fallback.
    (0.0, 0.0, 0.0)
}

/// The derivative (tangent) vector of a path segment at parameter `t`.
fn tangent_at(seg: &kurbo::PathSeg, t: f64) -> (f64, f64) {
    let d = match seg {
        kurbo::PathSeg::Line(line) => line.deriv().eval(t),
        kurbo::PathSeg::Quad(quad) => quad.deriv().eval(t),
        kurbo::PathSeg::Cubic(cubic) => cubic.deriv().eval(t),
    };
    (d.x, d.y)
}

/// Shape a [`TextSpan`]'s text using the font subsystem (HarfBuzz), so cursive
/// scripts like Arabic get proper contextual joining. Text is grouped into runs
/// of consecutive characters that use the same fallback font, and each run is
/// shaped as a whole.
fn shape_text_span(span: &mut TextSpan, node: ServoLayoutNode, context: &LayoutContext) {
    use app_units::Au;
    use fonts::{ShapingFlags, ShapingOptions};
    use layout_api::LayoutNode;
    use servo_svg::element::text::{DominantBaseline, LengthAdjust, ShapedGlyph};
    use style::computed_values::font_variant_position::T as FontVariantPosition;
    use style::values::computed::{
        FontFeatureSettings, FontVariantEastAsian, FontVariantLigatures, FontVariantNumeric,
    };
    use unicode_script::Script;

    if span.text.is_empty() {
        return;
    }

    // Build a font group from the element's computed style, and capture the
    // resolved font size (needed for the dominant-baseline offset below).
    let Some((font_group, font_size, letter_spacing, word_spacing)) = (|| {
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
        // Resolve `letter-spacing` / `word-spacing` to device pixels so HarfBuzz
        // shaping applies them per glyph. `em`/`%` units resolve against the
        // font size, mirroring `text_run.rs`.
        let font_size_au: Au = font_style.font_size.computed_size().into();
        let inherited_text = computed.get_inherited_text().clone();
        let letter_spacing = inherited_text.letter_spacing.0.to_used_value(font_size_au);
        let word_spacing = inherited_text.word_spacing.to_used_value(font_size_au);
        Some((
            context.font_context.font_group(font_style),
            font_size,
            letter_spacing,
            word_spacing,
        ))
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
        DominantBaseline::Auto | DominantBaseline::Alphabetic => 0.0,
        // Top of the em box (`text-before-edge` / `hanging`).
        DominantBaseline::TextBeforeEdge | DominantBaseline::Hanging => 0.8 * font_size,
        // Bottom of the em box (`text-after-edge` / `ideographic`).
        DominantBaseline::TextAfterEdge | DominantBaseline::Ideographic => -0.2 * font_size,
        // `middle` = alphabetic + x-height/2; `central`/`mathematical` = center
        // of the em box (= (ascent - descent) / 2), a little lower than `middle`.
        DominantBaseline::Middle => 0.35 * font_size,
        DominantBaseline::Central | DominantBaseline::Mathematical => 0.45 * font_size,
    };

    let language: icu_locale_core::subtags::Language = "und".parse().unwrap();
    let mut glyphs = Vec::with_capacity(span.text.len());
    // The current text position, tracked in *absolute* coordinates: it starts
    // at the span origin (`x[0]`/`y[0]`) and each per-character `x[i]`/`y[i]`
    // resets it (§11.5.2). Glyphs are stored relative to the origin so the
    // renderer can offset them by `origin_x`/`origin_y` once.
    let origin_x = span.origin_x();
    let origin_y = span.origin_y();
    let mut cur_x = origin_x;
    let mut cur_y = origin_y;
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
            if ci < span.x.len() {
                cur_x = span.x[ci];
            }
            if ci < span.y.len() {
                cur_y = span.y[ci];
            }
            cur_x += span.dx.get(ci).copied().unwrap_or(0.0);
            cur_y += span.dy.get(ci).copied().unwrap_or(0.0);
            let advance = if ch.is_whitespace() { 4.0f32 } else { 8.0f32 };
            if !ch.is_whitespace() {
                glyphs.push(ShapedGlyph {
                    x: cur_x - origin_x,
                    y: cur_y - origin_y + baseline_shift,
                    advance,
                    glyph_id: 0,
                    character: ch,
                    font_instance_key: None,
                });
            }
            cur_x += advance;
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
            letter_spacing,
            word_spacing,
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
            if char_idx < span.x.len() {
                cur_x = span.x[char_idx];
            }
            if char_idx < span.y.len() {
                cur_y = span.y[char_idx];
            }
            cur_x += span.dx.get(char_idx).copied().unwrap_or(0.0);
            cur_y += span.dy.get(char_idx).copied().unwrap_or(0.0);
            let advance = glyph_info.advance().to_f32_px();

            glyphs.push(ShapedGlyph {
                x: cur_x - origin_x,
                y: cur_y - origin_y + baseline_shift,
                advance,
                glyph_id: glyph_info.id() as u32,
                character: chars[char_idx],
                font_instance_key: Some(key),
            });
            cur_x += advance;
            run_char_index += usize::from(glyph_info.character_count()).max(1);
        }

        ci = cj;
    }

    // Apply `textLength` / `lengthAdjust` (§11.6): scale the glyph run so its
    // total advance equals the requested length. `spacing` stretches only the
    // inter-glyph space; `spacingAndGlyphs` additionally stretches the glyph
    // outlines (via `glyph_hscale`, handled by the renderer).
    if let Some(target) = span.text_length {
        let natural = glyphs.last().map(|g| g.x + g.advance).unwrap_or(0.0);
        if natural > 0.0 && (target - natural).abs() > 0.001 {
            let factor = target / natural;
            for g in &mut glyphs {
                g.x *= factor;
                g.advance *= factor;
            }
            if span.length_adjust == LengthAdjust::SpacingAndGlyphs {
                span.glyph_hscale = factor;
            }
        }
    }

    span.glyphs = glyphs;
    span.font_instance_key = font_instance_key;
}
