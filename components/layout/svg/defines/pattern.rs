/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<pattern>` definition parsing.

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{PatternContentUnits, PatternDef, PatternLength, PatternUnits};
use svgtypes::Length as SvgLength;
use web_atoms::ns;

use super::{DefinitionParser, collect_def_content, def_content_root};
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::primitives::transforms::parse_transform_str;
use crate::svg::primitives::viewport::{extract_viewbox, parse_aspect_ratio};

pub(crate) struct PatternParser;

impl DefinitionParser for PatternParser {
    type Definition = PatternDef;
    fn tag_names() -> &'static [&'static str] {
        &["pattern"]
    }

    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let parse_len = |attr: &str, default: PatternLength| -> PatternLength {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .and_then(|v| parse_pattern_length(v))
                .unwrap_or(default)
        };
        let width = parse_len("width", PatternLength::Number(0.0));
        let height = parse_len("height", PatternLength::Number(0.0));
        // §13.3.1: a zero or negative tile size disables rendering of the
        // element (no paint is applied).
        if width.is_non_positive() || height.is_non_positive() {
            return None;
        }
        let x = parse_len("x", PatternLength::Number(0.0));
        let y = parse_len("y", PatternLength::Number(0.0));
        let pattern_units = element
            .attribute_as_str(&ns!(), &local_name!("patternUnits"))
            .and_then(|s| match s.trim() {
                "userSpaceOnUse" => Some(PatternUnits::UserSpaceOnUse),
                "objectBoundingBox" => Some(PatternUnits::ObjectBoundingBox),
                _ => None,
            })
            // §13.3.1: the initial value of `patternUnits` is `objectBoundingBox`.
            .unwrap_or(PatternUnits::ObjectBoundingBox);
        let pattern_content_units = element
            .attribute_as_str(&ns!(), &local_name!("patternContentUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternContentUnits::ObjectBoundingBox),
                "userSpaceOnUse" => Some(PatternContentUnits::UserSpaceOnUse),
                _ => None,
            })
            // §13.3.1: the initial value of `patternContentUnits` is `userSpaceOnUse`.
            .unwrap_or(PatternContentUnits::UserSpaceOnUse);
        let transform = element
            .attribute_as_str(&ns!(), &local_name!("patternTransform"))
            .map(|s| parse_transform_str(s))
            .unwrap_or_default();
        let view_box = element
            .attribute_as_str(&ns!(), &local_name!("viewBox"))
            .as_deref()
            .and_then(extract_viewbox);
        let aspect_ratio = element
            .attribute_as_str(&ns!(), &local_name!("preserveAspectRatio"))
            .as_deref()
            .map(parse_aspect_ratio);
        let children = collect_def_content(node, builder);
        if children.is_empty() {
            return None;
        }
        Some((
            id,
            PatternDef {
                width,
                height,
                x,
                y,
                pattern_units,
                pattern_content_units,
                transform,
                view_box,
                aspect_ratio,
                root: def_content_root(children),
            },
        ))
    }
}

/// Parse an `x`/`y`/`width`/`height` attribute as a length or percentage.
///
/// Mirrors the gradient parser's length handling: the unit is preserved (rather
/// than baked to `f32`) so the value can be resolved against the host shape's
/// bounding box (`patternUnits="objectBoundingBox"`) or user space
/// (`patternUnits="userSpaceOnUse"`) at render time.
fn parse_pattern_length(v: &str) -> Option<PatternLength> {
    let len: SvgLength = v.trim().parse().ok()?;
    if len.unit == svgtypes::LengthUnit::Percent {
        Some(PatternLength::Percentage(len.number as f32))
    } else {
        Some(PatternLength::Number(len.number as f32))
    }
}
