/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `marker-start` / `marker-mid` / `marker-end` presentation attributes.

use script::layout_dom::ServoLayoutElement;
use servo_svg::document::DefRef;
use servo_svg::style::{MarkerRefs, NodeStyle};
use servo_svg::units::Id;

use crate::svg::primitives::attrs::{extract_url_fragment, get_attr, parse_inline_style_prop};

pub(crate) fn apply_marker_presentation_attrs(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let style_attr = get_attr(element, "style");
    let read_attr = |name: &str| -> Option<String> {
        get_attr(element, name).or_else(|| {
            style_attr
                .as_ref()
                .and_then(|s| parse_inline_style_prop(s, name))
        })
    };

    let start = read_attr("marker-start").as_deref().and_then(extract_url_fragment);
    let mid = read_attr("marker-mid").as_deref().and_then(extract_url_fragment);
    let end = read_attr("marker-end").as_deref().and_then(extract_url_fragment);

    if start.is_some() || mid.is_some() || end.is_some() {
        style.markers = Some(MarkerRefs {
            start: start.map(|id| DefRef::Ref(Id::new(id))),
            mid: mid.map(|id| DefRef::Ref(Id::new(id))),
            end: end.map(|id| DefRef::Ref(Id::new(id))),
        });
    }
}
