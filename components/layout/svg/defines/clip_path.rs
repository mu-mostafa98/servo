/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<clipPath>` definition parsing.

use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{ClipPathDef, ClipPathUnits};
use web_atoms::ns;

use super::{DefinitionParser, collect_def_content, def_content_root};
use crate::svg::builder::SvgTreeBuilder;

pub(crate) struct ClipPathParser;

impl DefinitionParser for ClipPathParser {
    type Definition = ClipPathDef;
    fn tag_names() -> &'static [&'static str] {
        &["clipPath"]
    }

    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let units = element
            .attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(ClipPathUnits::UserSpaceOnUse);
        let children = collect_def_content(node, builder);
        if children.is_empty() {
            return None;
        }
        Some((
            id,
            ClipPathDef {
                root: def_content_root(children),
                clip_path_units: units,
            },
        ))
    }
}
