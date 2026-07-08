/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use kurbo::BezPath;

/// SVG `<path>` element with its `d` attribute parsed into a [`BezPath`].
#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

impl crate::shapes::BuildFromElement for Path {
    fn from_attrs(_font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        let value = attrs.get_attr("d")?;
        BezPath::from_svg(&value).ok().map(|path| Path { path })
    }
}
