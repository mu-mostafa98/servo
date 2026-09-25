/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! SVG transform types.
//!
//! SVG spec: <https://www.w3.org/TR/SVG2/coords.html#InterfaceSVGTransform>
//!
//! **No WebRender dependency** — pure data types only. Parsing the raw
//! `transform` attribute lives in the layout layer
//! (`components/layout/svg/transforms.rs`).

/// A single SVG transform operation, in the order it was specified.
#[derive(Debug, Clone)]
pub enum TransformOp {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32, f32, f32), // (angle_deg, cx, cy)
    /// Skew along the X axis by the given angle in degrees.
    SkewX(f32),
    /// Skew along the Y axis by the given angle in degrees.
    SkewY(f32),
    /// Arbitrary 2D transform matrix: matrix(a, b, c, d, e, f).
    /// Represents the transform: [a c e; b d f; 0 0 1]
    Matrix([f32; 6]),
}
