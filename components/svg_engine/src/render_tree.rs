/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use crate::shapes::Shape;
use crate::style::NodeStyle;
use crate::style::gradient::GradientDef;

/// The SVG render tree — a tree of [`SvgRenderNode`]s plus viewport info
/// and gradient definitions collected from `<defs>`.
#[derive(Debug)]
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
    /// Gradient definitions keyed by their `id` (without the `#` prefix).
    pub gradients: HashMap<String, GradientDef>,
}

#[derive(Debug)]
pub struct SvgRenderNode {
    pub id: Option<String>,
    pub tag: SvgTag,
    pub style: NodeStyle,
    pub children: Vec<SvgRenderNode>,
}

#[derive(Debug)]
pub enum SvgTag {
    Shape(Shape),
    Container(Container),
}

#[derive(Debug)]
pub enum Container {
    Group,
    Svg,
    /// `<defs>` — definitions container whose children are not rendered directly.
    Defs,
    /// `<use>` — references another element by its `#id`.
    Use,
    /// `<symbol>` — a re-usable viewBox'd container referenced by `<use>`.
    Symbol,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub min_x: f32,
    pub min_y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: f32,
    pub height: f32,
    pub view_box: Option<ViewBox>,
}

// ======================= ViewBox Parsing =======================

/// Parse the `viewBox` attribute value into a [`ViewBox`].
/// Expected format: `"0 0 200 200"` or `"0,0 200,200"`.
pub fn extract_viewbox(value: &str) -> Option<ViewBox> {
    let parts: Vec<f32> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect();
    if parts.len() == 4 && parts[2] > 0.0 && parts[3] > 0.0 {
        Some(ViewBox {
            min_x: parts[0],
            min_y: parts[1],
            width: parts[2],
            height: parts[3],
        })
    } else {
        None
    }
}

// ======================= Tests =======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_valid() {
        let vb = extract_viewbox("0 0 200 200").unwrap();
        assert_eq!(vb.min_x, 0.0);
        assert_eq!(vb.min_y, 0.0);
        assert_eq!(vb.width, 200.0);
        assert_eq!(vb.height, 200.0);
    }

    #[test]
    fn viewbox_with_commas() {
        let vb = extract_viewbox("10,20 300,400").unwrap();
        assert_eq!(vb.min_x, 10.0);
        assert_eq!(vb.min_y, 20.0);
        assert_eq!(vb.width, 300.0);
        assert_eq!(vb.height, 400.0);
    }

    #[test]
    fn viewbox_invalid_too_few() {
        assert!(extract_viewbox("0 0 200").is_none());
    }

    #[test]
    fn viewbox_invalid_too_many() {
        // Too many values — function requires exactly 4.
        assert!(extract_viewbox("0 0 200 200 100").is_none());
    }

    #[test]
    fn viewbox_zero_width() {
        assert!(extract_viewbox("0 0 0 200").is_none());
    }

    #[test]
    fn viewbox_negative_width() {
        assert!(extract_viewbox("0 0 -100 200").is_none());
    }

    #[test]
    fn viewbox_negative_coords() {
        let vb = extract_viewbox("-100 -100 200 200").unwrap();
        assert_eq!(vb.min_x, -100.0);
        assert_eq!(vb.min_y, -100.0);
    }

    #[test]
    fn viewbox_empty() {
        assert!(extract_viewbox("").is_none());
    }

    #[test]
    fn viewbox_garbage() {
        assert!(extract_viewbox("abc def ghi jkl").is_none());
    }
}
