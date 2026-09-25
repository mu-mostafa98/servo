/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<linearGradient>` / `<radialGradient>` parsing and `href` inheritance.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html5ever::{LocalName, local_name};
use layout_api::{LayoutElement, LayoutNode};
use script::layout_dom::ServoLayoutNode;
use svg_engine::error::{SvgEngineError, SvgResult};
use svg_engine::style::gradient::{
    GradientDef, GradientExplicit, GradientLength, GradientStop, GradientUnits, LinearGradient,
    RadialGradient, SpreadMethod,
};
use svg_engine::style::transform::TransformOp;
use svgtypes::{Color as SvgColor, Length as SvgLength};
use web_atoms::ns;

use super::DefinitionParser;
use crate::svg::builder::SvgTreeBuilder;
use crate::svg::primitives::paint::parse_css_color;
use crate::svg::primitives::transforms::parse_transform_str;

pub(crate) struct GradientParser;

impl DefinitionParser for GradientParser {
    type Definition = GradientDef;
    fn tag_names() -> &'static [&'static str] {
        &["linearGradient", "radialGradient"]
    }

    fn parse<'dom, 'a>(
        node: ServoLayoutNode<'dom>,
        _builder: &SvgTreeBuilder<'dom, 'a>,
    ) -> Option<(String, Self::Definition)> {
        let element = node.as_element()?;
        let grad_name = element.local_name().as_ref().to_owned();
        if grad_name != "linearGradient" && grad_name != "radialGradient" {
            return None;
        }
        let mut stop_attrs: Vec<Vec<(String, String)>> = Vec::new();
        for stop_node in node.dom_children() {
            if let Some(stop_elem) = stop_node.as_element() {
                if stop_elem.local_name() == &local_name!("stop") {
                    let mut attrs: Vec<(String, String)> = Vec::new();
                    if let Some(offset) = stop_elem.attribute_as_str(&ns!(), &local_name!("offset"))
                    {
                        attrs.push(("offset".to_owned(), offset.to_string()));
                    }
                    if let Some(color) =
                        stop_elem.attribute_as_str(&ns!(), &local_name!("stop-color"))
                    {
                        attrs.push(("stop-color".to_owned(), color.to_string()));
                    }
                    if let Some(op) =
                        stop_elem.attribute_as_str(&ns!(), &local_name!("stop-opacity"))
                    {
                        attrs.push(("stop-opacity".to_owned(), op.to_string()));
                    }
                    if !attrs.is_empty() {
                        stop_attrs.push(attrs);
                    }
                }
            }
        }
        let grad_get = |attr: &str| {
            element
                .attribute_as_str(&ns!(), &LocalName::from(attr))
                .map(|s| s.to_string())
        };
        // `href`/`xlink:href` references another gradient whose stops are
        // inherited when this gradient has none of its own. The id is stored
        // without the `#` prefix.
        let href = element
            .attribute_as_str(&ns!(xlink), &local_name!("href"))
            .or_else(|| element.attribute_as_str(&ns!(), &local_name!("href")))
            .map(|s| s.trim().trim_start_matches('#').to_string());
        if let Ok(def) = parse_gradient_element(&grad_name, &grad_get, &stop_attrs, href) {
            match &def {
                GradientDef::Linear(lg) => return Some((lg.id.clone(), def)),
                GradientDef::Radial(rg) => return Some((rg.id.clone(), def)),
            }
        }
        None
    }
}

// ======================= Gradient Parsing =======================

