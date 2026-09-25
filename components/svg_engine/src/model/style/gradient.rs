/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG gradient data types and parsing.
//!
//! These types store parsed gradient definitions collected from `<defs>`.
//! The actual rendering converts gradients into multiple `push_rect` calls
//! with interpolated colors (software gradient rendering).
//!
//! **No WebRender dependency** — pure SVG data types via `svgtypes::Color`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use svgtypes::{Color as SvgColor, Length as SvgLength};

use crate::model::error::{SvgEngineError, SvgResult};
use crate::model::tree::PatternDef;
use crate::model::style::transform_ops::{TransformOp, parse_transform_str};
use crate::model::units::Id;

/// A paint server reference — a solid color, a gradient, or a pattern.
///
/// [`PaintServer::Ref`] is a transient build-time state: the layout layer emits
/// it while only the string `url(#id)` is known, then
/// [`crate::model::tree::SvgTree::resolve_references`] rewrites it into
/// a typed [`PaintServer::Gradient`]/[`PaintServer::Pattern`] `Arc` handle once
/// the definition maps are collected. No `Ref` value survives past build time.
#[derive(Debug, Clone)]
pub enum PaintServer {
    /// Solid color fill/stroke.
    Solid(SvgColor),
    /// A resolved gradient definition (`url(#myGrad)`).
    Gradient(Arc<GradientDef>),
    /// A resolved pattern definition (`url(#myPattern)`).
    Pattern(Arc<PatternDef>),
    /// Transient id reference, resolved to `Gradient`/`Pattern` after build.
    Ref(Id),
}

/// Definitions collected from `<defs>` during render tree construction.
#[derive(Debug, Clone)]
pub enum GradientDef {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

/// How gradient coordinates are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// How the gradient is extended beyond the 0..1 stop range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMethod {
    /// Clamp to the nearest stop color (default).
    Pad,
    /// Mirror the gradient (1→0→1...).
    Reflect,
    /// Repeat the gradient (0→1, 0→1...).
    Repeat,
}

#[derive(Debug, Clone, Copy)]
pub enum GradientLength {
    Number(f32),
    Percentage(f32),
}

impl GradientLength {
    pub fn to_object_bbox(self) -> f32 {
        match self {
            GradientLength::Number(v) => v,
            GradientLength::Percentage(p) => p / 100.0,
        }
    }

    pub fn to_user_space(self, axis_len: f32) -> f32 {
        match self {
            GradientLength::Number(v) => v,
            GradientLength::Percentage(p) => p / 100.0 * axis_len,
        }
    }

    /// Whether this length is zero, regardless of unit.
    pub fn is_zero(self) -> bool {
        match self {
            GradientLength::Number(v) => v == 0.0,
            GradientLength::Percentage(p) => p == 0.0,
        }
    }
}

/// Records which `<linearGradient>`/`<radialGradient>` attributes were
/// explicitly authored on the element (vs. left unspecified).
///
/// Unspecified attributes are inherited from a `href`-referenced gradient per
/// the SVG spec. Set by [`parse_gradient_element`] and consumed by
/// [`resolve_gradient_hrefs`] so an inherited value can be told apart from a
/// locally-defaulted one (which matters for `fx`/`fy`, whose default is the
/// gradient's own `cx`/`cy` rather than a constant).
#[derive(Debug, Clone, Copy, Default)]
pub struct GradientExplicit {
    pub x1: bool,
    pub y1: bool,
    pub x2: bool,
    pub y2: bool,
    pub cx: bool,
    pub cy: bool,
    pub r: bool,
    pub fx: bool,
    pub fy: bool,
    pub fr: bool,
    pub units: bool,
    pub spread_method: bool,
    pub transform: bool,
    pub stops: bool,
}

