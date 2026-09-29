/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG definitions collected from `<defs>` — clip paths, masks, filters,
//! gradients, patterns, and markers — plus the [`DefRef`] indirection used to
//! resolve them.
//!
//! Each definition kind lives in its own submodule (`clip_path`, `mask`,
//! `filter`, `pattern`, `marker`, `gradient`); this module holds only the
//! shared [`DefRef`] indirection and re-exports every definition type so it is
//! reachable at the [`crate::model::document`] level.
//!
//! The `<defs>` container itself: <https://www.w3.org/TR/SVG2/struct.html>.
//! Contained definitions map to their own specs: clip paths and masks
//! (<https://www.w3.org/TR/css-masking-1/>), filters
//! (<https://www.w3.org/TR/filter-effects-1/>), and paint servers — gradients
//! and patterns (<https://www.w3.org/TR/SVG2/pservers.html>).

use std::sync::Arc;

use crate::model::units::Id;

mod clip_path;
mod filter;
mod gradient;
mod marker;
mod mask;
mod pattern;

pub use self::clip_path::{ClipPathDef, ClipPathUnits};
pub use self::filter::{FeCompositeKind, FeImageKind, FilterDef, FilterPrimitive};
pub use self::gradient::{
    GradientDef, GradientExplicit, GradientLength, GradientStop, GradientUnits, LinearGradient,
    RadialGradient, SpreadMethod,
};
pub use self::marker::{MarkerDef, MarkerOrient, MarkerUnits};
pub use self::mask::{MaskContentUnits, MaskDef, MaskType};
pub use self::pattern::{PatternContentUnits, PatternDef, PatternLength, PatternUnits};

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
