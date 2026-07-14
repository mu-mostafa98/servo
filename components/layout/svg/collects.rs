/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Phase 1: viewport extraction + rect shape building only.

use layout_api::LayoutElement;
use script::layout_dom::ServoLayoutElement;
use style::values::computed::LengthPercentage;
use style::values::generics::length::GenericLengthPercentageOrAuto;
use svg_engine::render_tree::*;
use svg_engine::shapes::*;

use super::style::get_attr;

// ======================= Shared Shape Construction =======================

const SVG_DEFAULT_FONT_SIZE: f32 = 16.0;

fn lp_to_f32(lp: &LengthPercentage) -> f32 {
    lp.to_length().map(|l| l.px()).unwrap_or(0.0)
}

fn dom_length(name: &str, get: &dyn Fn(&str) -> Option<String>) -> f32 {
    use svg_engine::shapes::attr_parsers::parse_length;
    parse_length(name, get, SVG_DEFAULT_FONT_SIZE).unwrap_or(0.0)
}

/// Build a [`Shape`] from a DOM element — Phase 1: only `rect`.
pub(crate) fn build_shape_core(
    element: &ServoLayoutElement,
    tag_name: &str,
    computed: Option<&style::properties::ComputedValues>,
) -> Option<Shape> {
    let get = |name: &str| get_attr(element, name);

    match tag_name {
        "rect" => {
            let (x, y, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_x()),
                        lp_to_f32(&svg.clone_y()),
                        match svg.clone_rx() {
                            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                                Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
                            },
                            _ => None,
                        },
                        match svg.clone_ry() {
                            GenericLengthPercentageOrAuto::LengthPercentage(nn_lp) => {
                                Some(nn_lp.0.to_length().map(|l| l.px()).unwrap_or(0.0).max(0.0))
                            },
                            _ => None,
                        },
                    )
                },
                None => {
                    use svg_engine::shapes::attr_parsers::parse_length;
                    (
                        parse_length("x", &get, SVG_DEFAULT_FONT_SIZE).unwrap_or(0.0),
                        parse_length("y", &get, SVG_DEFAULT_FONT_SIZE).unwrap_or(0.0),
                        parse_length("rx", &get, SVG_DEFAULT_FONT_SIZE).ok(),
                        parse_length("ry", &get, SVG_DEFAULT_FONT_SIZE).ok(),
                    )
                },
            };
            let w = dom_length("width", &get);
            let h = dom_length("height", &get);
            if w < 0.0 || h < 0.0 {
                return None;
            }
            Some(Shape::Rect(Rectangle {
                x,
                y,
                width: w,
                height: h,
                rx,
                ry,
            }))
        },
        _ => None,
    }
}
