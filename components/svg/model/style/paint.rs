/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG fill and stroke properties — pure data types, no WebRender dependency.
//!
//! Painting spec (filling, stroking, marker effects):
//! <https://www.w3.org/TR/SVG2/painting.html>

use super::paint_servers::PaintServer;
use crate::model::document::{DefRef, MarkerDef};
use crate::model::units::{Length, Opacity};
use malloc_size_of_derive::MallocSizeOf;

/// SVG fill properties.
#[derive(Debug, Clone, MallocSizeOf)]
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
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// SVG stroke properties.
#[derive(Debug, Clone, MallocSizeOf)]
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
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style — how corners are rendered in a polyline/polygon.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum LineJoin {
    Miter,
    /// Miter join clipped flat at `miter_limit × width` (SVG 2 `miter-clip`)
    /// instead of falling back to a bevel.
    // TODO: needs `miter-clip` parsing/computed-value support in Stylo, and a
    // true clip in the renderer (kurbo only does bevel fallback).
    MiterClip,
    Round,
    Bevel,
    /// Circular arc join (SVG 2 `arcs`).
    // TODO: needs `arcs` parsing/computed-value support in Stylo, and true arc
    // geometry in the renderer (currently approximated as a round join).
    Arcs,
}

/// Marker references attached to a shape (`marker-start`, `marker-mid`,
/// `marker-end`), each holding a [`DefRef`] to a [`MarkerDef`] — a raw `#id`
/// during tree building, a typed `Arc` handle after the resolve pass.
#[derive(Debug, Clone, Default, MallocSizeOf)]
pub struct MarkerRefs {
    pub start: Option<DefRef<MarkerDef>>,
    pub mid: Option<DefRef<MarkerDef>>,
    pub end: Option<DefRef<MarkerDef>>,
}
