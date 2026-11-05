/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! An unresolved CSS `clip-path` basic shape.
//!
//! When an element's `clip-path` is a CSS `<basic-shape>` (`circle()`,
//! `ellipse()`, `inset()`, `polygon()`, `path()`) rather than a `url(#…)`
//! reference, the shape is carried inline on the [`NodeStyle`] instead of as a
//! [`ClipPathDef`]. Percentages resolve against the element's fill bounding box,
//! which is only known in the render layer, so the value stays unresolved
//! (lengths as user units, percentages as 0..1 fractions) across the layout →
//! model boundary.

use malloc_size_of_derive::MallocSizeOf;

use crate::model::geometry::PathData;
use crate::model::style::FillRule;
use crate::model::units::LengthOrPercent;

/// A resolved-or-defaulted radius for a `circle()`/`ellipse()`.
///
/// The `closest-side` / `farthest-side` keywords depend on the reference box
/// dimensions, so they are kept unresolved and evaluated at render time.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum ShapeRadius {
    /// An explicit `<length-percentage>`.
    Value(LengthOrPercent),
    /// Distance to the closest side of the reference box.
    ClosestSide,
    /// Distance to the farthest side of the reference box.
    FarthestSide,
    /// Distance from the reference-box center to a corner. `closest-corner`
    /// and `farthest-corner` coincide for a shape centered on the box.
    Corner,
}

/// A CSS `clip-path` basic shape, carried inline on a [`NodeStyle`].
///
/// [`NodeStyle`]: crate::model::style::NodeStyle
#[derive(Debug, Clone, MallocSizeOf)]
pub enum BasicShapeClip {
    /// `circle(<r> at <cx> <cy>)`.
    Circle {
        cx: LengthOrPercent,
        cy: LengthOrPercent,
        r: ShapeRadius,
    },
    /// `ellipse(<rx> <ry> at <cx> <cy>)`.
    Ellipse {
        cx: LengthOrPercent,
        cy: LengthOrPercent,
        rx: ShapeRadius,
        ry: ShapeRadius,
    },
    /// `inset(<top> <right> <bottom> <left> round <rx> <ry>)`.
    ///
    /// The radius is uniform: only the common `round <rx>` / `round <rx> <ry>`
    /// forms are preserved (per-corner radii are rare in clip paths and are
    /// flattened to the top-left corner).
    Inset {
        top: LengthOrPercent,
        right: LengthOrPercent,
        bottom: LengthOrPercent,
        left: LengthOrPercent,
        rx: LengthOrPercent,
        ry: LengthOrPercent,
    },
    /// `polygon(<fill-rule>, <x> <y>, …)`.
    Polygon {
        points: Vec<BasicShapePoint>,
        fill_rule: FillRule,
    },
    /// `path(<fill-rule>, "<d>")`.
    Path {
        path: PathData,
        fill_rule: FillRule,
    },
}

/// A polygon point whose coordinates are still unresolved [`LengthOrPercent`]s.
///
/// Kept distinct from [`Point`] (which holds resolved user-space `f32`s) so a
/// percentage coordinate cannot silently slip into geometry code that expects
/// an absolute value.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub struct BasicShapePoint {
    pub x: LengthOrPercent,
    pub y: LengthOrPercent,
}
