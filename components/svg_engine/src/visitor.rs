/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use crate::render_tree::{PatternDef, SvgRenderNode, SvgRenderTreeVisitorMut, VisitDecision};
use crate::style::gradient::PaintServer;

pub struct PaintServerFixupVisitor<'a> {
    pub pattern_ids: &'a HashMap<String, PatternDef>,
}

impl<'a> SvgRenderTreeVisitorMut for PaintServerFixupVisitor<'a> {
    fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision {
        if let Some(ref mut fill) = node.style.fill &&
            let Some(PaintServer::Gradient(ref id)) = fill.paint_server &&
            self.pattern_ids.contains_key(id)
        {
            fill.paint_server = Some(PaintServer::Pattern(id.clone()));
        }

        if let Some(ref mut stroke) = node.style.stroke &&
            let Some(PaintServer::Gradient(ref id)) = stroke.paint_server &&
            self.pattern_ids.contains_key(id)
        {
            stroke.paint_server = Some(PaintServer::Pattern(id.clone()));
        }

        VisitDecision::Continue
    }
}
