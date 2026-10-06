/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The SVG document: the root [`SvgTree`], its viewport, and the definitions
//! (`<defs>`) collected from the source document.
//!
//! Document Structure spec: <https://www.w3.org/TR/SVG2/struct.html>

mod defs;
mod viewport;

use std::collections::HashMap;
use std::sync::Arc;

pub use self::defs::{
    ClipPathDef, ClipPathUnits, DefRef, FeCompositeKind, FeImageKind, FilterDef, FilterPrimitive,
    GradientDef, GradientExplicit, GradientLength, GradientStop, GradientUnits, LinearGradient,
    MarkerDef, MarkerOrient, MarkerUnits, MaskContentUnits, MaskDef, MaskType, PatternContentUnits,
    PatternDef, PatternLength, PatternUnits, RadialGradient, SpreadMethod,
};
pub use self::viewport::{
    AspectAlign, AspectRatio, MeetOrSlice, SvgViewport, ViewBox, ViewportInfo,
};
use crate::model::element::{SvgNode, SvgTreeVisitor, SvgTreeVisitorMut};
use crate::model::style::paint_servers::PaintServer;

/// The SVG render tree — a tree of [`SvgNode`]s plus viewport info
/// and gradient/clip-path/pattern/mask/filter definitions collected from `<defs>`.
#[derive(Debug)]
pub struct SvgTree {
    pub root: SvgNode,
    pub viewport: ViewportInfo,
    /// Gradient definitions keyed by their `id` (without the `#` prefix).
    pub gradients: HashMap<String, Arc<GradientDef>>,
    /// Clip path definitions keyed by their `id` (without the `#` prefix).
    pub clip_paths: HashMap<String, Arc<ClipPathDef>>,
    /// Pattern definitions keyed by their `id` (without the `#` prefix).
    pub patterns: HashMap<String, Arc<PatternDef>>,
    /// Mask definitions keyed by their `id` (without the `#` prefix).
    pub masks: HashMap<String, Arc<MaskDef>>,
    /// Filter definitions keyed by their `id` (without the `#` prefix).
    pub filters: HashMap<String, Arc<FilterDef>>,
    /// Marker definitions keyed by their `id` (without the `#` prefix).
    pub markers: HashMap<String, Arc<MarkerDef>>,
}

impl SvgTree {
    /// Visit every node in the tree with a read-only visitor.
    pub fn visit(&self, visitor: &mut dyn SvgTreeVisitor) {
        self.root.accept(visitor);
    }

    /// Visit every node in the tree with a mutable visitor.
    pub fn visit_mut(&mut self, visitor: &mut dyn SvgTreeVisitorMut) {
        self.root.accept_mut(visitor);
    }

    /// A read-only [`Defs`] view over this tree's definition maps, for
    /// render-time reference resolution.
    pub fn defs(&self) -> Defs<'_> {
        Defs {
            gradients: &self.gradients,
            clip_paths: &self.clip_paths,
            patterns: &self.patterns,
            masks: &self.masks,
            filters: &self.filters,
            markers: &self.markers,
        }
    }
}

/// A read-only view over a tree's definition maps. Handed to the renderer so
/// it can resolve `url(#id)` references (paint servers and clip-path/mask/
/// filter/marker references) at render time without mutating the tree.
///
/// The tree is immutable after build; only [`Defs`]-backed lookups bind
/// references to definitions, so a cached subtree always renders against the
/// current definitions.
#[derive(Clone, Copy)]
pub struct Defs<'a> {
    pub gradients: &'a HashMap<String, Arc<GradientDef>>,
    pub clip_paths: &'a HashMap<String, Arc<ClipPathDef>>,
    pub patterns: &'a HashMap<String, Arc<PatternDef>>,
    pub masks: &'a HashMap<String, Arc<MaskDef>>,
    pub filters: &'a HashMap<String, Arc<FilterDef>>,
    pub markers: &'a HashMap<String, Arc<MarkerDef>>,
}

impl Defs<'_> {
    /// Resolve a [`PaintServer`] reference against the definition maps.
    ///
    /// A [`PaintServer::Ref`] binding to a gradient or pattern becomes a typed
    /// [`Arc`] handle; one binding to neither uses its fallback color; a broken
    /// reference with no fallback resolves to `None` (SVG 2: no paint is
    /// rendered). Every other paint server passes through unchanged.
    pub fn resolve_paint_server(&self, paint: &PaintServer) -> Option<PaintServer> {
        match paint {
            PaintServer::Ref { id, fallback } => {
                if let Some(gradient) = self.gradients.get(id.as_str()) {
                    Some(PaintServer::Gradient(Arc::clone(gradient)))
                } else if let Some(pattern) = self.patterns.get(id.as_str()) {
                    Some(PaintServer::Pattern(Arc::clone(pattern)))
                } else if let Some(color) = fallback {
                    Some(PaintServer::Solid(*color))
                } else {
                    None
                }
            },
            other => Some(other.clone()),
        }
    }
}
