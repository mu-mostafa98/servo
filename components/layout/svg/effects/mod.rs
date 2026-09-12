/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Paint servers, clip paths, masks and filters.
//!
//! These resolvers turn Servo SVG DOM nodes into usvg's effect nodes. They sit
//! between the leaf [`crate::svg::primitives`] (parsing/geometry/text) and the
//! [`crate::svg::usvg_builder`] facades, and may call back into the builder for
//! recursive content (`<pattern>`/`<clipPath>`/`<mask>` children) — the cycle is
//! fine in Rust.

pub(crate) mod clip;
pub(crate) mod filter;
pub(crate) mod mask;
pub(crate) mod paint;

use std::sync::Arc;

use resvg::usvg;
use script::layout_dom::ServoLayoutElement;

use crate::svg::effects::clip::{ClipPathOutcome, resolve_clip_path};
use crate::svg::effects::filter::{FilterOutcome, resolve_filter};
use crate::svg::effects::mask::{MaskOutcome, resolve_mask};
use crate::svg::usvg_builder::SvgContext;

/// The clip-path/mask/filter triple resolved for an element.
pub(crate) struct Effects {
    pub(crate) clip_path: Option<Arc<usvg::ClipPath>>,
    pub(crate) mask: Option<Arc<usvg::Mask>>,
    pub(crate) filter: Option<Arc<usvg::filter::Filter>>,
}

/// Resolves an element's `clip-path`, `mask` and `filter` references into an
/// [`Effects`] triple.
///
/// `object_bbox` is the bounding box of the element being affected (used by
/// `objectBoundingBox` units). `None` means a present-but-invalid effect — the
/// caller must drop the element (usvg's `convert_group` handshake).
pub(crate) fn resolve_effects<'a, 'dom>(
    element: &ServoLayoutElement<'dom>,
    ctx: &SvgContext<'a, 'dom>,
    object_bbox: Option<usvg::NonZeroRect>,
) -> Option<Effects> {
    let clip_path = match resolve_clip_path(element, ctx, object_bbox) {
        ClipPathOutcome::Clip(clip) => Some(clip),
        ClipPathOutcome::Invalid => return None,
        ClipPathOutcome::None => None,
    };
    let mask = match resolve_mask(element, ctx, object_bbox) {
        MaskOutcome::Mask(mask) => Some(mask),
        MaskOutcome::Invalid => return None,
        MaskOutcome::None => None,
    };
    let filter = match resolve_filter(element, ctx, object_bbox) {
        FilterOutcome::Filter(filter) => Some(filter),
        FilterOutcome::Invalid => return None,
        FilterOutcome::None => None,
    };

    Some(Effects {
        clip_path,
        mask,
        filter,
    })
}