/// Parse a `<linearGradient>` or `<radialGradient>` element from its attributes.
/// `element_name` is `"linearGradient"` or `"radialGradient"`.
/// Returns the GradientDef keyed by id.
pub(crate) fn parse_gradient_element(
    element_name: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
    stop_attrs: &[Vec<(String, String)>], // list of stop attributes: [(("offset","0"),("stop-color","red")), ...]
    href: Option<String>,
) -> SvgResult<GradientDef> {
    let id = get_attr("id").unwrap_or_default();
    if id.is_empty() {
        return Err(SvgEngineError::MissingAttribute(
            "id on gradient".to_owned(),
        ));
    }

    let mut stops: Vec<GradientStop> = Vec::new();
    for attrs in stop_attrs {
        let offset = attrs
            .iter()
            .find(|(k, _)| k == "offset")
            .and_then(|(_, v)| parse_offset(v));
        let mut color = attrs
            .iter()
            .find(|(k, _)| k == "stop-color")
            .and_then(|(_, v)| parse_css_color(v))
            .unwrap_or(SvgColor::new_rgb(0, 0, 0));
        if let Some(stop_opacity) = attrs
            .iter()
            .find(|(k, _)| k == "stop-opacity")
            .and_then(|(_, v)| parse_offset(v))
        {
            color.alpha = (color.alpha as f32 * stop_opacity).round() as u8;
        }
        if let Some(offset) = offset {
            stops.push(GradientStop { offset, color });
        }
    }

    stops.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // A gradient with no `<stop>` children of its own inherits stops from its
    // `href` target during `resolve_gradient_hrefs`. One with neither stops nor
    // an `href` still gets default black stops so it paints rather than being
    // skipped (matching historical behavior).
    let has_own_stops = !stops.is_empty();
    if !has_own_stops && href.is_none() {
        stops = default_stops();
    }

    let units_explicit = get_attr("gradientUnits").is_some();
    let gradient_units = get_attr("gradientUnits")
        .and_then(|val| parse_gradient_units(&val))
        .unwrap_or(GradientUnits::ObjectBoundingBox);

    let spread_explicit = get_attr("spreadMethod").is_some();
    let spread_method = get_attr("spreadMethod")
        .as_deref()
        .and_then(parse_spread_method)
        .unwrap_or(SpreadMethod::Pad);

    let transform_explicit = get_attr("gradientTransform").is_some();
    let transform = parse_transform_str(&get_attr("gradientTransform").unwrap_or_default());

    let common_explicit = GradientExplicit {
        units: units_explicit,
        spread_method: spread_explicit,
        transform: transform_explicit,
        stops: has_own_stops,
        ..Default::default()
    };

    match element_name {
        "linearGradient" => {
            let x1 = parse_length_attr("x1", get_attr);
            let y1 = parse_length_attr("y1", get_attr);
            let x2 = parse_length_attr("x2", get_attr);
            let y2 = parse_length_attr("y2", get_attr);
            Ok(GradientDef::Linear(LinearGradient {
                id,
                href: href.clone(),
                x1: x1.unwrap_or(GradientLength::Number(0.0)),
                y1: y1.unwrap_or(GradientLength::Number(0.0)),
                x2: x2.unwrap_or(match gradient_units {
                    GradientUnits::ObjectBoundingBox => GradientLength::Percentage(100.0),
                    GradientUnits::UserSpaceOnUse => GradientLength::Number(100.0),
                }),
                y2: y2.unwrap_or(GradientLength::Number(0.0)),
                units: gradient_units,
                stops,
                transform,
                spread_method,
                explicit: GradientExplicit {
                    x1: x1.is_some(),
                    y1: y1.is_some(),
                    x2: x2.is_some(),
                    y2: y2.is_some(),
                    ..common_explicit
                },
            }))
        },
        "radialGradient" => {
            let cx_opt = parse_length_attr("cx", get_attr);
            let cy_opt = parse_length_attr("cy", get_attr);
            let r_opt = parse_length_attr("r", get_attr);
            let fx_opt = parse_length_attr("fx", get_attr);
            let fy_opt = parse_length_attr("fy", get_attr);
            let fr_opt = parse_length_attr("fr", get_attr);
            let cx = cx_opt.unwrap_or(GradientLength::Percentage(50.0));
            let cy = cy_opt.unwrap_or(GradientLength::Percentage(50.0));
            Ok(GradientDef::Radial(RadialGradient {
                id,
                href,
                cx,
                cy,
                r: r_opt.unwrap_or(GradientLength::Percentage(50.0)),
                fx: fx_opt.unwrap_or(cx),
                fy: fy_opt.unwrap_or(cy),
                fr: fr_opt.unwrap_or(GradientLength::Number(0.0)),
                units: gradient_units,
                stops,
                transform,
                spread_method,
                explicit: GradientExplicit {
                    cx: cx_opt.is_some(),
                    cy: cy_opt.is_some(),
                    r: r_opt.is_some(),
                    fx: fx_opt.is_some(),
                    fy: fy_opt.is_some(),
                    fr: fr_opt.is_some(),
                    ..common_explicit
                },
            }))
        },
        _ => Err(SvgEngineError::UnsupportedFeature(format!(
            "unknown gradient: {element_name}"
        ))),
    }
}

