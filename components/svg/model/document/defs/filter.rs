/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `<filter>` definitions and their primitive operations.

use malloc_size_of_derive::MallocSizeOf;

/// A single SVG filter primitive operation.
#[derive(Debug, MallocSizeOf)]
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
    /// Blend: combines two inputs with a separable/HSL blend mode.
    Blend(BlendMode),
    /// Merge: composites a list of inputs (`feMergeNode` children) source-over
    /// onto each other. WebRender has no `feMerge` graph op, so this is
    /// emulated as a chain of `feComposite operator="over"`.
    Merge(Vec<String>),
    /// Component transfer: remaps R/G/B/A via per-channel transfer functions.
    ComponentTransfer(ComponentTransfer),
    /// Morphology: dilates (max) or erodes (min) the input over a rectangular
    /// kernel of the given radius.
    Morphology(MorphologyOperator, f32, f32),
    /// Displacement map: displaces `in` by a channel of `in2` scaled by `scale`.
    DisplacementMap(f32, ColorChannel, ColorChannel),
}

/// A single color channel selector (`xChannelSelector`/`yChannelSelector`).
#[derive(Debug, MallocSizeOf)]
pub enum ColorChannel {
    Red,
    Green,
    Blue,
    Alpha,
}

impl ColorChannel {
    /// The SVG IDL channel constant (`SVG_CHANNEL_R` = 1 … `SVG_CHANNEL_A` = 4)
    /// that WebRender's displacement shader expects.
    pub fn to_u32(&self) -> u32 {
        match self {
            ColorChannel::Red => 1,
            ColorChannel::Green => 2,
            ColorChannel::Blue => 3,
            ColorChannel::Alpha => 4,
        }
    }
}

/// The operator for `feMorphology`.
#[derive(Debug, MallocSizeOf)]
pub enum MorphologyOperator {
    /// `dilate`: output the kernel maximum (grows light regions).
    Dilate,
    /// `erode`: output the kernel minimum (shrinks light regions).
    Erode,
}

/// The kind of composite operation for `feComposite`.
#[derive(Debug, MallocSizeOf)]
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
#[derive(Debug, MallocSizeOf)]
pub enum FeImageKind {
    /// Reference to another element via URL fragment (e.g., `#myElement`).
    FragmentRef(String),
    /// External image URL.
    ExternalUrl(String),
}

/// The blend mode for `feBlend`.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// A single filter primitive plus its `in`/`in2`/`result` wiring.
#[derive(Debug, MallocSizeOf)]
pub struct FilterPrimitiveNode {
    /// The primitive operation itself.
    pub op: FilterPrimitive,
    /// `in` attribute: the input source (`SourceGraphic`, `SourceAlpha`,
    /// `BackgroundImage`, a `result` name) or `None` for the previous result.
    pub in1: Option<String>,
    /// `in2` attribute: the second input for `feBlend`/`feComposite`/
    /// `feDisplacementMap`.
    pub in2: Option<String>,
    /// `result` attribute: the name under which this primitive's output is
    /// available to later primitives.
    pub result: Option<String>,
}

/// A single `feComponentTransfer` channel transfer function (feFuncR/G/B/A).
#[derive(Debug, MallocSizeOf)]
pub enum ComponentTransferFunc {
    /// Identity: output = input (the default when no feFunc child is given).
    Identity,
    /// Table: piecewise-linear interpolation through `values` (n values).
    Table(Vec<f32>),
    /// Discrete: step function through `values` (n values).
    Discrete(Vec<f32>),
    /// Linear: output = slope * input + intercept.
    Linear { slope: f32, intercept: f32 },
    /// Gamma: output = amplitude * input^exponent + offset.
    Gamma { amplitude: f32, exponent: f32, offset: f32 },
}

/// The four channel transfer functions of a single `feComponentTransfer`.
#[derive(Debug, MallocSizeOf)]
pub struct ComponentTransfer {
    pub r: ComponentTransferFunc,
    pub g: ComponentTransferFunc,
    pub b: ComponentTransferFunc,
    pub a: ComponentTransferFunc,
}

/// A filter definition collected from `<filter>`.
#[derive(Debug, MallocSizeOf)]
pub struct FilterDef {
    /// Filter primitives in order (applied left-to-right), with input wiring.
    pub primitives: Vec<FilterPrimitiveNode>,
    /// Filter bounds (x, y, width, height) - may be negative for drop-shadows.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
