# WebRender API Analysis for SVG Rendering

> Based on `webrender_api` v0.69.0  
> Source: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/webrender_api-0.69.0/src/`

---

## 1. Complete WebRender API Reference

### 1.1 Display Items (`DisplayItem` enum in `display_item.rs`)

| Display Item | Push Method | SVG Relevance |
|---|---|---|
| `Rectangle` | `push_rect()` / `push_rect_with_animation()` | ✅ Core — fills, axis-aligned lines |
| `Border` | `push_border()` | ✅ Stroke rendering, rounded rect stroke |
| `Line` | `push_line()` | ❌ **Only Horizontal/Vertical** — text-decoration only |
| `Image` | `push_image()` | ✅ `<image>` element |
| `RepeatingImage` | `push_repeating_image()` | 🔄 `<pattern>` fill |
| `Text` | `push_text()` | ✅ `<text>` element |
| `Gradient` | `push_gradient()` | ✅ `<linearGradient>` |
| `RadialGradient` | `push_radial_gradient()` | ✅ `<radialGradient>` |
| `ConicGradient` | `push_conic_gradient()` | 🔄 CSS conic-gradient, SVG 2 |
| `BoxShadow` | `push_box_shadow()` | 🔄 `<filter>` drop-shadow |
| `BackdropFilter` | `push_backdrop_filter()` | 🔄 CSS backdrop-filter |
| `HitTest` | `push_hit_test()` | 🔄 Pointer events |
| `PushStackingContext` | `push_stacking_context()` | ✅ **Groups** (`<g>`), opacity, mix-blend-mode, filters |
| `PushReferenceFrame` | `push_reference_frame()` | ✅ **Transforms**, viewBox, coordinate nesting |
| `Iframe` | `push_iframe()` | 🔄 `<foreignObject>` |
| `PushShadow` | `push_shadow()` | 🔄 SVG shadow filters |
| `YuvImage` | `push_yuv_image()` | 🔄 video/images |

### 1.2 Struct Fields (from `display_item.rs`)

```rust
// --- Rectangle — pure fill, solid or animated color ---
pub struct RectangleDisplayItem {
    pub common: CommonItemProperties,
    pub bounds: LayoutRect,
    pub color: PropertyBinding<ColorF>,
}

// --- Border — per-side styles, widths, and per-corner radius ---
pub struct NormalBorder {
    pub left: BorderSide,
    pub right: BorderSide,
    pub top: BorderSide,
    pub bottom: BorderSide,
    pub radius: BorderRadius,      // <-- per-corner radii
    pub do_aa: bool,
}
pub struct BorderSide {
    pub color: ColorF,
    pub style: BorderStyle,       // None, Solid, Double, Dotted, Dashed, etc.
}

// --- BorderRadius — different rx/ry per corner ---
pub struct BorderRadius {
    pub top_left: LayoutSize,
    pub top_right: LayoutSize,
    pub bottom_left: LayoutSize,
    pub bottom_right: LayoutSize,
}

// --- CommonItemProperties — every display item needs one ---
pub struct CommonItemProperties {
    pub clip_rect: LayoutRect,
    pub clip_chain_id: ClipChainId,
    pub spatial_id: SpatialId,
    pub flags: PrimitiveFlags,
}
```

### 1.3 Clipping APIs (in `display_list.rs`)

| Method | Signature | SVG Use |
|---|---|---|
| `define_clip_rect` | `(spatial_id, clip_rect) → ClipId` | ✅ Basic `<clipPath>` |
| `define_clip_rounded_rect` | `(spatial_id, ComplexClipRegion{rect, radii, mode}) → ClipId` | ✅ Ellipse/circle fill, rounded shape fill |
| `define_clip_image_mask` | `(spatial_id, ImageMask{image, rect}, points, fill_rule) → ClipId` | ✅ **Polygon/path clip** — takes `Vec<LayoutPoint>` + `FillRule` |
| `define_clip_chain` | `(parent, [clip_ids]) → ClipChainId` | ✅ Composing multiple clips |

**Critical detail:** `define_clip_image_mask` uses `IMPLICIT points: Vec<LayoutPoint>` — you push `SetPoints` marker + point data before the clip item. The `ImageMask` can use `ImageKey::DUMMY` for pure polygon clipping.

#### FillRule (in `display_item.rs`)
```rust
pub enum FillRule {
    Nonzero = 0x1,   // Behaves as the SVG fill-rule definition for nonzero.
    Evenodd = 0x2,   // Behaves as the SVG fill-rule definition for evenodd.
}
```

### 1.4 Coordinate System APIs (in `display_list.rs`)

```rust
// Creates a new coordinate system with a transform matrix
push_reference_frame(
    origin: LayoutPoint,
    parent_spatial_id: SpatialId,
    transform_style: TransformStyle,       // Flat | Preserve3D
    transform: PropertyBinding<LayoutTransform>,
    kind: ReferenceFrameKind,
) -> SpatialId

