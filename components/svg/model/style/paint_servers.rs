/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG paint servers — solid colors, gradients, and patterns.
//!
//! Paint Servers spec: <https://www.w3.org/TR/SVG2/pservers.html>
//!
//! Holds the paint-server half of `fill`/`stroke`: the [`PaintServer`]
//! abstraction (solid color, gradient, pattern, `url(#id)` reference,
//! `context-fill`/`context-stroke`). The gradient and pattern *definitions*
//! referenced here live in [`crate::model::document`] (`GradientDef`,
//! `PatternDef`); only the [`PaintServer`] enum lives in this module.
//!
//! The actual rendering converts gradients into multiple `push_rect` calls
//! with interpolated colors (software gradient rendering).
//!
//! **No WebRender dependency** — pure SVG data types via `svgtypes::Color`.
//! Parsing and `href` resolution live in the layout layer
//! (`components/layout/svg/defines`).

use std::sync::Arc;

use svgtypes::Color as SvgColor;

use crate::model::document::{GradientDef, PatternDef};
use crate::model::units::Id;

/// A paint server reference — a solid color, a gradient, a pattern, a
/// `url(#id)` reference, or a `context-fill`/`context-stroke` keyword.
///
/// [`PaintServer::Ref`] is resolved at render time (see
/// [`crate::model::document::Defs::resolve_paint_server`]): the layout layer
/// stores only the string `url(#id)`, and the renderer binds it to a
/// [`PaintServer::Gradient`]/[`PaintServer::Pattern`] `Arc` handle (or its
/// fallback color) against the current definition maps. Keeping the raw
/// reference in the immutable tree is what lets clean subtrees be shared
/// across incremental reflows.
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
