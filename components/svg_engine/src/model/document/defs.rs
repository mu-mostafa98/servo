/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG definitions collected from `<defs>` — clip paths, masks, filters,
//! patterns, and markers — plus the [`DefRef`] indirection used to resolve them.

use std::sync::Arc;

use crate::model::document::viewport::{AspectRatio, ViewBox};
use crate::model::element::SvgNode;
use crate::model::style::transform::TransformOp;
use crate::model::units::Id;

/// A clip path definition collected from `<clipPath>`.
#[derive(Debug)]
pub struct ClipPathDef {
    /// The clipping region, stored as a nested node subtree so that `<g>`,
    /// `<use>`, `<text>` and nested-`<defs>` children are not silently dropped.
    pub root: SvgNode,
    /// Coordinate system for the clip path.
    pub clip_path_units: ClipPathUnits,
}

/// Coordinate system for clip path contents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipPathUnits {
    /// Coordinates are relative to the object's bounding box (0..1 range).
    ObjectBoundingBox,
    /// Coordinates are in the current user coordinate system.
    UserSpaceOnUse,
}

/// Coordinate system for pattern tile sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// The `mask-type` of a `<mask>`: whether the mask value is derived from the
/// mask content's luminance or its alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    /// `mask-type="luminance"` (the SVG default): mask value = luminance of the
    /// mask content's composited color.
    Luminance,
    /// `mask-type="alpha"`: mask value = alpha channel of the mask content.
    Alpha,
}

/// Coordinate system for `<mask>` content (`maskContentUnits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A mask definition collected from `<mask>`.
#[derive(Debug)]
pub struct MaskDef {
    /// The mask content, stored as a nested node subtree (see [`ClipPathDef::root`]).
    pub root: SvgNode,
    /// Whether the mask value is luminance- or alpha-derived (`mask-type`).
    pub mask_type: MaskType,
    /// Coordinate system for the mask content (`maskContentUnits`).
    pub content_units: MaskContentUnits,
}

/// A single SVG filter primitive operation.
#[derive(Debug)]
pub enum FilterPrimitive {
    /// Gaussian blur: std_deviation_x, std_deviation_y.
    GaussianBlur(f32, f32),
    /// Drop shadow: dx, dy, std_deviation, color_r, color_g, color_b, color_a.
    DropShadow(f32, f32, f32, f32, f32, f32, f32),
    /// Full color matrix: 20 values (5 columns × 4 rows).
    ColorMatrix([f32; 20]),
    /// Saturate: single saturation value (0.0 = grayscale, 1.0 = normal, >1.0 = oversaturate).
    Saturate(f32),
    /// Luminance-to-alpha: converts luminance to alpha channel.
    LuminanceToAlpha,
    /// Offset: shifts the input by (dx, dy).
    Offset(f32, f32),
    /// Flood: fills the filter subregion with a solid color (RGBA).
    Flood(f32, f32, f32, f32),
    /// Composite: combines two inputs with an arithmetic composite (k1-k4)
    /// or a Porter-Duff operator.
    Composite(FeCompositeKind),
    /// Tile: repeats the input to fill the filter subregion.
    Tile,
    /// Image: renders an external image or referenced element as a filter input.
    Image(FeImageKind),
}

/// The kind of composite operation for `feComposite`.
#[derive(Debug)]
pub enum FeCompositeKind {
    /// Arithmetic composite: result = k1*i1*i2 + k2*i1 + k3*i2 + k4.
    Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
    /// Porter-Duff `over` operator.
    Over,
    /// Porter-Duff `in` operator.
    In,
    /// Porter-Duff `out` operator.
    Out,
    /// Porter-Duff `atop` operator.
    Atop,
    /// Porter-Duff `xor` operator.
    Xor,
    /// Lighter (additive) composite.
    Lighter,
}

/// The kind of image source for `feImage`.
#[derive(Debug)]
pub enum FeImageKind {
    /// Reference to another element via URL fragment (e.g., `#myElement`).
    FragmentRef(String),
    /// External image URL.
    ExternalUrl(String),
}