// Must be paired with:
pop_reference_frame()

// Isolates rendering (opacity, filters, blend modes)
push_stacking_context(
    spatial_id: SpatialId,
    prim_flags: PrimitiveFlags,
    clip_chain_id: Option<ClipChainId>,
    transform_style: TransformStyle,
    mix_blend_mode: MixBlendMode,
    filters: &[FilterOp],
    filter_datas: &[FilterData],
    raster_space: RasterSpace,            // Local(f32) | Screen
    flags: StackingContextFlags,
    snapshot: Option<SnapshotInfo>,
)

// Must be paired with:
pop_stacking_context()
```

#### ReferenceFrameKind (in `display_item.rs`)
```rust
pub enum ReferenceFrameKind {
    Transform {
        is_2d_scale_translation: bool,  // Performance hint
        should_snap: bool,              // Snap to pixels
        paired_with_perspective: bool,
    },
    Perspective {
        scrolling_relative_to: Option<ExternalScrollId>,
    }
}
```

### 1.5 Gradient Types (in `display_item.rs`)

```rust
// Linear gradient
pub struct Gradient {
    pub start_point: LayoutPoint,
    pub end_point: LayoutPoint,
    pub extend_mode: ExtendMode,  // Clamp | Repeat
} // IMPLICIT: stops: Vec<GradientStop>

// Radial gradient
pub struct RadialGradient {
    pub center: LayoutPoint,
    pub radius: LayoutSize,
    pub start_offset: f32,
    pub end_offset: f32,
    pub extend_mode: ExtendMode,
} // IMPLICIT: stops: Vec<GradientStop>

// Conic gradient
pub struct ConicGradient {
    pub center: LayoutPoint,
    pub angle: f32,
    pub start_offset: f32,
    pub end_offset: f32,
    pub extend_mode: ExtendMode,
} // IMPLICIT: stops: Vec<GradientStop>
```

### 1.6 SVG Filter Support (in `display_item.rs`)

WebRender has **comprehensive** SVG filter primitive support in `FilterOp`:

| Category | FilterOp Variants |
|---|---|
| **CSS filters** | `Blur`, `Brightness`, `Contrast`, `Grayscale`, `HueRotate`, `Invert`, `Opacity`, `Saturate`, `Sepia`, `DropShadow`, `ColorMatrix` |
| **Source inputs** | `SVGFESourceGraphic`, `SVGFESourceAlpha` |
| **Blend modes** (18) | `SVGFEBlendNormal/Multiply/Screen/Overlay/Darken/Lighten/ColorDodge/ColorBurn/HardLight/SoftLight/Difference/Exclusion/Hue/Saturation/Color/Luminosity` |
| **Color** | `SVGFEColorMatrix`, `SVGFEComponentTransfer`, `SVGFEFlood`, `SVGFEToAlpha` |
| **Composite** | `SVGFECompositeOver/In/Out/Atop/XOR/Lighter/Arithmetic` |
| **Filter primitives** | `SVGFEGaussianBlur`, `SVGFEDropShadow`, `SVGFEDisplacementMap`, `SVGFEMorphologyDilate/Erode`, `SVGFEOffset`, `SVGFETile`, `SVGFEImage`, `SVGFEIdentity` |
| **Lighting** | `SVGFEDiffuseLightingDistant/Point/Spot`, `SVGFESpecularLightingDistant/Point/Spot` |
| **Turbulence** | `SVGFETurbulenceWithFractalNoiseWith/WithoutStitching`, `SVGFETurbulenceWithTurbulenceNoiseWith/WithoutStitching` |

---

## 2. Current `svg_engine` Crate Architecture

### 2.1 File Map

```
components/svg_engine/
├── Cargo.toml                   # Deps: webrender_api, euclid, kurbo, stylo
└── src/
    ├── lib.rs                   # Re-exports, public API
    ├── shapes.rs                # Shape enum + geometry structs (no SvgTransform)
    ├── styles.rs                # NodeStyle, FillParams, StrokeParams
    ├── extract.rs               # SVG attribute parsing → engine types
    ├── render_tree.rs           # SvgRenderTree, SvgRenderNode
    ├── render.rs                # Tree traversal → per-shape dispatch
    └── renderers/
        ├── mod.rs               # Re-exports 4 render functions
        ├── rect.rs              # Rect fill+stroke (rounded corners via clip+border)
        ├── ellipse.rs           # Ellipse → Rect delegation (rx=w/2, ry=h/2)
        ├── circle.rs            # Circle → Ellipse delegation
        └── line.rs              # Line → rotated rect via reference frame
