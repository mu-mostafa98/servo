/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use svgtypes::Color as SvgColor;

pub fn parse_css_color(val: &str) -> Option<SvgColor> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("none") || val.eq_ignore_ascii_case("transparent") {
        return None;
    }
    val.parse().ok()
}
