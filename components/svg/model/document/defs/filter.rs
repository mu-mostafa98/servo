/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<filter>` definitions and their primitive operations.

/// A single SVG filter primitive operation.
#[derive(Debug)]
pub enum FilterPrimitive {
    /// Gaussian blur: std_deviation_x, std_deviation_y.
    GaussianBlur(f32, f32),
    /// Drop shadow: dx, dy, std_deviation, color_r, color_g, color_b, color_a.
    DropShadow(f32, f32, f32, f32, f32, f32, f32),
    /// Full color matrix: 20 values (5 columns × 4 rows).
    ColorMatrix([f32; 20]),
    /// Saturate: single saturation value (0.0 = grayscale, 1.0 = normal, >1.0 = oversaturate).
    Saturate(f32),
    /// Luminance-to-alpha: converts luminance to alpha channel.
    LuminanceToAlpha,
    /// Offset: shifts the input by (dx, dy).
    Offset(f32, f32),
    /// Flood: fills the filter subregion with a solid color (RGBA).
    Flood(f32, f32, f32, f32),
    /// Composite: combines two inputs with an arithmetic composite (k1-k4)
    /// or a Porter-Duff operator.
    Composite(FeCompositeKind),
    /// Tile: repeats the input to fill the filter subregion.
    Tile,
    /// Image: renders an external image or referenced element as a filter input.
    Image(FeImageKind),
}

/// The kind of composite operation for `feComposite`.
#[derive(Debug)]
pub enum FeCompositeKind {
    /// Arithmetic composite: result = k1*i1*i2 + k2*i1 + k3*i2 + k4.
    Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
    /// Porter-Duff `over` operator.
    Over,
    /// Porter-Duff `in` operator.
    In,
    /// Porter-Duff `out` operator.
    Out,
    /// Porter-Duff `atop` operator.
    Atop,
    /// Porter-Duff `xor` operator.
    Xor,
    /// Lighter (additive) composite.
    Lighter,
}

/// The kind of image source for `feImage`.
#[derive(Debug)]
pub enum FeImageKind {
    /// Reference to another element via URL fragment (e.g., `#myElement`).
    FragmentRef(String),
    /// External image URL.
    ExternalUrl(String),
}

/// A filter definition collected from `<filter>`.
#[derive(Debug)]
pub struct FilterDef {
    /// Filter primitives in order (applied left-to-right).
    pub primitives: Vec<FilterPrimitive>,
    /// Filter bounds (x, y, width, height) - may be negative for drop-shadows.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