/// SVG `<linearGradient>` element data.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub id: String,
    /// Referenced gradient id (via `href`/`xlink:href`), without the `#` prefix.
    /// Unspecified attributes (geometry, units, spread, transform, stops) are
    /// inherited from the referenced gradient during [`resolve_gradient_hrefs`].
    pub href: Option<String>,
    pub x1: GradientLength,
    pub y1: GradientLength,
    pub x2: GradientLength,
    pub y2: GradientLength,
    pub units: GradientUnits,
    pub stops: Vec<GradientStop>,
    /// Transform applied to gradient coordinates (gradientTransform attribute).
    pub transform: Vec<TransformOp>,
    /// How the gradient extends beyond its stop range.
    pub spread_method: SpreadMethod,
    /// Which attributes were explicitly specified (see [`GradientExplicit`]).
    pub explicit: GradientExplicit,
}

/// SVG `<radialGradient>` element data.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub id: String,
    /// Referenced gradient id (via `href`/`xlink:href`), without the `#` prefix.
    /// Unspecified attributes (geometry, units, spread, transform, stops) are
    /// inherited from the referenced gradient during [`resolve_gradient_hrefs`].
    pub href: Option<String>,
    pub cx: GradientLength,
    pub cy: GradientLength,
    pub r: GradientLength,
    pub fx: GradientLength,
    pub fy: GradientLength,
    /// Focal radius (`fr`): radius of the focal circle. `0` means the focal
    /// point is a point; a positive value renders a solid disk of the first
    /// stop color around the focal point.
    pub fr: GradientLength,
    pub units: GradientUnits,
    pub stops: Vec<GradientStop>,
    /// Transform applied to gradient coordinates (gradientTransform attribute).
    pub transform: Vec<TransformOp>,
    /// How the gradient extends beyond its stop range.
    pub spread_method: SpreadMethod,
    /// Which attributes were explicitly specified (see [`GradientExplicit`]).
    pub explicit: GradientExplicit,
}

/// A single `<stop>` element in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    /// Offset in the range 0.0 – 1.0.
    pub offset: f32,
    /// Color at this offset.
    pub color: SvgColor,
}

impl PaintServer {
    /// Try to parse a paint server value from an attribute string.
    /// Supports: `"red"`, `"#ff0000"`, `"url(#myGrad)"`.
    ///
    /// URL references yield a transient [`PaintServer::Ref`], which is resolved
    /// to a typed handle later (see [`PaintServer`]).
    pub fn from_attr(val: &str) -> Option<Self> {
        let val = val.trim();
        if val.starts_with("url(#") && val.ends_with(')') {
            let id = &val[5..val.len() - 1];
            if !id.is_empty() {
                return Some(PaintServer::Ref(Id::new(id)));
            }
        }
        crate::model::style::color::parse_css_color(val).map(PaintServer::Solid)
    }
}

