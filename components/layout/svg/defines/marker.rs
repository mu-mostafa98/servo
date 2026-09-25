/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<marker>` definition parsing.

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{MarkerDef, MarkerOrient, MarkerUnits};
use web_atoms::ns;

use super::{DefinitionParser, collect_def_content, def_content_root};
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::primitives::viewport::extract_viewbox;

pub(crate) struct MarkerParser;

impl DefinitionParser for MarkerParser {
    type Definition = MarkerDef;
    fn tag_names() -> &'static [&'static str] {
        &["marker"]
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

        let view_box = element
            .attribute_as_str(&ns!(), &local_name!("viewBox"))
            .as_deref()
            .and_then(extract_viewbox);

        let marker_units = element
            .attribute_as_str(&ns!(), &local_name!("markerUnits"))
            .and_then(|s| match s.trim() {
                "userSpaceOnUse" => Some(MarkerUnits::UserSpaceOnUse),
                _ => None,
            })
            .unwrap_or(MarkerUnits::StrokeWidth);

        let orient = element
            .attribute_as_str(&ns!(), &local_name!("orient"))
            .map(|s| parse_orient(s.trim()))
            .unwrap_or_default();

        let children = collect_def_content(node, builder);

        Some((
            id,
            MarkerDef {
                root: def_content_root(children),
                view_box,
                ref_x: parse_attr("refX", 0.0),
                ref_y: parse_attr("refY", 0.0),
                marker_width: parse_attr("markerWidth", 3.0),
                marker_height: parse_attr("markerHeight", 3.0),
                marker_units,
                orient,
            },
        ))
    }
}

fn parse_orient(s: &str) -> MarkerOrient {
    match s {
        "auto" => MarkerOrient::Auto,
        "auto-start-reverse" => MarkerOrient::AutoStartReverse,
        _ => {
            // Extract the leading numeric part, e.g. "45" from "45deg".
            let digits: String = s
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if let Ok(deg) = digits.parse::<f32>() {
                MarkerOrient::Angle(deg)
            } else {
                MarkerOrient::Auto
            }
        },
    }
}
