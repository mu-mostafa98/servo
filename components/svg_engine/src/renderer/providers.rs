/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Resource provider traits for the SVG rendering pipeline.
//!
//! The [`PaintResourceProvider`] trait abstracts over where paint resources
//! (gradients, patterns) are stored — typically the [`SvgRenderTree`] itself,
//! but mock providers exist for non-geometric elements. Clip-path, mask,
//! filter, and marker references are now typed `Arc` handles resolved at
//! build time, so they no longer go through a provider trait.

use crate::render_tree::PatternDef;
use crate::style::gradient::GradientDef;

/// Provider for paint-server resources (gradients and patterns).
pub(crate) trait PaintResourceProvider {
    fn gradient(&self, id: &str) -> Option<&GradientDef>;
    fn pattern(&self, id: &str) -> Option<&PatternDef>;
    fn has_pattern(&self, id: &str) -> bool {
        self.pattern(id).is_some()
    }
}