/// Parse a `<linearGradient>` or `<radialGradient>` element from its attributes.
/// `element_name` is `"linearGradient"` or `"radialGradient"`.
/// Returns the GradientDef keyed by id.
pub fn parse_gradient_element(
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
            .and_then(|(_, v)| crate::model::style::color::parse_css_color(v))
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

/// Resolve `href` inheritance between gradients.
///
/// A gradient that references another via `href`/`xlink:href` inherits every
/// attribute it does not specify itself — `gradientUnits`, `spreadMethod`,
/// `gradientTransform`, geometry, and `<stop>` colors — following the
/// reference chain transitively and cycle-safely. This is the common pattern of
/// a radial gradient reusing a linear gradient's stops, and the general SVG
/// mechanism for sharing gradient definitions.
///
/// Each gradient carrying an `href` is replaced in place with a fully-inherited
/// copy. Gradients without an `href` are left untouched (their parse-time
/// defaults are already final).
pub fn resolve_gradient_hrefs(map: &mut HashMap<String, Arc<GradientDef>>) {
    let ids: Vec<String> = map.keys().cloned().collect();
    for id in ids {
        let has_href = match map.get(&id) {
            Some(def) => def.href().is_some(),
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
    let inherited = def
        .href()
        .and_then(|href| resolve_gradient(href, map, visiting));

    let resolved = match &def {
        GradientDef::Linear(lg) => {
            let units = if lg.explicit.units {
                lg.units
            } else {
                inherited
                    .as_ref()
                    .map(GradientDef::units)
                    .unwrap_or(GradientUnits::ObjectBoundingBox)
            };
            let spread_method = if lg.explicit.spread_method {
                lg.spread_method
            } else {
                inherited
                    .as_ref()
                    .map(GradientDef::spread_method)
                    .unwrap_or(SpreadMethod::Pad)
            };
            let transform = if lg.explicit.transform {
                lg.transform.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| d.transform().to_vec())
                    .unwrap_or_default()
            };
            let stops = if lg.explicit.stops {
                lg.stops.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| d.stops().to_vec())
                    .unwrap_or_else(default_stops)
            };
            let x1 = if lg.explicit.x1 {
                lg.x1
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::linear_x1)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            let y1 = if lg.explicit.y1 {
                lg.y1
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::linear_y1)
                    .unwrap_or(GradientLength::Number(0.0))
            };
            // `x2` defaults to 100% (objectBoundingBox) or 100 (userSpaceOnUse),
            // so its default depends on the *resolved* units.
            let x2 = if lg.explicit.x2 {
                lg.x2
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::linear_x2)
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
                    .and_then(GradientDef::linear_y2)
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
                    .map(GradientDef::units)
                    .unwrap_or(GradientUnits::ObjectBoundingBox)
            };
            let spread_method = if rg.explicit.spread_method {
                rg.spread_method
            } else {
                inherited
                    .as_ref()
                    .map(GradientDef::spread_method)
                    .unwrap_or(SpreadMethod::Pad)
            };
            let transform = if rg.explicit.transform {
                rg.transform.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| d.transform().to_vec())
                    .unwrap_or_default()
            };
            let stops = if rg.explicit.stops {
                rg.stops.clone()
            } else {
                inherited
                    .as_ref()
                    .map(|d| d.stops().to_vec())
                    .unwrap_or_else(default_stops)
            };
            let cx = if rg.explicit.cx {
                rg.cx
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::radial_cx)
                    .unwrap_or(GradientLength::Percentage(50.0))
            };
            let cy = if rg.explicit.cy {
                rg.cy
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::radial_cy)
                    .unwrap_or(GradientLength::Percentage(50.0))
            };
            let r = if rg.explicit.r {
                rg.r
            } else {
                inherited
                    .as_ref()
                    .and_then(GradientDef::radial_r)
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
                    .and_then(GradientDef::radial_fr)
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

impl GradientDef {
    fn href(&self) -> Option<&str> {
        match self {
            GradientDef::Linear(lg) => lg.href.as_deref(),
            GradientDef::Radial(rg) => rg.href.as_deref(),
        }
    }

    fn units(&self) -> GradientUnits {
        match self {
            GradientDef::Linear(lg) => lg.units,
            GradientDef::Radial(rg) => rg.units,
        }
    }

    fn spread_method(&self) -> SpreadMethod {
        match self {
            GradientDef::Linear(lg) => lg.spread_method,
            GradientDef::Radial(rg) => rg.spread_method,
        }
    }

    fn transform(&self) -> &[TransformOp] {
        match self {
            GradientDef::Linear(lg) => &lg.transform,
            GradientDef::Radial(rg) => &rg.transform,
        }
    }

    fn stops(&self) -> &[GradientStop] {
        match self {
            GradientDef::Linear(lg) => &lg.stops,
            GradientDef::Radial(rg) => &rg.stops,
        }
    }

    fn linear_x1(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.x1),
            _ => None,
        }
    }

    fn linear_y1(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.y1),
            _ => None,
        }
    }

    fn linear_x2(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.x2),
            _ => None,
        }
    }

    fn linear_y2(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.y2),
            _ => None,
        }
    }

    fn radial_cx(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.cx),
            _ => None,
        }
    }

    fn radial_cy(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.cy),
            _ => None,
        }
    }

    fn radial_r(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.r),
            _ => None,
        }
    }

    fn radial_fr(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.fr),
            _ => None,
        }
    }
}

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