// ======================= Gradient href Resolution =======================

/// Resolve `href` inheritance between gradients.
///
/// A gradient that references another via `href`/`xlink:href` inherits every
/// attribute it does not specify itself — `gradientUnits`, `spreadMethod`,
/// `gradientTransform`, geometry, and `<stop>` colors — following the
/// reference chain transitively and cycle-safely.
///
/// Each gradient carrying an `href` is replaced in place with a fully-inherited
/// copy. Gradients without an `href` are left untouched (their parse-time
/// defaults are already final).
pub(crate) fn resolve_gradient_hrefs(map: &mut HashMap<String, Arc<GradientDef>>) {
    let ids: Vec<String> = map.keys().cloned().collect();
    for id in ids {
        let has_href = match map.get(&id) {
            Some(def) => grad_href(def).is_some(),
            None => false,
        };
        if !has_href {
            continue;
        }
        let mut visiting = HashSet::new();
        if let Some(resolved) = resolve_gradient(&id, map, &mut visiting) {
            if let Some(def) = map.get_mut(&id) {
                *Arc::make_mut(def) = resolved;
            }
        }
    }
}

/// Recursively resolve one gradient's `href` chain into a fully-inherited,
/// concrete definition.
///
/// Returns `None` when the reference is missing or cyclic, in which case the
/// caller keeps the gradient as parsed.
fn resolve_gradient(
    id: &str,
    map: &HashMap<String, Arc<GradientDef>>,
    visiting: &mut HashSet<String>,
) -> Option<GradientDef> {
    if !visiting.insert(id.to_owned()) {
        return None; // reference cycle
    }

    let def = map.get(id)?.as_ref().clone();
    let inherited = grad_href(&def).and_then(|href| resolve_gradient(href, map, visiting));

    let resolved = match &def {
        GradientDef::Linear(lg) => {
            let units = if lg.explicit.units {
                lg.units
            } else {
                inherited
                    .as_ref()
                    .map(grad_units)
                    .unwrap_or(GradientUnits::ObjectBoundingBox)
            };
            let spread_method = if lg.explicit.spread_method {
                lg.spread_method
            } else {
                inherited
                    .as_ref()
                    .map(grad_spread)
                    .unwrap_or(SpreadMethod::Pad)
            };
            let transform = if lg.explicit.transform {
                lg.transform.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| grad_transform(d).to_vec())
                    .unwrap_or_default()
            };
            let stops = if lg.explicit.stops {
                lg.stops.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| grad_stops(d).to_vec())
                    .unwrap_or_else(default_stops)
            };
            let x1 = if lg.explicit.x1 {
                lg.x1
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_linear_x1)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            let y1 = if lg.explicit.y1 {
                lg.y1
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_linear_y1)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            // `x2` defaults to 100% (objectBoundingBox) or 100 (userSpaceOnUse),
            // so its default depends on the *resolved* units.
            let x2 = if lg.explicit.x2 {
                lg.x2
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_linear_x2)
                    .unwrap_or(match units {
                        GradientUnits::ObjectBoundingBox => GradientLength::Percentage(100.0),
                        GradientUnits::UserSpaceOnUse => GradientLength::Number(100.0),
                    })
            };
            let y2 = if lg.explicit.y2 {
                lg.y2
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_linear_y2)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            GradientDef::Linear(LinearGradient {
                id: lg.id.clone(),
                href: lg.href.clone(),
                x1,
                y1,
                x2,
                y2,
                units,
                stops,
                transform,
                spread_method,
                explicit: lg.explicit,
            })
        },
        GradientDef::Radial(rg) => {
            let units = if rg.explicit.units {
                rg.units
            } else {
                inherited
                    .as_ref()
                    .map(grad_units)
                    .unwrap_or(GradientUnits::ObjectBoundingBox)
            };
            let spread_method = if rg.explicit.spread_method {
                rg.spread_method
            } else {
                inherited
                    .as_ref()
                    .map(grad_spread)
                    .unwrap_or(SpreadMethod::Pad)
            };
            let transform = if rg.explicit.transform {
                rg.transform.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| grad_transform(d).to_vec())
                    .unwrap_or_default()
            };
            let stops = if rg.explicit.stops {
                rg.stops.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| grad_stops(d).to_vec())
                    .unwrap_or_else(default_stops)
            };
            let cx = if rg.explicit.cx {
                rg.cx
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_radial_cx)
                    .unwrap_or(GradientLength::Percentage(50.0))
            };
            let cy = if rg.explicit.cy {
                rg.cy
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_radial_cy)
                    .unwrap_or(GradientLength::Percentage(50.0))
            };
            let r = if rg.explicit.r {
                rg.r
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_radial_r)
                    .unwrap_or(GradientLength::Percentage(50.0))
            };
            // `fx`/`fy` default to the gradient's own `cx`/`cy`. Walk the chain
            // for the first *explicitly specified* value, only falling back to
            // the resolved `cx`/`cy` when no ancestor specifies one.
            let fx = if rg.explicit.fx {
                rg.fx
            } else {
                rg.href
                    .as_deref()
                    .and_then(|h| first_explicit_radial(h, map, |g| g.explicit.fx.then_some(g.fx)))
                    .unwrap_or(cx)
            };
            let fy = if rg.explicit.fy {
                rg.fy
            } else {
                rg.href
                    .as_deref()
                    .and_then(|h| first_explicit_radial(h, map, |g| g.explicit.fy.then_some(g.fy)))
                    .unwrap_or(cy)
            };
            let fr = if rg.explicit.fr {
                rg.fr
            } else {
                inherited
                    .as_ref()
                    .and_then(grad_radial_fr)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            GradientDef::Radial(RadialGradient {
                id: rg.id.clone(),
                href: rg.href.clone(),
                cx,
                cy,
                r,
                fx,
                fy,
                fr,
                units,
                stops,
                transform,
                spread_method,
                explicit: rg.explicit,
            })
        },
    };

    visiting.remove(id);
    Some(resolved)
}

