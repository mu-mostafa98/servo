/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<linearGradient>` / `<radialGradient>` definitions and their stop/unit data.

use svgtypes::Color as SvgColor;

use crate::model::transform::TransformOp;

/// Definitions collected from `<defs>` during render tree construction.
#[derive(Debug, Clone)]
pub enum GradientDef {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

impl GradientDef {
    /// The referenced gradient id (via `href`/`xlink:href`), without the `#`
    /// prefix. `None` when this gradient stands alone.
    pub fn href(&self) -> Option<&str> {
        match self {
            GradientDef::Linear(lg) => lg.href.as_deref(),
            GradientDef::Radial(rg) => rg.href.as_deref(),
        }
    }

    /// The gradient coordinate system (`gradientUnits`).
    pub fn units(&self) -> GradientUnits {
        match self {
            GradientDef::Linear(lg) => lg.units,
            GradientDef::Radial(rg) => rg.units,
        }
    }

    /// How the gradient extends beyond its stop range (`spreadMethod`).
    pub fn spread_method(&self) -> SpreadMethod {
        match self {
            GradientDef::Linear(lg) => lg.spread_method,
            GradientDef::Radial(rg) => rg.spread_method,
        }
    }

    /// The `gradientTransform` transform ops.
    pub fn transform(&self) -> &[TransformOp] {
        match self {
            GradientDef::Linear(lg) => &lg.transform,
            GradientDef::Radial(rg) => &rg.transform,
        }
    }

    /// The gradient `<stop>` colors.
    pub fn stops(&self) -> &[GradientStop] {
        match self {
            GradientDef::Linear(lg) => &lg.stops,
            GradientDef::Radial(rg) => &rg.stops,
        }
    }

    /// `x1` of a `<linearGradient>`, or `None` for a radial gradient.
    pub fn linear_x1(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.x1),
            _ => None,
        }
    }

    /// `y1` of a `<linearGradient>`, or `None` for a radial gradient.
    pub fn linear_y1(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.y1),
            _ => None,
        }
    }

    /// `x2` of a `<linearGradient>`, or `None` for a radial gradient.
    pub fn linear_x2(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.x2),
            _ => None,
        }
    }

    /// `y2` of a `<linearGradient>`, or `None` for a radial gradient.
    pub fn linear_y2(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Linear(lg) => Some(lg.y2),
            _ => None,
        }
    }

    /// `cx` of a `<radialGradient>`, or `None` for a linear gradient.
    pub fn radial_cx(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.cx),
            _ => None,
        }
    }

    /// `cy` of a `<radialGradient>`, or `None` for a linear gradient.
    pub fn radial_cy(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.cy),
            _ => None,
        }
    }

    /// `r` of a `<radialGradient>`, or `None` for a linear gradient.
    pub fn radial_r(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.r),
            _ => None,
        }
    }

    /// `fr` of a `<radialGradient>`, or `None` for a linear gradient.
    pub fn radial_fr(&self) -> Option<GradientLength> {
        match self {
            GradientDef::Radial(rg) => Some(rg.fr),
            _ => None,
        }
    }
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
/// the SVG spec. Set during gradient parsing and consumed during `href`
/// resolution so an inherited value can be told apart from a locally-defaulted
/// one (which matters for `fx`/`fy`, whose default is the gradient's own
/// `cx`/`cy` rather than a constant).
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
    /// inherited from the referenced gradient during `href` resolution.
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
    /// inherited from the referenced gradient during `href` resolution.
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
