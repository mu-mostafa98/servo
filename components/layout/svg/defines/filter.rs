/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<filter>` definition parsing (filter primitives).

use html5ever::LocalName;
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use servo_svg::document::{
    BlendMode, ColorChannel, ComponentTransfer, ComponentTransferFunc, FeCompositeKind,
    FeImageKind, FilterDef, FilterPrimitive, FilterPrimitiveNode, MorphologyOperator,
};
use web_atoms::ns;

use super::{DefinitionParser, element_id};
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::limits::MAX_STD_DEVIATION;
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
        let id = element_id(&element)?;
        let get = |attr: &str| {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .map(|s| s.to_string())
        };
        // The filter region attributes are `<number-or-percentage>`; under the
        // default `filterUnits="objectBoundingBox"` a bare number is already a
        // fraction, so a trailing `%` is divided by 100 to match (e.g.
        // `x="-10%"` and `x="-0.1"` are equivalent).
        let get_region = |attr: &str, default: f32| -> f32 {
            get(attr)
                .and_then(|v| {
                    let v = v.trim();
                    match v.strip_suffix('%') {
                        Some(p) => p.trim().parse::<f32>().ok().map(|p| p / 100.0),
                        None => v.parse::<f32>().ok(),
                    }
                })
                .unwrap_or(default)
        };
        let x = get_region("x", -0.1);
        let y = get_region("y", -0.1);
        let width = get_region("width", 1.2);
        let height = get_region("height", 1.2);

        let mut primitives: Vec<FilterPrimitiveNode> = Vec::new();
        for prim_child in node.dom_children() {
            if let Some(prim_elem) = prim_child.as_element() {
                let pname = prim_elem.local_name().to_string();
                let prim_get = |attr: &str| {
                    prim_elem
                        .attribute_as_str(&ns!(), &LocalName::from(attr))
                        .map(|s| s.to_string())
                };
                // Wrap a parsed primitive together with its `in`/`in2`/`result`
                // wiring (all optional; a missing `in` defaults to the previous
                // result at render time).
                let mut push = |op: FilterPrimitive| {
                    primitives.push(FilterPrimitiveNode {
                        op,
                        in1: prim_get("in"),
                        in2: prim_get("in2"),
                        result: prim_get("result"),
                    });
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
                        push(FilterPrimitive::GaussianBlur(
                            sdx.min(MAX_STD_DEVIATION),
                            sdy.min(MAX_STD_DEVIATION),
                        ));
                    },
                    "feDropShadow" => {
                        let dx = prim_get_float("dx", 2.0);
                        let dy = prim_get_float("dy", 2.0);
                        let std_dev = prim_get_float("stdDeviation", 2.0).min(MAX_STD_DEVIATION);
                        let flood_color_str =
                            prim_get("flood-color").unwrap_or_else(|| "black".to_owned());
                        let (r, g, b, a) = parse_color_rgba(&flood_color_str);
                        let flood_opacity = prim_get_float("flood-opacity", 1.0);
                        push(FilterPrimitive::DropShadow(
                            dx,
                            dy,
                            std_dev,
                            r,
                            g,
                            b,
                            a * flood_opacity,
                        ));
                    },
                    "feColorMatrix" => {
                        let type_attr = prim_get("type").unwrap_or_else(|| "matrix".to_owned());
                        match type_attr.trim() {
                            "saturate" => {
                                let s = prim_get_float("values", 1.0);
                                push(FilterPrimitive::Saturate(s));
                            },
                            "hueRotate" => {
                                // SVG hueRotate matrix (filter-effects-1 §feColorMatrix),
                                // with the standard luma coefficients.
                                let angle = prim_get_float("values", 0.0).to_radians();
                                let (sin, cos) = angle.sin_cos();
                                let lum_r = 0.213;
                                let lum_g = 0.715;
                                let lum_b = 0.072;
                                push(FilterPrimitive::ColorMatrix([
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
                                push(FilterPrimitive::LuminanceToAlpha);
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
                                push(FilterPrimitive::ColorMatrix(matrix));
                            },
                        }
                    },
                    "feOffset" => {
                        let dx = prim_get_float("dx", 0.0);
                        let dy = prim_get_float("dy", 0.0);
                        push(FilterPrimitive::Offset(dx, dy));
                    },
                    "feFlood" => {
                        let flood_color_str =
                            prim_get("flood-color").unwrap_or_else(|| "black".to_owned());
                        let (r, g, b, a) = parse_color_rgba(&flood_color_str);
                        let flood_opacity = prim_get_float("flood-opacity", 1.0);
                        push(FilterPrimitive::Flood(r, g, b, a * flood_opacity));
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
                        push(FilterPrimitive::Composite(composite));
                    },
                    "feBlend" => {
                        let mode = prim_get("mode").unwrap_or_else(|| "normal".to_owned());
                        let blend = match mode.trim() {
                            "multiply" => BlendMode::Multiply,
                            "screen" => BlendMode::Screen,
                            "darken" => BlendMode::Darken,
                            "lighten" => BlendMode::Lighten,
                            "color-dodge" => BlendMode::ColorDodge,
                            "color-burn" => BlendMode::ColorBurn,
                            "hard-light" => BlendMode::HardLight,
                            "soft-light" => BlendMode::SoftLight,
                            "difference" => BlendMode::Difference,
                            "exclusion" => BlendMode::Exclusion,
                            "hue" => BlendMode::Hue,
                            "saturation" => BlendMode::Saturation,
                            "color" => BlendMode::Color,
                            "luminosity" => BlendMode::Luminosity,
                            _ => BlendMode::Normal,
                        };
                        push(FilterPrimitive::Blend(blend));
                    },
                    "feMerge" => {
                        // feMerge composites its feMergeNode children in document
                        // order (source-over). WebRender has no feMerge op, so the
                        // render layer emulates it as a chain of feComposite over.
                        let mut inputs = Vec::new();
                        for merge_child in prim_child.dom_children() {
                            if let Some(mc) = merge_child.as_element() {
                                if mc.local_name().to_string() == "feMergeNode" {
                                    if let Some(in_name) = mc
                                        .attribute_as_str(&ns!(), &LocalName::from("in"))
                                        .map(|s| s.to_string())
                                    {
                                        inputs.push(in_name);
                                    }
                                }
                            }
                        }
                        push(FilterPrimitive::Merge(inputs));
                    },
                    "feComponentTransfer" => {
                        // Each feFuncR/G/B/A child carries a transfer function;
                        // a missing channel defaults to identity.
                        let parse_func = |fe: &ServoLayoutNode| -> ComponentTransferFunc {
                            let Some(e) = fe.as_element() else {
                                return ComponentTransferFunc::Identity;
                            };
                            let fget = |attr: &str| {
                                e.attribute_as_str(&ns!(), &LocalName::from(attr))
                                    .map(|s| s.to_string())
                            };
                            // A single `<number>` attribute (slope/intercept/
                            // amplitude/exponent/offset), defaulting when absent
                            // or unparsable.
                            let fget_float = |attr: &str, default: f32| -> f32 {
                                fget(attr)
                                    .and_then(|v| v.trim().parse::<f32>().ok())
                                    .unwrap_or(default)
                            };
                            // table/discrete take a value list via `tableValues`
                            // (the `values` fallback also accepted for robustness).
                            let values = || -> Vec<f32> {
                                fget("tableValues")
                                    .or_else(|| fget("values"))
                                    .unwrap_or_default()
                                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                                    .filter_map(|s| {
                                        let t = s.trim();
                                        if t.is_empty() {
                                            None
                                        } else {
                                            t.parse::<f32>().ok()
                                        }
                                    })
                                    .collect()
                            };
                            match fget("type").as_deref() {
                                Some("identity") | None => ComponentTransferFunc::Identity,
                                Some("table") => ComponentTransferFunc::Table(values()),
                                Some("discrete") => ComponentTransferFunc::Discrete(values()),
                                Some("linear") => ComponentTransferFunc::Linear {
                                    slope: fget_float("slope", 1.0),
                                    intercept: fget_float("intercept", 0.0),
                                },
                                Some("gamma") => ComponentTransferFunc::Gamma {
                                    amplitude: fget_float("amplitude", 1.0),
                                    exponent: fget_float("exponent", 1.0),
                                    offset: fget_float("offset", 0.0),
                                },
                                _ => ComponentTransferFunc::Identity,
                            }
                        };
                        let mut ct = ComponentTransfer {
                            r: ComponentTransferFunc::Identity,
                            g: ComponentTransferFunc::Identity,
                            b: ComponentTransferFunc::Identity,
                            a: ComponentTransferFunc::Identity,
                        };
                        for func_child in prim_child.dom_children() {
                            if let Some(fe) = func_child.as_element() {
                                let func = parse_func(&func_child);
                                match fe.local_name().to_string().as_str() {
                                    "feFuncR" => ct.r = func,
                                    "feFuncG" => ct.g = func,
                                    "feFuncB" => ct.b = func,
                                    "feFuncA" => ct.a = func,
                                    _ => {},
                                }
                            }
                        }
                        push(FilterPrimitive::ComponentTransfer(ct));
                    },
                    "feTile" => {
                        push(FilterPrimitive::Tile);
                    },
                    "feMorphology" => {
                        // operator="dilate"|"erode" (default "erode"), radius is
                        // a <number-optional-number> like stdDeviation.
                        let operator = match prim_get("operator").as_deref() {
                            Some("dilate") => MorphologyOperator::Dilate,
                            _ => MorphologyOperator::Erode,
                        };
                        let (rx, ry) = prim_get_float_pair("radius", 0.0);
                        push(FilterPrimitive::Morphology(operator, rx, ry));
                    },
                    "feDisplacementMap" => {
                        // scale displaces `in` by a channel of `in2`; the
                        // channel selectors default to alpha.
                        let scale = prim_get_float("scale", 0.0);
                        let channel = |attr: &str| -> ColorChannel {
                            match prim_get(attr).as_deref() {
                                Some("R") => ColorChannel::Red,
                                Some("G") => ColorChannel::Green,
                                Some("B") => ColorChannel::Blue,
                                _ => ColorChannel::Alpha,
                            }
                        };
                        push(FilterPrimitive::DisplacementMap(
                            scale,
                            channel("xChannelSelector"),
                            channel("yChannelSelector"),
                        ));
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
                        push(FilterPrimitive::Image(img_kind));
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
