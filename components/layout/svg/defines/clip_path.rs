/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<clipPath>` definition parsing.

use html5ever::local_name;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::{ServoLayoutElement, ServoLayoutNode};
use servo_svg::document::{ClipPathDef, ClipPathUnits, DefRef};
use servo_svg::style::FillRule;
use servo_svg::units::Id;
use web_atoms::ns;

use super::{DefinitionParser, build_def_content_root, collect_def_content, element_id};
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::primitives::attrs::{get_attr, parse_inline_style_prop};

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
        let id = element_id(&element)?;
        let units = element
            .attribute_as_str(&ns!(), &local_name!("clipPathUnits"))
            .and_then(|s| match s.trim() {
                "objectBoundingBox" => Some(ClipPathUnits::ObjectBoundingBox),
                _ => None,
            })
            .unwrap_or(ClipPathUnits::UserSpaceOnUse);
        // A `clip-path` presentation attribute on the `<clipPath>` element clips
        // its own content (nested clip-paths). Parse `url(#id)` / `#id`.
        let clip_path = element
            .attribute_as_str(&ns!(), &local_name!("clip-path"))
            .and_then(parse_clip_path_ref)
            .map(|id| DefRef::Ref(id));
        // `clip-rule` is an inherited property and is not computed by Stylo
        // (Gecko-only), so read it raw here (inline style wins over the
        // attribute, matching `apply_clip_rule_attr`). It becomes the default
        // fill rule for shapes that do not set their own.
        let clip_rule = parse_clip_rule(&element);
        let children = collect_def_content(node, builder);
        if children.is_empty() {
            return None;
        }
        Some((
            id,
            ClipPathDef {
                root: build_def_content_root(children),
                clip_path_units: units,
                clip_path,
                clip_rule,
            },
        ))
    }
}

/// Extract the referenced `#id` from a `clip-path` value (`url(#id)` or bare
/// `#id`), returning `None` for `none` and malformed values.
fn parse_clip_path_ref(value: &str) -> Option<Id> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = if value.starts_with("url(") {
        value.strip_prefix("url(")?.strip_suffix(')')?
    } else {
        value
    };
    let id = inner.trim().trim_start_matches('#').trim();
    if id.is_empty() {
        None
    } else {
        Some(Id::new(id))
    }
}

/// Read the `clip-rule` presentation attribute (`nonzero` / `evenodd`) on the
/// `<clipPath>` element, with an inline-`style` fallback (inline wins, matching
/// the cascade). Defaults to `nonzero` when unset.
fn parse_clip_rule(element: &ServoLayoutElement) -> FillRule {
    let style_attr = get_attr(element, "style");
    let value = get_attr(element, "clip-rule").or_else(|| {
        style_attr
            .as_ref()
            .and_then(|s| parse_inline_style_prop(s, "clip-rule"))
    });
    match value.as_deref().map(str::trim) {
        Some("evenodd" | "even-odd") => FillRule::EvenOdd,
        _ => FillRule::NonZero,
    }
}
