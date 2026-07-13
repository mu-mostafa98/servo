/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::render_tree::{
    SvgRenderNode, SvgRenderTreeVisitorMut, VisitDecision,
};

pub struct PaintServerFixupVisitor;

impl SvgRenderTreeVisitorMut for PaintServerFixupVisitor {
    fn visit_node_mut(&mut self, _node: &mut SvgRenderNode) -> VisitDecision {
        VisitDecision::Continue
    }
}
