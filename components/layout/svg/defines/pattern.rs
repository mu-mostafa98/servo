/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<pattern>` definition parsing.

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{PatternContentUnits, PatternDef, PatternUnits};
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
        let parse_attr = |attr: &str, default: f32| -> f32 {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(default)
        };
        let width = parse_attr("width", 0.0);
        let height = parse_attr("height", 0.0);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let x = parse_attr("x", 0.0);
        let y = parse_attr("y", 0.0);
        let pattern_units = element
            .attribute_as_str(&ns!(), &local_name!("patternUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(PatternUnits::UserSpaceOnUse);
        let pattern_content_units = element
            .attribute_as_str(&ns!(), &local_name!("patternContentUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(PatternContentUnits::ObjectBoundingBox),
                _ => None,
            })
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
