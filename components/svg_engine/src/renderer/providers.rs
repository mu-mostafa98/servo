/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::render_tree::{ClipPathDef, FilterDef, MaskDef, PatternDef};
use crate::style::gradient::GradientDef;

pub(crate) trait PaintResourceProvider {
    fn gradient(&self, id: &str) -> Option<&GradientDef>;
    fn pattern(&self, id: &str) -> Option<&PatternDef>;
    fn has_pattern(&self, id: &str) -> bool {
        self.pattern(id).is_some()
    }
}

pub(crate) trait ClipMaskProvider {
    fn clip_path(&self, id: &str) -> Option<&ClipPathDef>;
    fn mask(&self, id: &str) -> Option<&MaskDef>;
}

pub(crate) trait FilterProvider {
    fn filter(&self, id: &str) -> Option<&FilterDef>;
}
