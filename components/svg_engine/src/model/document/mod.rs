/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The SVG document: the root [`SvgTree`], its viewport, and the definitions
//! (`<defs>`) collected from the source document.

pub mod defs;
pub mod viewport;

pub use self::defs::{
    ClipPathDef, ClipPathUnits, DefRef, FeCompositeKind, FeImageKind, FilterDef, FilterPrimitive,
    MarkerDef, MarkerOrient, MarkerUnits, MaskContentUnits, MaskDef, MaskType, PatternContentUnits,
    PatternDef, PatternLength, PatternUnits,
};
pub use self::viewport::{AspectAlign, AspectRatio, MeetOrSlice, SvgViewport, ViewBox, ViewportInfo};

use std::collections::HashMap;
use std::sync::Arc;

use crate::model::element::{SvgNode, SvgTreeVisitor, SvgTreeVisitorMut};
use crate::model::style::gradient::GradientDef;

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
}
