/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use webrender_api::{
    DisplayListBuilder,
    units::{LayoutPoint, LayoutRect},
};

use crate::shapes::*;
use crate::styles::*;

// ------------------ Renderers ------------------

pub fn render_rect(
    rect: &Rectangle,
    style: &NodeStyle,
    svg_origin: &LayoutPoint,
    wr: &mut DisplayListBuilder,
) {
    

}