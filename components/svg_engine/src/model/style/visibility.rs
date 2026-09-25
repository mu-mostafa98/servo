/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG visibility and display properties.
//!
//! These are universal CSS presentation attributes per the SVG 2 spec,
//! not node effects. They live in their own file for clarity.

/// Element visibility.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Visible,
    Hidden,
}

/// Element display type.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Display {
    Inline,
    Block,
    None,
}
