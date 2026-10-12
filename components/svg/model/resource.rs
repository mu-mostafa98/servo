/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Opaque handles to WebRender resources uploaded by the layout layer.

/// An opaque handle to a WebRender resource (an image or font instance)
/// uploaded by the layout layer during build.
///
/// Stores only the raw `(namespace, id)` pair so the model carries no
/// WebRender dependency; the render layer reconstructs the concrete
/// [`webrender_api`] key type (`ImageKey` / `FontInstanceKey`) when pushing
/// display items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceKey {
    /// WebRender id namespace.
    pub namespace: u32,
    /// Resource index within the namespace.
    pub id: u32,
}