/// Walk the `href` chain from `start`, returning the first explicitly-specified
/// radial geometry value (via `pick`) found on a `<radialGradient>`. Linear
/// ancestors are skipped (they carry no radial geometry). Cycle-safe.
fn first_explicit_radial(
    start: &str,
    map: &HashMap<String, Arc<GradientDef>>,
    pick: impl Fn(&RadialGradient) -> Option<GradientLength>,
) -> Option<GradientLength> {
    let mut seen = HashSet::new();
    let mut current = Some(start.to_owned());
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            return None; // cycle
        }
        match map.get(&id) {
            Some(def) => match def.as_ref() {
                GradientDef::Radial(rg) => {
                    if let Some(value) = pick(rg) {
                        return Some(value);
                    }
                    current = rg.href.clone();
                },
                GradientDef::Linear(lg) => current = lg.href.clone(),
            },
            None => return None,
        }
    }
    None
}

/// Default stops for a gradient with neither `<stop>` children nor an `href`.
fn default_stops() -> Vec<GradientStop> {
    vec![
        GradientStop {
            offset: 0.0,
            color: SvgColor::new_rgb(0, 0, 0),
        },
        GradientStop {
            offset: 1.0,
            color: SvgColor::new_rgb(0, 0, 0),
        },
    ]
}

// ======================= Gradient field accessors =======================
//
// Free-function equivalents of `GradientDef`'s former private accessor methods.
// The resolution logic lives in layout (a separate crate from `svg_engine`), so
// it reads the `pub` fields of `LinearGradient`/`RadialGradient` directly
// rather than through inherent methods.

