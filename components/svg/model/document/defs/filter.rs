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
    /// Convolve matrix: 2D convolution of the input with an `order×order` kernel.
    ConvolveMatrix(ConvolveMatrix),
    /// Turbulence: Perlin fractal-noise / turbulence, self-contained (no input).
    Turbulence(Turbulence),
    /// Diffuse lighting: classic diffuse lighting of an alpha height map.
    DiffuseLighting(DiffuseLighting),
    /// Specular lighting: specular highlights of an alpha height map.
    SpecularLighting(SpecularLighting),
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

/// The coordinate system for a filter primitive's subregion attributes
/// (`x`/`y`/`width`/`height`), from the `<filter>` element's `primitiveUnits`.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum PrimitiveUnits {
    /// Attribute lengths are in the current user coordinate system (default).
    UserSpaceOnUse,
    /// Attribute lengths are fractions of the element's object bounding box.
    ObjectBoundingBox,
}

/// The coordinate system for the `<filter>` element's own region attributes
/// (`x`/`y`/`width`/`height`), from `filterUnits`.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum FilterUnits {
    /// Region values are in the current user coordinate system.
    UserSpaceOnUse,
    /// Region values are fractions/percentages of the referencing element's
    /// object bounding box (the default).
    ObjectBoundingBox,
}

/// One component of a filter primitive subregion, parsed from a
/// `<number-or-percentage>` attribute. A percentage is stored as a 0..1
/// fraction so the render layer can resolve it against the filter region,
/// while a bare number is kept as-is so `primitiveUnits` decides whether it
/// is user units or a bounding-box fraction.
#[derive(Debug, Clone, Copy, MallocSizeOf)]
pub enum RegionValue {
    /// A bare `<number>`.
    Number(f32),
    /// A `<percentage>` stored as a 0..1 fraction.
    Percent(f32),
}

/// A single filter primitive plus its `in`/`in2`/`result` wiring and its
/// optional subregion (`x`/`y`/`width`/`height`).
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
    /// `x` attribute (subregion left edge); `None` = the filter region.
    pub x: Option<RegionValue>,
    /// `y` attribute (subregion top edge); `None` = the filter region.
    pub y: Option<RegionValue>,
    /// `width` attribute (subregion width); `None` = the filter region.
    pub width: Option<RegionValue>,
    /// `height` attribute (subregion height); `None` = the filter region.
    pub height: Option<RegionValue>,
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

/// The edge-mode of `feConvolveMatrix` (how out-of-bounds samples are handled).
#[derive(Debug, MallocSizeOf)]
pub enum EdgeMode {
    /// Duplicate: clamp samples to the input edge.
    Duplicate,
    /// None: out-of-bounds samples are transparent black.
    None_,
    /// Wrap: tile/repeat the input.
    Wrap,
}

/// Parameters of `feConvolveMatrix`.
#[derive(Debug, MallocSizeOf)]
pub struct ConvolveMatrix {
    /// Kernel width (number of columns).
    pub order_x: i32,
    /// Kernel height (number of rows).
    pub order_y: i32,
    /// Kernel weights, row-major, zero-padded to a fixed 5×5 slot.
    pub kernel: [f32; 25],
    /// Divisor applied after summing (defaults to the kernel sum).
    pub divisor: f32,
    /// Constant bias added after division.
    pub bias: f32,
    /// Column of the kernel considered the target pixel.
    pub target_x: i32,
    /// Row of the kernel considered the target pixel.
    pub target_y: i32,
    /// Edge handling for out-of-bounds samples.
    pub edge_mode: EdgeMode,
    /// Kernel-unit X scale (usually 1).
    pub kernel_unit_length_x: f32,
    /// Kernel-unit Y scale (usually 1).
    pub kernel_unit_length_y: f32,
    /// When true, the convolution affects only the RGB channels (alpha kept).
    pub preserve_alpha: bool,
}

/// The noise type of `feTurbulence`.
#[derive(Debug, MallocSizeOf)]
pub enum TurbulenceType {
    /// `fractalNoise`: sums the absolute value of each octave.
    FractalNoise,
    /// `turbulence`: sums signed octaves.
    Turbulence,
}

/// Parameters of `feTurbulence`.
#[derive(Debug, MallocSizeOf)]
pub struct Turbulence {
    /// Noise type (`fractalNoise` / `turbulence`).
    pub type_: TurbulenceType,
    /// Base frequency in X.
    pub base_frequency_x: f32,
    /// Base frequency in Y.
    pub base_frequency_y: f32,
    /// Number of octaves to accumulate.
    pub num_octaves: u32,
    /// Random seed.
    pub seed: u32,
    /// Whether to stitch tiles seamlessly.
    pub stitch_tiles: bool,
}

/// The light source of `feDiffuseLighting` / `feSpecularLighting`.
#[derive(Debug, MallocSizeOf)]
pub enum LightSource {
    /// Distant light: a constant direction from `azimuth`/`elevation` (degrees).
    Distant { azimuth: f32, elevation: f32 },
    /// Point light at `(x, y, z)`.
    Point { x: f32, y: f32, z: f32 },
    /// Spot light at `(x, y, z)` pointing at `(points_at_x, points_at_y, points_at_z)`.
    Spot {
        x: f32,
        y: f32,
        z: f32,
        points_at_x: f32,
        points_at_y: f32,
        points_at_z: f32,
        cone_exponent: f32,
        limiting_cone_angle: f32,
    },
}

/// Parameters of `feDiffuseLighting`.
#[derive(Debug, MallocSizeOf)]
pub struct DiffuseLighting {
    /// Surface scale applied to the height map.
    pub surface_scale: f32,
    /// Diffuse reflection constant.
    pub diffuse_constant: f32,
    /// Kernel-unit X scale (usually 1).
    pub kernel_unit_length_x: f32,
    /// Kernel-unit Y scale (usually 1).
    pub kernel_unit_length_y: f32,
    /// `lighting-color` (RGBA), applied to the lighting result.
    pub lighting_color: [f32; 4],
    /// The light source.
    pub light_source: LightSource,
}

/// Parameters of `feSpecularLighting`.
#[derive(Debug, MallocSizeOf)]
pub struct SpecularLighting {
    /// Surface scale applied to the height map.
    pub surface_scale: f32,
    /// Specular reflection constant.
    pub specular_constant: f32,
    /// Specular exponent (shininess).
    pub specular_exponent: f32,
    /// Kernel-unit X scale (usually 1).
    pub kernel_unit_length_x: f32,
    /// Kernel-unit Y scale (usually 1).
    pub kernel_unit_length_y: f32,
    /// `lighting-color` (RGBA), applied to the lighting result.
    pub lighting_color: [f32; 4],
    /// The light source.
    pub light_source: LightSource,
}

/// A filter definition collected from `<filter>`.
#[derive(Debug, MallocSizeOf)]
pub struct FilterDef {
    /// Filter primitives in order (applied left-to-right), with input wiring.
    pub primitives: Vec<FilterPrimitiveNode>,
    /// Filter bounds (x, y, width, height) — `<number-or-percentage>` values
    /// resolved against the referencing element's bounding box at render time.
    /// May be negative for drop-shadows.
    pub x: RegionValue,
    pub y: RegionValue,
    pub width: RegionValue,
    pub height: RegionValue,
    /// Coordinate system for the filter region (`filterUnits`).
    pub filter_units: FilterUnits,
    /// Coordinate system for primitive subregions (`primitiveUnits`).
    pub primitive_units: PrimitiveUnits,
}
