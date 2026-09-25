/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<mask>` definition parsing.

use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{MaskContentUnits, MaskDef, MaskType};
use web_atoms::ns;

use super::{DefinitionParser, collect_def_content, def_content_root};
use crate::svg::builder::SvgTreeBuilder;

pub(crate) struct MaskParser;

impl DefinitionParser for MaskParser {
    type Definition = MaskDef;
    fn tag_names() -> &'static [&'static str] {
        &["mask"]
    }

    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let mask_type = element
            .attribute_as_str(&ns!(), &local_name!("mask-type"))
            .and_then(|s| match s.trim() {
                "alpha" => Some(MaskType::Alpha),
                _ => None,
            })
            .unwrap_or(MaskType::Luminance);
        let content_units = element
            .attribute_as_str(&ns!(), &local_name!("maskContentUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(MaskContentUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(MaskContentUnits::UserSpaceOnUse);
        let children = collect_def_content(node, builder);
        if children.is_empty() {
            return None;
        }
        Some((
            id,
            MaskDef {
                root: def_content_root(children),
                mask_type,
                content_units,
            },
        ))
    }
}