fn grad_href(d: &GradientDef) -> Option<&str> {
    match d {
        GradientDef::Linear(lg) => lg.href.as_deref(),
        GradientDef::Radial(rg) => rg.href.as_deref(),
    }
}

fn grad_units(d: &GradientDef) -> GradientUnits {
    match d {
        GradientDef::Linear(lg) => lg.units,
        GradientDef::Radial(rg) => rg.units,
    }
}

fn grad_spread(d: &GradientDef) -> SpreadMethod {
    match d {
        GradientDef::Linear(lg) => lg.spread_method,
        GradientDef::Radial(rg) => rg.spread_method,
    }
}

fn grad_transform(d: &GradientDef) -> &[TransformOp] {
    match d {
        GradientDef::Linear(lg) => &lg.transform,
        GradientDef::Radial(rg) => &rg.transform,
    }
}

fn grad_stops(d: &GradientDef) -> &[GradientStop] {
    match d {
        GradientDef::Linear(lg) => &lg.stops,
        GradientDef::Radial(rg) => &rg.stops,
    }
}

fn grad_linear_x1(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Linear(lg) => Some(lg.x1),
        _ => None,
    }
}

fn grad_linear_y1(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Linear(lg) => Some(lg.y1),
        _ => None,
    }
}

fn grad_linear_x2(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Linear(lg) => Some(lg.x2),
        _ => None,
    }
}

fn grad_linear_y2(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Linear(lg) => Some(lg.y2),
        _ => None,
    }
}

fn grad_radial_cx(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Radial(rg) => Some(rg.cx),
        _ => None,
    }
}

fn grad_radial_cy(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Radial(rg) => Some(rg.cy),
        _ => None,
    }
}

fn grad_radial_r(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Radial(rg) => Some(rg.r),
        _ => None,
    }
}

fn grad_radial_fr(d: &GradientDef) -> Option<GradientLength> {
    match d {
        GradientDef::Radial(rg) => Some(rg.fr),
        _ => None,
    }
}

// ======================= Gradient attribute parsing =======================

/// Parse a length attribute from a gradient coordinate.
///
/// Delegates to [`svgtypes::Length`] for spec-compliant SVG length parsing
/// that handles all units (`px`, `em`, `ex`, `cm`, `mm`, `in`, `pt`, `pc`, `%`).
/// Percent values are kept explicit so they can be interpreted correctly
/// in objectBoundingBox coordinates.
fn parse_length_attr(
    attr: &str,
    get_attr: &dyn Fn(&str) -> Option<String>,
) -> Option<GradientLength> {
    let v = get_attr(attr)?;
    let len: SvgLength = v.parse().ok()?;
    if len.unit == svgtypes::LengthUnit::Percent {
        Some(GradientLength::Percentage(len.number as f32))
    } else {
        Some(GradientLength::Number(len.number as f32))
    }
}

fn parse_gradient_units(val: &str) -> Option<GradientUnits> {
    match val.trim() {
        "objectBoundingBox" => Some(GradientUnits::ObjectBoundingBox),
        "userSpaceOnUse" => Some(GradientUnits::UserSpaceOnUse),
        _ => None,
    }
}

fn parse_spread_method(val: &str) -> Option<SpreadMethod> {
    match val.trim() {
        "pad" => Some(SpreadMethod::Pad),
        "reflect" => Some(SpreadMethod::Reflect),
        "repeat" => Some(SpreadMethod::Repeat),
        _ => None,
    }
}

/// Parse a stop offset value (e.g. "0", "0.5", "50%", "100%").
fn parse_offset(val: &str) -> Option<f32> {
    let val = val.trim();
    if let Some(pct) = val.strip_suffix('%') {
        pct.trim().parse::<f32>().ok().map(|v| v / 100.0)
    } else {
        val.parse::<f32>().ok()
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::GradientParser;

    #[test]
    fn gradient_parser_collects_both_linear_and_radial_tags() {
        let tags = GradientParser::tag_names();
        assert!(tags.contains(&"linearGradient"));
        assert!(tags.contains(&"radialGradient"));
    }
}
