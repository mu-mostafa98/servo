/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG viewport types — `viewBox`, `preserveAspectRatio`, and viewport info.

use crate::model::units::Length;

/// SVG `preserveAspectRatio` alignment type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectAlign {
    None,
    XMinYMin,
    XMidYMin,
    XMaxYMin,
    XMinYMid,
    XMidYMid,
    XMaxYMid,
    XMinYMax,
    XMidYMax,
    XMaxYMax,
}

/// SVG `preserveAspectRatio` meet-or-slice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeetOrSlice {
    Meet,
    Slice,
}

/// Parsed `preserveAspectRatio` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectRatio {
    pub align: AspectAlign,
    pub meet_or_slice: MeetOrSlice,
}

impl Default for AspectRatio {
    fn default() -> Self {
        // SVG spec: viewBox alone implies preserveAspectRatio="xMidYMid meet".
        AspectRatio {
            align: AspectAlign::XMidYMid,
            meet_or_slice: MeetOrSlice::Meet,
        }
    }
}

/// SVG `viewBox` attribute.
#[derive(Debug, Clone, Copy)]
pub struct ViewBox {
    pub min_x: Length,
    pub min_y: Length,
    pub width: Length,
    pub height: Length,
}

/// Viewport information for the root `<svg>` element.
#[derive(Debug, Clone)]
pub struct ViewportInfo {
    pub width: Length,
    pub height: Length,
    pub view_box: Option<ViewBox>,
    /// When true, the viewport clip is omitted (CSS `overflow: visible`).
    pub overflow_visible: bool,
    /// Parsed preserveAspectRatio (defaults to xMidYMid meet).
    pub aspect_ratio: Option<AspectRatio>,
}

/// Viewport established by a nested `<svg>` element.
///
/// Unlike the root [`ViewportInfo`] (whose size is imposed by layout), a nested
/// `<svg>` carries its own `x`/`y`/`width`/`height` attributes that position and
/// size the sub-viewport in the parent user coordinate system, plus an optional
/// `viewBox` and `preserveAspectRatio` that map content into it.
#[derive(Debug, Clone)]
pub struct SvgViewport {
    /// Position of the viewport in the parent user coordinate system.
    pub x: Length,
    pub y: Length,
    /// Size of the viewport (from the `width`/`height` attributes).
    pub width: Length,
    pub height: Length,
    pub view_box: Option<ViewBox>,
    /// Parsed preserveAspectRatio (defaults to xMidYMid meet via the renderer).
    pub aspect_ratio: Option<AspectRatio>,
    /// When true, the sub-viewport clip is omitted (`overflow: visible`).
    pub overflow_visible: bool,
}
