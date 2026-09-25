/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<filter>` definition parsing (filter primitives).

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::document::{FeCompositeKind, FeImageKind, FilterDef, FilterPrimitive};
use web_atoms::ns;

use super::DefinitionParser;
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::primitives::paint::parse_color_rgba;

pub(crate) struct FilterParser;

impl DefinitionParser for FilterParser {
    type Definition = FilterDef;
    fn tag_names() -> &'static [&'static str] {
        &["filter"]
    }

    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        _builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let id = element
            .attribute_as_str(&ns!(), &local_name!("id"))
            .map(|s| s.to_string())?;
        let get = |attr: &str| {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .map(|s| s.to_string())
        };
        let get_float = |attr: &str, default: f32| -> f32 {
            get(attr)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(default)
        };
        let x = get_float("x", -0.1);
        let y = get_float("y", -0.1);
        let width = get_float("width", 1.2);
        let height = get_float("height", 1.2);

        let mut primitives = Vec::new();
        for prim_child in node.dom_children() {
            if let Some(prim_elem) = prim_child.as_element() {
                let pname = prim_elem.local_name().as_ref().to_owned();
                let prim_get = |attr: &str| {
                    prim_elem
                        .attribute_as_str(&ns!(), &LocalName::from(attr))
                        .map(|s| s.to_string())
                };
                let prim_get_float = |attr: &str, default: f32| -> f32 {
                    prim_get(attr)
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(default)
                };
                // Parse a `<number-optional-number>` attribute (e.g. stdDeviation
                // "8,0" or "8 6"). Returns (x, y); when only one number is given,
                // y defaults to x. Returns (default, default) when absent/unparsable.
                let prim_get_float_pair = |attr: &str, default: f32| -> (f32, f32) {
                    prim_get(attr)
                        .map(|v| {
                            let mut vals = v
                                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                                .filter_map(|s| {
                                    let t = s.trim();
                                    if t.is_empty() {
                                        None
                                    } else {
                                        t.parse::<f32>().ok()
                                    }
                                });
                            let x = vals.next().unwrap_or(default);
                            let y = vals.next().unwrap_or(x);
                            (x, y)
                        })
                        .unwrap_or((default, default))
                };
                match pname.as_str() {
                    "feGaussianBlur" => {
                        let (sdx, sdy) = prim_get_float_pair("stdDeviation", 0.0);
                        primitives.push(FilterPrimitive::GaussianBlur(sdx, sdy));
                    },
                    "feDropShadow" => {
                        let dx = prim_get_float("dx", 2.0);
                        let dy = prim_get_float("dy", 2.0);
                        let std_dev = prim_get_float("stdDeviation", 2.0);
                        let flood_color_str =
                            prim_get("flood-color").unwrap_or_else(|| "black".to_owned());
                        let (r, g, b, a) = parse_color_rgba(&flood_color_str);
                        let flood_opacity = prim_get_float("flood-opacity", 1.0);
                        primitives.push(FilterPrimitive::DropShadow(
                            dx, dy, std_dev, r, g, b, a * flood_opacity,
                        ));
                    },
                    "feColorMatrix" => {
                        let type_attr = prim_get("type").unwrap_or_else(|| "matrix".to_owned());
                        match type_attr.trim() {
                            "saturate" => {
                                let s = prim_get_float("values", 1.0);
                                primitives.push(FilterPrimitive::Saturate(s));
                            },
                            "hueRotate" => {
                                // SVG hueRotate matrix (filter-effects-1 §feColorMatrix),
                                // with the standard luma coefficients.
                                let angle = prim_get_float("values", 0.0).to_radians();
                                let (sin, cos) = angle.sin_cos();
                                let lum_r = 0.213;
                                let lum_g = 0.715;
                                let lum_b = 0.072;
                                primitives.push(FilterPrimitive::ColorMatrix([
                                    lum_r + cos * (1.0 - lum_r) - sin * lum_r,
                                    lum_g - cos * lum_g - sin * lum_g,
                                    lum_b - cos * lum_b + sin * (1.0 - lum_b),
                                    0.0,
                                    0.0,
                                    lum_r - cos * lum_r + sin * 0.143,
                                    lum_g + cos * (1.0 - lum_g) + sin * 0.140,
                                    lum_b - cos * lum_b - sin * 0.283,
                                    0.0,
                                    0.0,
                                    lum_r - cos * lum_r - sin * (1.0 - lum_r),
                                    lum_g - cos * lum_g + sin * lum_g,
                                    lum_b + cos * (1.0 - lum_b) + sin * lum_b,
                                    0.0,
                                    0.0,
                                    0.0,
                                    0.0,
                                    0.0,
                                    1.0,
                                    0.0,
                                ]));
                            },
                            "luminanceToAlpha" => {
                                primitives.push(FilterPrimitive::LuminanceToAlpha);
                            },
                            _ => {
                                // "matrix" or unknown — parse 20 values, default identity
                                let values_str = prim_get("values").unwrap_or_default();
                                let vals: Vec<f32> = values_str
                                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                                    .filter_map(|s| {
                                        let t = s.trim();
                                        if t.is_empty() {
                                            None
                                        } else {
                                            t.parse::<f32>().ok()
                                        }
                                    })
                                    .collect();
                                let mut matrix = [
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                                    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                                ];
                                for (i, v) in vals.iter().enumerate().take(20) {
                                    matrix[i] = *v;
                                }
                                primitives.push(FilterPrimitive::ColorMatrix(matrix));
                            },
                        }
                    },
                    "feOffset" => {
                        let dx = prim_get_float("dx", 0.0);
                        let dy = prim_get_float("dy", 0.0);
                        primitives.push(FilterPrimitive::Offset(dx, dy));
                    },
                    "feFlood" => {
                        let flood_color_str =
                            prim_get("flood-color").unwrap_or_else(|| "black".to_owned());
                        let (r, g, b, a) = parse_color_rgba(&flood_color_str);
                        let flood_opacity = prim_get_float("flood-opacity", 1.0);
                        primitives.push(FilterPrimitive::Flood(r, g, b, a * flood_opacity));
                    },
                    "feComposite" => {
                        let operator = prim_get("operator").unwrap_or_else(|| "over".to_owned());
                        let composite = match operator.trim() {
                            "arithmetic" => {
                                let k1 = prim_get_float("k1", 0.0);
                                let k2 = prim_get_float("k2", 0.0);
                                let k3 = prim_get_float("k3", 0.0);
                                let k4 = prim_get_float("k4", 0.0);
                                FeCompositeKind::Arithmetic { k1, k2, k3, k4 }
                            },
                            "in" => FeCompositeKind::In,
                            "out" => FeCompositeKind::Out,
                            "atop" => FeCompositeKind::Atop,
                            "xor" => FeCompositeKind::Xor,
                            "lighter" => FeCompositeKind::Lighter,
                            _ => FeCompositeKind::Over,
                        };
                        primitives.push(FilterPrimitive::Composite(composite));
                    },
                    "feTile" => {
                        primitives.push(FilterPrimitive::Tile);
                    },
                    "feImage" => {
                        // feImage can reference an element via fragment or load an external URL.
                        let href = prim_get("href")
                            .or_else(|| prim_get("xlink:href"))
                            .unwrap_or_default();
                        let img_kind = if let Some(frag) = href.strip_prefix('#') {
                            FeImageKind::FragmentRef(frag.to_owned())
                        } else if !href.is_empty() {
                            FeImageKind::ExternalUrl(href)
                        } else {
                            // No valid source, but still include the primitive
                            // so the filter isn't discarded.
                            FeImageKind::FragmentRef(String::new())
                        };
                        primitives.push(FilterPrimitive::Image(img_kind));
                    },
                    _ => {},
                }
            }
        }
        if !primitives.is_empty() {
            Some((
                id,
                FilterDef {
                    primitives,
                    x,
                    y,
                    width,
                    height,
                },
            ))
        } else {
            None
        }
    }
}
