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

        // `refX`/`refY` accept a `<length>` (optionally `px`), a `<percentage>`,
        // or the SVG 2 keywords `left|center|right` (refX) / `top|center|bottom`
        // (refY) mapping to 0% / 50% / 100% of the viewBox extent.
        let parse_ref_coord =
            |attr: &str, extent: f32, kw_start: &str, kw_mid: &str, kw_end: &str| -> f32 {
                let Some(v) = element.attribute_as_str(&ns!(), &LocalName::from(attr)) else {
                    return 0.0;
                };
                let v = v.trim();
                if v == kw_start {
                    return 0.0;
                }
                if v == kw_mid {
                    return extent * 0.5;
                }
                if v == kw_end {
                    return extent;
                }
                if let Some(pct) = v.strip_suffix('%') {
                    if let Ok(p) = pct.trim().parse::<f32>() {
                        return extent * p / 100.0;
                    }
                }
                v.trim_end_matches("px").parse::<f32>().unwrap_or(0.0)
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

        let marker_width = parse_attr("markerWidth", 3.0);
        let marker_height = parse_attr("markerHeight", 3.0);

        // Percentages and keywords resolve against the viewBox dimensions,
        // falling back to the marker viewport when there is no viewBox.
        let (vb_w, vb_h) = view_box
            .as_ref()
            .map(|vb| (vb.width.get(), vb.height.get()))
            .unwrap_or((marker_width, marker_height));
        let ref_x = parse_ref_coord("refX", vb_w, "left", "center", "right");
        let ref_y = parse_ref_coord("refY", vb_h, "top", "center", "bottom");

        Some((
            id,
            MarkerDef {
                root: def_content_root(children),
                view_box,
                ref_x,
                ref_y,
                marker_width,
                marker_height,
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
                // SVG 2: an unparseable `orient` falls back to the initial
                // value `0`, not `auto`.
                MarkerOrient::Angle(0.0)
            }
        },
    }
}
