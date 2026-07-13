/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Visitor implementations for the SVG render tree.
//!
//! Provides concrete visitors that implement [`SvgRenderTreeVisitor`] and
//! [`SvgRenderTreeVisitorMut`] for common tree operations, keeping traversal
//! logic in one place rather than scattered across ad-hoc recursive functions.

use std::collections::HashMap;

use crate::render_tree::{PatternDef, SvgRenderNode, SvgRenderTreeVisitorMut, VisitDecision};
use crate::style::gradient::PaintServer;

/// Visitor that converts `PaintServer::Gradient` references to
/// `PaintServer::Pattern` when the referenced ID is actually a pattern
/// definition (not a gradient).
///
/// This is a post-processing step after the render tree is constructed,
/// because definition collection separates gradients and patterns into
/// separate maps and the distinction is only known after both are collected.
pub struct PaintServerFixupVisitor<'a> {
    /// Set of pattern IDs (keys from the collected patterns map).
    pub pattern_ids: &'a HashMap<String, PatternDef>,
}

impl<'a> SvgRenderTreeVisitorMut for PaintServerFixupVisitor<'a> {
    fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision {
        // Check fill paint server: if it references a gradient ID that
        // is actually a pattern, convert it to PaintServer::Pattern.
        if let Some(ref mut fill) = node.style.fill &&
            let Some(PaintServer::Gradient(ref id)) = fill.paint_server &&
            self.pattern_ids.contains_key(id)
        {
            fill.paint_server = Some(PaintServer::Pattern(id.clone()));
        }

        // Check stroke paint server: same conversion.
        if let Some(ref mut stroke) = node.style.stroke &&
            let Some(PaintServer::Gradient(ref id)) = stroke.paint_server &&
            self.pattern_ids.contains_key(id)
        {
            stroke.paint_server = Some(PaintServer::Pattern(id.clone()));
        }

        VisitDecision::Continue
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::render_tree::{PatternDef, SvgRenderNode};
    use crate::style::NodeStyle;
    use crate::style::gradient::PaintServer;

    fn make_node_with_fill_gradient(id: &str) -> SvgRenderNode {
        use crate::render_tree::SvgTag;
        SvgRenderNode {
            id: None,
            tag: SvgTag::Shape(crate::shapes::Shape::Rect(crate::shapes::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                rx: None,
                ry: None,
            })),
            style: NodeStyle {
                fill: Some(crate::style::FillParams {
                    color: None,
                    paint_server: Some(PaintServer::Gradient(id.to_owned())),
                    opacity: 1.0,
                    fill_rule: crate::style::FillRule::NonZero,
                }),
                ..Default::default()
            },
            children: vec![],
        }
    }

    #[test]
    fn fixup_converts_gradient_to_pattern() {
        let mut node = make_node_with_fill_gradient("myPat");
        let mut patterns = HashMap::new();
        patterns.insert(
            "myPat".to_owned(),
            PatternDef {
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
                pattern_units: crate::render_tree::PatternUnits::UserSpaceOnUse,
                pattern_content_units: crate::render_tree::PatternContentUnits::UserSpaceOnUse,
                shapes: vec![],
            },
        );

        let mut visitor = PaintServerFixupVisitor {
            pattern_ids: &patterns,
        };
        node.accept_mut(&mut visitor);

        let fill = node.style.fill.unwrap();
        assert!(matches!(fill.paint_server, Some(PaintServer::Pattern(ref id)) if id == "myPat"));
    }

    #[test]
    fn fixup_leaves_actual_gradient_alone() {
        let mut node = make_node_with_fill_gradient("realGrad");
        let patterns = HashMap::new();

        let mut visitor = PaintServerFixupVisitor {
            pattern_ids: &patterns,
        };
        node.accept_mut(&mut visitor);

        let fill = node.style.fill.unwrap();
        assert!(
            matches!(fill.paint_server, Some(PaintServer::Gradient(ref id)) if id == "realGrad")
        );
    }
}