```

### 2.2 Data Flow

```
Servo layout (replaced.rs)
    │
    ▼
extract.rs ──→ render_tree.rs ──→ render.rs ──→ renderers/*.rs
    │               │                │                │
    │ Extract       │ Build          │ Walk            │ Push WebRender
    │ attributes    │ render         │ tree,           │ display items
    │ from DOM      │ tree from      │ dispatch        │ per shape
    │                │ extracted data │ by shape type   │
    ▼                ▼                ▼                ▼
NodeStyle          SvgRenderTree    render_dispatch() push_rect()
SvgTag                                                push_border()
                                                     push_reference_frame()
```

### 2.3 All WebRender API Calls in `svg_engine`

| Location | WebRender Call | Purpose |
|----------|---------------|---------|
| `renderers/rect.rs:65` | `wr.push_rect()` | Filled rect with rounded corners (via clip) |
| `renderers/rect.rs:71` | `wr.push_rect()` | Filled rect without rounded corners |
| `renderers/rect.rs:43-60` | `wr.define_clip_rounded_rect()` + `wr.define_clip_chain()` | Rounded clip for fill |
| `renderers/rect.rs:98` | `wr.push_border()` | Stroked rect (with or without radius) |
| `renderers/line.rs` | `wr.push_reference_frame()` + `wr.push_rect()` + `wr.pop_reference_frame()` | Single code path for all lines |

---

## 3. Shape Rendering Strategy

### 3.1 Current Strategy

| Shape | Fill | Stroke | Notes |
|-------|------|--------|-------|
| `<rect>` (plain) | `push_rect` | `push_border` | Direct |
| `<rect>` (rounded) | clip-rounded-rect + `push_rect` | `push_border` with radius | Clip for fill, border for stroke |
| `<circle>` | Ellipse→Rect delegation | Same chain | `rx=ry=r` |
| `<ellipse>` | Rect delegation (`rx=w/2, ry=h/2`) | Same chain | Mathematically perfect |
| `<line>` | `push_reference_frame` + `push_rect` | N/A (stroke is fill) | Single path, no branching |
| `<polyline>` | ❌ Not implemented | ❌ Not implemented | |
| `<polygon>` | ❌ Not implemented | ❌ Not implemented | |
| `<path>` | ❌ Not implemented | ❌ Not implemented | |

### 3.2 Future Strategy Recommendations

| Phase | Shape | Recommended WebRender Approach |
|---|---|---|
| **Phase 2** | `<polygon>` | `define_clip_image_mask` with vertex `points` + `FillRule` + `push_rect` fill |
| **Phase 2** | `<polyline>` | Same as polygon (no implicit Z close) + `push_border` for stroke |
| **Phase 2** | `<path>` | Tessellate via `lyon` → polygon clip + fill, or push individual triangles |
| **Phase 3** | `<g>` (group) | `push_stacking_context` with `MixBlendMode::Normal` |
| **Phase 3** | `transform` | `push_reference_frame` with computed `LayoutTransform` |
| **Phase 3** | `viewBox` | `push_reference_frame` with scale+translate matrix |
| **Phase 4** | `<text>` | `push_text` with `GlyphInstance` glyphs + `FontInstanceKey` |
| **Phase 5** | `<clipPath>` | `define_clip_image_mask` for polygon, `define_clip_rounded_rect` for rounded |
| **Phase 5** | `<mask>` | Stacking context with snapshot |
| **Phase 6** | `<linearGradient>` | `push_gradient` with `create_gradient` + stops |
| **Phase 6** | `<radialGradient>` | `push_radial_gradient` with `create_radial_gradient` |
| **Phase 6** | SVG filters | `push_stacking_context` with `&[FilterOp]` |

---

## 4. Key Facts

- **No `push_line` for SVG** — it's Horizontal/Vertical only, for CSS text decorations
- **No `push_ellipse` / `push_circle`** — use `push_border` with 50% border-radius
- **`define_clip_image_mask` with `ImageKey::DUMMY`** — polygon clipping for path/polygon/polyline
- **`push_reference_frame`** — the correct mechanism for SVG transforms and angled lines
- **Gradients are tiled** — `tile_size` + `tile_spacing` for pattern support
- **All SVG filter primitives exist** in `FilterOp` enum

### Dependencies

| Crate | Used For | Status |
|-------|----------|--------|
| `webrender_api` | Display list building | ✅ Workspace |
| `euclid` | Geometry types, transforms | ✅ Workspace |
| `kurbo` | BezPath for path data | ✅ Workspace |
| `stylo` | ComputedValues in extract.rs | ✅ Workspace |
| `lyon` | Path tessellation | ⬜ Recommend adding for Phase 2 |