/// A filter definition collected from `<filter>`.
#[derive(Debug)]
pub struct FilterDef {
    /// Filter primitives in order (applied left-to-right).
    pub primitives: Vec<FilterPrimitive>,
    /// Filter bounds (x, y, width, height) - may be negative for drop-shadows.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Coordinate system for pattern content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternContentUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

/// A pattern definition collected from `<pattern>`.
#[derive(Debug)]
pub struct PatternDef {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
    pub pattern_units: PatternUnits,
    pub pattern_content_units: PatternContentUnits,
    /// The `patternTransform` attribute, applied to the tile coordinate system.
    pub transform: Vec<TransformOp>,
    /// Optional `viewBox` on the pattern, mapped into the tile via
    /// `preserveAspectRatio`.
    pub view_box: Option<ViewBox>,
    pub aspect_ratio: Option<AspectRatio>,
    /// The pattern tile content, stored as a nested node subtree (see
    /// [`ClipPathDef::root`]).
    pub root: SvgNode,
}

/// Coordinate system for marker sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerUnits {
    /// Marker size scales with the referencing shape's stroke width (default).
    StrokeWidth,
    /// Marker size is fixed in the current user coordinate system.
    UserSpaceOnUse,
}

/// Orientation of a marker relative to the path.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerOrient {
    /// Rotate to align with the path tangent (default).
    Auto,
    /// Like `Auto`, but the marker at the path *start* is flipped 180°.
    AutoStartReverse,
    /// Fixed rotation angle in degrees.
    Angle(f32),
}

impl Default for MarkerOrient {
    fn default() -> Self {
        MarkerOrient::Auto
    }
}

/// A marker definition collected from `<marker>`.
#[derive(Debug)]
pub struct MarkerDef {
    /// Marker content, stored as a nested node subtree (see [`ClipPathDef::root`]).
    pub root: SvgNode,
    /// Optional `viewBox` establishing the marker's coordinate system.
    pub view_box: Option<ViewBox>,
    /// Reference point (in viewBox coords) aligned with the path vertex.
    pub ref_x: f32,
    pub ref_y: f32,
    /// Rendered marker viewport width (scaled per `marker_units`).
    pub marker_width: f32,
    pub marker_height: f32,
    pub marker_units: MarkerUnits,
    pub orient: MarkerOrient,
}

/// A reference to a definition (clip-path, mask, filter, or marker) that
/// starts as a raw `#id` string during tree building and is rewritten to a
/// typed [`Arc`] handle once the definition maps are collected, during the
/// post-build resolve pass.
///
/// This mirrors the transient `PaintServer::Ref` variant: the build layer
/// emits [`DefRef::Ref`] and the resolve pass rewrites it to
/// [`DefRef::Resolved`], so render-time consumers only ever see a resolved
/// handle.
#[derive(Debug)]
pub enum DefRef<T> {
    /// Raw `#id` (without the `#` prefix), not yet resolved.
    Ref(Id),
    /// Typed definition handle, resolved after build.
    Resolved(Arc<T>),
}

// Manual `Clone` rather than a derived one: both variants clone without
// cloning `T` itself (`Arc<T>` clones the handle, `String` clones the id),
// so `DefRef<T>` is `Clone` for *any* `T` — including definitions like
// `ClipPathDef`/`MaskDef`/`FilterDef`/`MarkerDef` that embed a non-`Clone`
// `SvgNode`.
impl<T> Clone for DefRef<T> {
    fn clone(&self) -> Self {
        match self {
            DefRef::Ref(id) => DefRef::Ref(id.clone()),
            DefRef::Resolved(def) => DefRef::Resolved(Arc::clone(def)),
        }
    }
}

impl<T> DefRef<T> {
    /// The resolved definition, or `None` if this reference is still
    /// unresolved (which should not happen after the resolve pass).
    pub fn resolved(&self) -> Option<&T> {
        match self {
            DefRef::Resolved(def) => Some(def.as_ref()),
            DefRef::Ref(_) => None,
        }
    }
}
