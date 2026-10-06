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

use std::collections::HashMap;
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

/// A reference to a definition (clip-path, mask, filter, or marker): either a
/// raw `#id` string ([`DefRef::Ref`]) or an already-bound typed [`Arc`] handle
/// ([`DefRef::Resolved`]).
///
/// References are resolved at render time via [`DefRef::resolve`] against the
/// definition maps on [`crate::model::document::SvgTree`]. The tree is never
/// mutated after build, which is what lets clean subtrees be shared across
/// incremental reflows.
#[derive(Debug)]
pub enum DefRef<T> {
    /// Raw `#id` (without the `#` prefix), not yet resolved.
    Ref(Id),
    /// Typed definition handle, already bound (no lookup needed).
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
    /// Resolve this reference against a definition map at render time.
    ///
    /// An already-`Resolved` handle is passed through; a [`DefRef::Ref`]
    /// looks up its `id` in `map` and returns `None` when the reference is
    /// broken (SVG 2: the effect is omitted).
    pub fn resolve<'a>(&'a self, map: &'a HashMap<String, Arc<T>>) -> Option<&'a T> {
        match self {
            DefRef::Resolved(def) => Some(def.as_ref()),
            DefRef::Ref(id) => map.get(id.as_str()).map(Arc::as_ref),
        }
    }
}
