/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG effects pipeline — clip paths, masks, and filters.
//!
//! **Single responsibility:** resolve SVG effect references (clip-path,
//! mask, filter) into WebRender display list primitives.  No tree
//! traversal logic — just effect resolution.

pub(crate) mod clip;
pub(crate) mod filter;
