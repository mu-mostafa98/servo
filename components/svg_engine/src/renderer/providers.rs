/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Resource provider traits for the SVG rendering pipeline.
//!
//! These traits abstract over where paint resources (gradients, patterns),
//! clip masks, and filters are stored — typically the [`SvgRenderTree`]
//! itself, but mock providers exist for non-geometric elements.

use crate::render_tree::{ClipPathDef, FilterDef, MarkerDef, MaskDef, PatternDef};
use crate::style::gradient::GradientDef;

/// Provider for paint-server resources (gradients and patterns).
pub(crate) trait PaintResourceProvider {
    fn gradient(&self, id: &str) -> Option<&GradientDef>;
    fn pattern(&self, id: &str) -> Option<&PatternDef>;
    fn has_pattern(&self, id: &str) -> bool {
        self.pattern(id).is_some()
    }
}

/// Provider for clip-path and mask resources.
pub(crate) trait ClipMaskProvider {
    fn clip_path(&self, id: &str) -> Option<&ClipPathDef>;
    fn mask(&self, id: &str) -> Option<&MaskDef>;
}

/// Provider for filter-effect resources.
pub(crate) trait FilterProvider {
    fn filter(&self, id: &str) -> Option<&FilterDef>;
}

/// Provider for marker definitions.
pub(crate) trait MarkerProvider {
    fn marker(&self, id: &str) -> Option<&MarkerDef>;
}
