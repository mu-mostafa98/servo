/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Programmatic construction of a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! This is the Phase 2 bridge: instead of serializing the SVG subtree back to
//! XML (which discards the CSS cascade) and letting usvg re-parse it, we walk
//! the DOM on the layout thread — where computed styles are available — and
//! build the usvg tree directly via usvg's public constructors.
//!
//! Reading [`ComputedValues`] means `fill`, `stroke`, `opacity`, and the shape
//! geometry properties (`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) are taken from the
//! post-cascade result, so stylesheets and presentation attributes both apply.
//!
//! The code is layered, with dependencies flowing downward:
//!
//! * [`builder`] — the entry point and per-element converter facades, coordinated
//!   through a single [`builder::SvgContext`].
//! * [`effects`] — paint servers, clip paths, masks and filters.
//! * [`primitives`] — leaf attribute/geometry/text parsing, free of any effect or
//!   builder knowledge.
//! * [`raster`] — the final rasterization step into WebRender image-cache pixels.

mod builder;
mod effects;
mod primitives;
mod raster;

pub(crate) use builder::build_usvg_tree;
pub(crate) use raster::rasterize_svg_tree;
