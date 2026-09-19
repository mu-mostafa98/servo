/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Programmatic construction of a [`usvg::Tree`] from Servo's SVG DOM.
//!
//! Instead of serializing the SVG subtree back to XML (which discards the CSS
//! cascade) and letting usvg re-parse it, we walk the DOM on the layout thread —
//! where computed styles are available — and build the usvg tree directly via
//! usvg's public constructors.
//!
//! This first phase supports basic shape elements (`rect`, `circle`, `ellipse`,
//! `line`, `polyline`, `polygon`, `path`) with solid `fill`/`stroke`, inside
//! `<svg>`/`<g>` containers. Paint servers (gradients/patterns), `<use>`,
//! `<text>`, `<image>` and clip/mask/filter effects are deferred to later phases.
//!
//! The code is layered, with dependencies flowing downward:
//!
//! * [`usvg_builder`] — the entry point and per-element build facades.
//! * [`primitives`] — leaf attribute/geometry/paint parsing, free of any builder
//!   knowledge.
//! * [`raster`] — the final rasterization step into WebRender image-cache pixels.

mod primitives;
mod raster;
mod usvg_builder;

pub(crate) use raster::rasterize_svg_tree;
pub(crate) use usvg_builder::build_usvg_tree;
