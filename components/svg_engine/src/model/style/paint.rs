/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG fill and stroke properties — pure data types, no WebRender dependency.

use std::sync::Arc;

use svgtypes::Color as SvgColor;

use super::gradient::GradientDef;
use crate::model::document::PatternDef;
use crate::model::units::{Id, Length, Opacity};

/// A paint server reference — a solid color, a gradient, a pattern, a
/// `url(#id)` reference, or a `context-fill`/`context-stroke` keyword.
///
/// [`PaintServer::Ref`] is a transient build-time state: the layout layer emits
/// it while only the string `url(#id)` is known, then the resolve pass rewrites
/// it into a typed [`PaintServer::Gradient`]/[`PaintServer::Pattern`] `Arc`
/// handle (or its fallback color) once the definition maps are collected. No
/// `Ref` value survives past build time.
#[derive(Debug, Clone)]
pub enum PaintServer {
    /// Solid color fill/stroke.
    Solid(SvgColor),
    /// A resolved gradient definition (`url(#myGrad)`).
    Gradient(Arc<GradientDef>),
    /// A resolved pattern definition (`url(#myPattern)`).
    Pattern(Arc<PatternDef>),
    /// Transient `url(#id)` reference, with an optional fallback color used if
    /// the reference cannot be resolved. `fallback: None` means "no paint" for
    /// a broken reference (SVG 2 behavior).
    Ref { id: Id, fallback: Option<SvgColor> },
    /// `context-fill`: inherit the fill paint from the referencing element's
    /// context (used by `<marker>`/`<use>`). Renders as no paint when there is
    /// no context element providing the value.
    ContextFill,
    /// `context-stroke`: like [`PaintServer::ContextFill`], but for the stroke.
    ContextStroke,
}

/// SVG fill properties.
#[derive(Debug, Clone)]
pub struct FillParams {
    /// The paint applied to the interior — a solid color, gradient, or pattern.
    /// `None` means "no paint" (reached only transiently during building, e.g.
    /// for `transparent`/unparseable values); `fill: none` is represented by
    /// [`NodeStyle::fill`] being `None`.
    pub paint_server: Option<PaintServer>,
    pub opacity: Opacity,
    pub fill_rule: FillRule,
}

/// SVG fill rule: determines how overlapping regions are filled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// SVG stroke properties.
#[derive(Debug, Clone)]
pub struct StrokeParams {
    /// The paint applied to the stroke — a solid color, gradient, or pattern.
    /// `None` means "no paint"; see [`FillParams::paint_server`].
    pub paint_server: Option<PaintServer>,
    pub opacity: Opacity,
    pub width: Length,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Option<Vec<f32>>,
    pub dash_offset: f32,
}

/// SVG line cap style — how the ends of open paths are rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style — how corners are rendered in a polyline/polygon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineJoin {
    Miter,
    MiterClip,
    Round,
    Bevel,
    Arcs,
}
