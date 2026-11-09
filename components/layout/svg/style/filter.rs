/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `filter` presentation attribute.

use script::layout_dom::ServoLayoutElement;
use servo_svg::document::DefRef;
use servo_svg::style::NodeStyle;
use servo_svg::units::Id;

use crate::svg::primitives::attrs::{extract_url_fragment, get_attr};

/// Apply the `filter` attribute to a style's filter reference.
///
/// Filter URLs are not available via Stylo computed values in Servo builds
/// (the `Filter` type uses `Impossible` for its URL parameter), so we read
/// the DOM attribute directly. The attribute is already parsed as a
/// presentation attribute by `SVGElement::synthesize_presentational_hints`
/// but cannot round-trip through Stylo's computed-value types.
pub(crate) fn apply_filter_attribute(element: &ServoLayoutElement, style: &mut NodeStyle) {
    let filter_ref = get_attr(element, "filter")
        .as_deref()
        .and_then(extract_url_fragment);
    if let Some(filter_id) = filter_ref {
        style.filter = Some(DefRef::Ref(Id::new(filter_id)));
    }
}
