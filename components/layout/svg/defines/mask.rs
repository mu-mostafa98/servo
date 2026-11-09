/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<mask>` definition parsing.

use html5ever::LocalName;
use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use servo_svg::document::{MaskContentUnits, MaskDef, MaskType, MaskUnits, RegionValue};
use web_atoms::ns;

use super::{DefinitionParser, build_def_content_root, collect_def_content, element_id};
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
        let id = element_id(&element)?;
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
        // The mask region attributes are `<number-or-percentage>`. A `%` suffix
        // becomes a bbox fraction; a bare number is kept as a length whose
        // meaning is decided later by `maskUnits` (a bounding-box fraction under
        // the default `objectBoundingBox`, user units otherwise).
        let get_region = |attr: &str, default: RegionValue| -> RegionValue {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .and_then(|v| {
                    let v = v.trim();
                    match v.strip_suffix('%') {
                        Some(p) => p
                            .trim()
                            .parse::<f32>()
                            .ok()
                            .map(|p| RegionValue::Percent(p / 100.0)),
                        None => v.parse::<f32>().ok().map(RegionValue::Number),
                    }
                })
                .unwrap_or(default)
        };
        let x = get_region("x", RegionValue::Percent(-0.1));
        let y = get_region("y", RegionValue::Percent(-0.1));
        let width = get_region("width", RegionValue::Percent(1.2));
        let height = get_region("height", RegionValue::Percent(1.2));
        // `maskUnits` controls how a *bare number* in the mask region
        // x/y/width/height is interpreted (user units vs bbox fraction); a `%`
        // suffix is always a bounding-box fraction regardless of this value.
        // Defaults to `objectBoundingBox`.
        let mask_units = match element
            .attribute_as_str(&ns!(), &local_name!("maskUnits"))
            .as_deref()
        {
            Some("userSpaceOnUse") => MaskUnits::UserSpaceOnUse,
            _ => MaskUnits::ObjectBoundingBox,
        };
        let children = collect_def_content(node, builder);
        if children.is_empty() {
            return None;
        }
        Some((
            id,
            MaskDef {
                root: build_def_content_root(children),
                mask_type,
                content_units,
                x,
                y,
                width,
                height,
                mask_units,
            },
        ))
    }
}
