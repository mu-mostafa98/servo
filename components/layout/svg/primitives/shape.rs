/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape geometry: turns a Servo SVG shape element into a [`tiny_skia_path::Path`].
//!
//! Geometry properties that are CSS longhands (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are
//! read from `computed`; the rest (`width`/`height`/`x1`/`y1`/`x2`/`y2`/`points`/`d`)
//! fall back to attributes.

use html5ever::{LocalName, ns};
use layout_api::{LayoutElement, LayoutElementType};
use resvg::usvg::tiny_skia_path;
use script::layout_dom::ServoLayoutElement;
use style::properties::ComputedValues;

use crate::svg::primitives::attrs::{length_attr, length_attr_opt, lp_or_auto_to_f32, lp_to_f32};
use crate::svg::primitives::geometry::{parse_path_d, polygon_points, rounded_rect};

/// Builds the path geometry for a shape element.
pub(crate) fn resolve_shape_path(
    element: &ServoLayoutElement<'_>,
    ty: LayoutElementType,
    computed: Option<&ComputedValues>,
) -> Option<tiny_skia_path::Path> {
    match ty {
        LayoutElementType::SVGRectElement => {
            let (x, y, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_x()),
                        lp_to_f32(&svg.clone_y()),
                        lp_or_auto_to_f32(&svg.clone_rx()),
                        lp_or_auto_to_f32(&svg.clone_ry()),
                    )
                },
                None => (
                    length_attr(element, "x", 0.0),
                    length_attr(element, "y", 0.0),
                    length_attr_opt(element, "rx"),
                    length_attr_opt(element, "ry"),
                ),
            };
            let w = length_attr(element, "width", 0.0);
            let h = length_attr(element, "height", 0.0);
            if w <= 0.0 || h <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            let (rx, ry) = match (rx, ry) {
                (Some(rx), Some(ry)) => (rx.min(w / 2.0), ry.min(h / 2.0)),
                (Some(rx), None) => {
                    let r = rx.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, Some(ry)) => {
                    let r = ry.min(w / 2.0).min(h / 2.0);
                    (r, r)
                },
                (None, None) => (0.0, 0.0),
            };
            rounded_rect(&mut pb, x, y, w, h, rx, ry);
            pb.finish()
        },
        LayoutElementType::SVGCircleElement => {
            let (cx, cy, r) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_to_f32(&svg.clone_r().0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "r", 0.0),
                ),
            };
            if r <= 0.0 {
                return None;
            }
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.push_circle(cx, cy, r);
            pb.finish()
        },
        LayoutElementType::SVGEllipseElement => {
            let (cx, cy, rx, ry) = match computed {
                Some(cv) => {
                    let svg = cv.get_svg();
                    (
                        lp_to_f32(&svg.clone_cx()),
                        lp_to_f32(&svg.clone_cy()),
                        lp_or_auto_to_f32(&svg.clone_rx()).unwrap_or(0.0),
                        lp_or_auto_to_f32(&svg.clone_ry()).unwrap_or(0.0),
                    )
                },
                None => (
                    length_attr(element, "cx", 0.0),
                    length_attr(element, "cy", 0.0),
                    length_attr(element, "rx", 0.0),
                    length_attr(element, "ry", 0.0),
                ),
            };
            if rx <= 0.0 || ry <= 0.0 {
                return None;
            }
            let Some(oval) = tiny_skia_path::Rect::from_xywh(cx - rx, cy - ry, rx + rx, ry + ry)
            else {
                return None;
            };
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.push_oval(oval);
            pb.finish()
        },
        LayoutElementType::SVGLineElement => {
            let x1 = length_attr(element, "x1", 0.0);
            let y1 = length_attr(element, "y1", 0.0);
            let x2 = length_attr(element, "x2", 0.0);
            let y2 = length_attr(element, "y2", 0.0);
            let mut pb = tiny_skia_path::PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            pb.finish()
        },
        LayoutElementType::SVGPolylineElement => element
            .attribute_as_str(&ns!(), &LocalName::from("points"))
            .and_then(|v| polygon_points(v, false)),
        LayoutElementType::SVGPolygonElement => element
            .attribute_as_str(&ns!(), &LocalName::from("points"))
            .and_then(|v| polygon_points(v, true)),
        LayoutElementType::SVGPathElement => element
            .attribute_as_str(&ns!(), &LocalName::from("d"))
            .and_then(parse_path_d),
        _ => None,
    }
}
