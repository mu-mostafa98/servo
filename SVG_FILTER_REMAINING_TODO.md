# SVG Filter Gaps — Remaining Work (handoff spec)

This document is a **self-contained handoff** for finishing the SVG `<filter>`
primitive gaps in the Servo SVG engine. A fresh Claude Code session (another
device/chat) should be able to read this file plus the two "worked example"
primitives already committed in the tree, and implement the rest without
re-deriving the architecture.

---

## 0. What this project is

- Repo root: `D:\Projects\servo` (branch `svg-vello-one-commit`).
- The task: complete all gaps in `<filter>`, `<mask>`, `<clipPath>`.
  - `<clipPath>` and `<mask>` are **done** (objectBoundingBox + stroke
    rasterization). Do not redo them.
- `<filter>` is partially done. See §2 for what is already finished and §4 for
  what remains.
- **Do NOT touch the resvg/tiny-skia (vello_cpu) legacy pipeline** — it is
  deprecated. The only acceptable CPU-raster fallback already in use is for
  masks.

### Standing constraints (from the user, still in force)

1. Do not mention a problem/gap without stating a clear solution.
2. Work autonomously ("auto mode") — do not ask for confirmation.
3. Deprecate resvg/vello_cpu; do not use it for new filter work.

---

## 1. The architecture (4 layers) — read this first

Every filter primitive is implemented across exactly 4 layers. There are two
**already-finished, working examples** in the tree to copy from:
**`feMorphology`** (single input, simple params) and **`feDisplacementMap`**
(two inputs + params). Read these files and mirror them.

### Layer 1 — Model (servo-svg)

`components/svg/model/document/defs/filter.rs`

- Add a variant to `enum FilterPrimitive`.
- If the primitive has a small enum-of-options, add that enum here too
  (see `MorphologyOperator`, `ColorChannel`).
- Re-export any **new public type** in **two** places:
  1. `components/svg/model/document/defs/mod.rs` (`pub use self::filter::{...}`)
  2. `components/svg/model/document/mod.rs` (`pub use self::defs::{...}`)

### Layer 2 — Parser

`components/layout/svg/defines/filter.rs`

- Add a `match pname.as_str()` branch inside `parse(...)`.
- Helper closures already available inside the loop:
  - `prim_get(attr) -> Option<String>`
  - `prim_get_float(attr, default) -> f32`
  - `prim_get_float_pair(attr, default) -> (f32, f32)` (number-optional-number)
  - `push(FilterPrimitive)` wraps a parsed primitive with its `in`/`in2`/`result`
  - `parse_color_rgba(&str) -> (r,g,b,a)` for colors
- For primitives with child elements (feComponentTransfer's `feFuncR`…, lighting's
  `feDistantLight`…, feMerge's `feMergeNode`), iterate `prim_child.dom_children()`
  exactly like the existing `feComponentTransfer`/`feMerge` branches do.

### Layer 3 — Render (servo-svg → WebRender FilterOp)

`components/svg/render/effects/filter.rs`, function `get_filter_ops`.

- Node 0 = `SVGFESourceGraphic`, node 1 = `SVGFESourceAlpha` (already emitted
  upfront). Inputs resolve via `resolve_input`.
- Helper constructors (already defined, all use `linear: false`):
  - `single_input_node(input: i16, region)`
  - `two_input_node(input1: i16, input2: i16, region)`
  - `no_input_node(region)`
- Add a `match &prim.op` arm emitting the right `FilterOp::SVGFE*` variant.

### Layer 4 — WebRender fork (shader + CPU param packing)

WebRender is **forked** at `D:\Projects\webrender\webrender`, wired in
`D:\Projects\servo\Cargo.toml` via:

```toml
[patch.crates-io]
webrender = { path = "../webrender/webrender" }
```

(only `webrender` is patched; `webrender_api` still comes from crates.io 0.70.0).

The relevant files:

1. **`D:\Projects\webrender\webrender\res\cs_svg_filter_node.glsl`** — one file
   containing both vertex and fragment shaders. `main(void)` is the fragment
   shader. The `// TODO` branches live in the long `if/else if (vFilterKind == …)`
   chain starting around line 614.
2. **`D:\Projects\webrender\webrender\src\render_task.rs`** — `extra_gpu_data`
   packing (function around line 740–810). Some primitives are already packed
   (see §3); others are empty `{}` and need packing added.
3. `D:\Projects\webrender\webrender\src\scene_building.rs` — already handles
   source/target padding for all primitives; do not edit unless a new primitive
   needs padding (morphology/displacement/lighting/convolve already do).

### Shader data-flow cheat sheet (fragment shader)

- `Rs` = raw premultiplied color of input 1 (`sColor0`), `Ns` = unpremultiplied.
- `Rb` / `Nb` = same for input 2 (`sColor1`).
- `vInput1Uv` / `vInput2Uv` = UV of the current output pixel in each input texture.
- `vInput1UvRect` / `vInput2UvRect` = clamp rects (in UV) for each input.
- `vFilterData0`, `vFilterData1` = `vec4` scratch varyings for params.
- `vData` = `ivec4` (used to pass a gpu-buffer address to the fragment stage).
- `sampleInUvRect(sampler, uv, uvRect)` = clamped sample.
- `sampleInUvRectRepeat(sampler, uv, uvRect)` = tiled/repeat sample.
- `TEX_SIZE(sColor0).xy` gives a texture's size; `1.0/vec2(TEX_SIZE(...))` is one
  texel in UV space (== one device pixel, because SVG filter nodes are 1:1).
- `fetch_from_gpu_buffer_1f(addr)` / `_4f_direct` / `_1f_direct` read the gpu
  buffer (see existing `FILTER_COLOR_MATRIX`, `FILTER_COMPONENT_TRANSFER`,
  `FILTER_DISPLACEMENT_MAP` cases).
- Param values from `FilterOp` are converted to **device pixels** in
  `render_task.rs` (~line 1300–1430) before reaching the shader (e.g. morphology
  radius, displacement scale, kernel unit lengths are all device-space).

---

## 2. Already done (do not redo)

| Primitive | Status |
|---|---|
| feMerge | done, verified |
| feComponentTransfer (table/discrete/linear/gamma) | done, verified (fork `filterdata.rs`) |
| feMorphology (dilate/erode) | done, **verified pixel-exact** |
| feDisplacementMap | **all 4 layers written, `servo-svg` compiles, but NOT render-verified yet** |

The `feDisplacementMap` code is in the tree and is a valid template. Before
using it as a reference, it is worth verifying it renders (see §6) so you know
the 4-layer pattern is sound end-to-end.

---

## 3. Reference: FilterOp variants and CPU-packing state

All variants are defined in
`webrender_api-0.70.0/src/display_item.rs` (lines ~1500–1696). Field lists below.

`[node]` = `FilterOpGraphNode` (built by the helper constructors in Layer 3).

| Primitive | `FilterOp` variant(s) | `extra_gpu_data` packing (render_task.rs) |
|---|---|---|
| Convolve | `SVGFEConvolveMatrixEdgeMode{Duplicate\|None\|Wrap}` | **DONE** (~line 755) — see caveat |
| Displacement | `SVGFEDisplacementMap` | DONE (~line 773) |
| Morphology | `SVGFEMorphologyDilate` / `Erode` | DONE (redundant; shader uses `input_2_content_scale_and_offset`) |
| DiffuseLighting | 3 variants | **NOT DONE** (empty `{}`) |
| SpecularLighting | 3 variants | **NOT DONE** (empty `{}`) |
| Turbulence | 4 variants | **NOT DONE** (empty `{}`) |
| Image | `SVGFEImage` | n/a (needs real image loading, not gpu params) |

Exact `FilterOp` field lists (copy verbatim):

```
SVGFEConvolveMatrixEdgeMode{Duplicate|None|Wrap} {
    node, order_x: i32, order_y: i32, kernel: [f32; 25], divisor: f32,
    bias: f32, target_x: i32, target_y: i32,
    kernel_unit_length_x: f32, kernel_unit_length_y: f32, preserve_alpha: i32
}

SVGFEDisplacementMap { node, scale: f32, x_channel_selector: u32, y_channel_selector: u32 }

SVGFEDiffuseLightingDistant { node, surface_scale: f32, diffuse_constant: f32,
    kernel_unit_length_x: f32, kernel_unit_length_y: f32, azimuth: f32, elevation: f32 }
SVGFEDiffuseLightingPoint   { node, surface_scale: f32, diffuse_constant: f32,
    kernel_unit_length_x: f32, kernel_unit_length_y: f32, x: f32, y: f32, z: f32 }
SVGFEDiffuseLightingSpot    { node, surface_scale: f32, diffuse_constant: f32,
    kernel_unit_length_x: f32, kernel_unit_length_y: f32, x: f32, y: f32, z: f32,
    points_at_x: f32, points_at_y: f32, points_at_z: f32,
    cone_exponent: f32, limiting_cone_angle: f32 }

SVGFESpecularLightingDistant { node, surface_scale: f32, specular_constant: f32,
    specular_exponent: f32, kernel_unit_length_x: f32, kernel_unit_length_y: f32,
    azimuth: f32, elevation: f32 }
SVGFESpecularLightingPoint   { node, surface_scale: f32, specular_constant: f32,
    specular_exponent: f32, kernel_unit_length_x: f32, kernel_unit_length_y: f32,
    x: f32, y: f32, z: f32 }
SVGFESpecularLightingSpot    { node, surface_scale: f32, specular_constant: f32,
    specular_exponent: f32, kernel_unit_length_x: f32, kernel_unit_length_y: f32,
    x: f32, y: f32, z: f32, points_at_x: f32, points_at_y: f32, points_at_z: f32,
    cone_exponent: f32, limiting_cone_angle: f32 }

SVGFETurbulenceWith{FractalNoise|TurbulenceNoise}With{NoStitching|Stitching} {
    node, base_frequency_x: f32, base_frequency_y: f32, num_octaves: u32, seed: u32 }

SVGFEImage { node, sampling_filter: u32, matrix: [f32; 6] }
```

### Shader `vFilterKind` constants (cs_svg_filter_node.glsl, top)

```
FILTER_MORPHOLOGY_DILATE 80 / _CONVERTSRGB 81, ERODE 82 / 83
FILTER_CONVOLVE_MATRIX_EDGE_MODE_DUPLICATE 56, NONE 58, WRAP 60  (each +1 = CONVERTSRGB)
FILTER_DIFFUSE_LIGHTING_DISTANT 62, POINT 64, SPOT 66
FILTER_DISPLACEMENT_MAP 68
FILTER_SPECULAR_LIGHTING_DISTANT 86, POINT 88, SPOT 90
FILTER_TURBULENCE_* 94..101
FILTER_IMAGE 76
FILTER_BITFLAGS_CONVERTSRGB 1
```

`node.linear == true` selects the `+1` (`_CONVERTSRGB`) variant. **Note:** the
servo-side helper constructors hardcode `linear: false`, so the `_CONVERTSRGB`
paths are currently never taken — see §7.

---

## 4. The remaining tasks

Do them in this order (roughly increasing difficulty). Each task lists the
specific work per layer.

### Task A — Verify feDisplacementMap (5 min)

It is already written. Build + render a test (§6) to confirm, then move on.

Suggested test: a horizontal gradient displaced by a vertical gradient:

```html
<svg width="300" height="100" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="a" x1="0" x2="1"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>
    <linearGradient id="b" x1="0" x2="1"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>
    <filter id="disp">
      <feDisplacementMap in="SourceGraphic" in2="SourceGraphic" scale="30"
                         xChannelSelector="R" yChannelSelector="G"/>
    </filter>
  </defs>
  <rect x="0" y="0" width="300" height="100" fill="url(#a)" filter="url(#disp)"/>
</svg>
```

(For a stronger effect use an image/gradient as `in2`; the point of the test is
just "does it render non-magenta and visibly displace".)

---

### Task B — feConvolveMatrix

**Layer 1 (model):** add
`FilterPrimitive::ConvolveMatrix(ConvolveMatrix)` with a struct holding:

```
order_x: i32, order_y: i32, kernel: [f32; 25],
divisor: f32, bias: f32, target_x: i32, target_y: i32,
edge_mode: EdgeMode,           // new enum {Duplicate, None_, Wrap}
kernel_unit_length_x: f32, kernel_unit_length_y: f32,
preserve_alpha: bool
```

(Re-export `EdgeMode` in the two `mod.rs` files.)

**Layer 2 (parser):** parse `order` ("3" or "3 4"), `kernelMatrix`
(whitespace/comma-separated numbers, must equal order_x*order_y, pad/truncate to
25), `divisor` (default = sum of kernel if absent), `bias`, `targetX`/`targetY`
(default = floor(order/2)), `edgeMode` (default "duplicate"), `kernelUnitLength`
(number-optional-number, default 1), `preserveAlpha` (default false).

**Layer 3 (render):** emit the matching
`SVGFEConvolveMatrixEdgeMode{Duplicate|None|Wrap}` variant with `single_input_node`.

**Layer 4 (shader):**
- `render_task.rs` already packs the params (8 gpu blocks) — but **fix the kernel
  packing bug**: it currently writes only `kernel[0..=20]` (21 of 25 values).
  Extend the writer to `write_blocks(9)` and write all 25 kernel values (the last
  block should hold `kernel[21..24]` + `preserve_alpha`).
- Vertex shader: pass the extra-data address to the fragment stage (reuse the
  `vData = ivec4(gpu_buffer_uv,0,0)` trick from `FILTER_COMPONENT_TRANSFER`),
  then in the fragment stage `get_gpu_buffer_uv(vData.xy)` and
  `fetch_from_gpu_buffer_4f_direct` to read the 9 blocks into local `vec4`s.
- Fragment shader: standard 2D convolution. For each output pixel:
  ```
  sum = vec4(0); 
  for (dy, dx in 0..order): 
      weight = kernel[dy*order_x + dx]
      sample input at (pixel + (dx - target_x, dy - target_y))
      sum += weight * sample
  result = sum / divisor + bias; then premultiply alpha unless preserve_alpha
  ```
  Edge handling by variant: duplicate → `sampleInUvRect` (clamp), none →
  return transparent black outside rect, wrap → `sampleInUvRectRepeat`.
  `target_x/target_y` and `order` come from block 0; `1/divisor` and `bias` from
  block 1; `kernel_unit_length` scales the sampling step (usually 1).

---

### Task C — feTurbulence

**Layer 1 (model):** add
`FilterPrimitive::Turbulence(Turbulence)` with a struct:

```
type_: TurbulenceType,   // new enum {FractalNoise, Turbulence}
base_frequency_x: f32, base_frequency_y: f32,
num_octaves: u32, seed: u32, stitch_tiles: bool
```

**Layer 2 (parser):** parse `type` ("fractalNoise"|"turbulence", default
"turbulence"), `baseFrequency` (number-optional-number, default 0), `numOctaves`
(default 1), `seed` (default 0), `stitchTiles` (default false).

**Layer 3 (render):** pick one of the 4 `SVGFETurbulence*` variants from
(`type_`, `stitch_tiles`) with `no_input_node`.

**Layer 4 (shader):**
- `render_task.rs`: add `extra_gpu_data` packing (one block, e.g.
  `[base_frequency_x, base_frequency_y, num_octaves as f32, seed as f32]`; you
  may need a second block for stitch params).
- Vertex shader: `vFilterData0 = fetch_from_gpu_buffer_1f(aFilterExtraDataAddress)`.
- Fragment shader: implement Perlin turbulence/fractal noise. This is the
  hardest shader task — **port the standard reference implementation** (Gecko's
  `FilterInstance::GenerateNoise` or the classic Perlin by Ken Perlin). Needs:
  a hash/gradient-lattice function seeded by `seed`, octave accumulation with
  frequency doubling and amplitude halving, and (for fractal noise) summing
  absolute values vs. turbulence summing signed noise. `stitchTiles` wraps the
  lattice period to tile width/height (can be a follow-up if time-constrained,
  but mention the gap).

---

### Task D — feDiffuseLighting / feSpecularLighting

**Layer 1 (model):** add two variants
`FilterPrimitive::DiffuseLighting(DiffuseLighting)` and
`FilterPrimitive::SpecularLighting(SpecularLighting)`, with structs holding
`surface_scale`, `kernel_unit_length_x/y`, `lighting_color: [f32;4]`, and a
`LightSource` enum:

```
enum LightSource {
    Distant { azimuth: f32, elevation: f32 },
    Point { x: f32, y: f32, z: f32 },
    Spot { x, y, z, points_at_x, points_at_y, points_at_z, cone_exponent, limiting_cone_angle },
}
```

Diffuse adds `diffuse_constant`; Specular adds `specular_constant`,
`specular_exponent`.

**Layer 2 (parser):** parse `surfaceScale`, `diffuseConstant` /
`specularConstant` / `specularExponent`, `kernelUnitLength`, and `lighting-color`
(via `parse_color_rgba`). Walk children for `feDistantLight`/`fePointLight`/
`feSpotLight` (each has its own attributes: azimuth/elevation; x/y/z;
x/y/z/pointsAtX/pointsAtY/pointsAtZ/specularExponent/limitingConeAngle).

**Layer 3 (render):** emit the matching 6 variants with `single_input_node`.
`lighting-color` is applied in the shader (multiply the lighting result), not
passed as a separate gpu param — so it must be packed into `extra_gpu_data`.

**Layer 4 (shader):**
- `render_task.rs`: add `extra_gpu_data` packing for all 6 variants (currently
  empty `{}`). Pack surface_scale, constant(s), kernel_unit_length, lighting_color,
  and the light-specific params into 2–3 blocks.
- Vertex shader: fetch blocks into `vFilterData0`/`vFilterData1`.
- Fragment shader: classic lighting. For each output pixel:
  1. Sample the input 3×3, compute the **Sobel** gradient → surface normal
     `N = normalize(vec3(-gx*surfaceScale, -gy*surfaceScale, 1))`.
  2. For the light type, compute the light vector `L` (distant = constant from
     azimuth/elevation; point/spot = from light position to surface point) and
     `dot(N,L)`.
  3. Diffuse: `color = lighting_color * diffuse_constant * max(dot,0)`.
     Specular: add `specular_constant * pow(max(dot(R,V),0), specular_exponent)`.
  4. Output = `vec4(color.rgb * input_alpha, input_alpha)` (premultiplied),
     preserving the input alpha (lighting affects RGB, not A).
  Reference math: W3C filter-effects-1 §feDiffuseLighting/§feSpecularLighting.

---

### Task E — feImage (bigger than a shader fill)

The `FilterOp::SVGFEImage` is already emitted in `filter.rs` (with
`no_input_node` + identity matrix), but the shader case is `// TODO` and — per the
shader's own comment — the **uvrect is not set up in the code**, so the image is
never actually loaded.

**Solution (two sub-steps):**
1. **Image loading:** `feImage` can reference a fragment (`#id`) or an external
   URL. The filter render path must resolve that reference to a WebRender image
   and feed it into the filter graph. The `FilterOpGraphNode` has a `subregion`
   but no image-source field, so WebRender's upstream approach is: the `SVGFEImage`
   node is backed by an **image render task** whose uv rect is passed through the
   node's input machinery. Look at how the mask/clipPath path (if any) or the
   HTML `<img>` path uploads images to WebRender, and wire `feImage` to create a
   `RenderTask::new_image` + set the node's input1 to that task (with a correct
   content scale/offset) — this is the "get the uvrect set up in the code" part.
2. **Shader:** once input1 carries the image, the fragment case is just
   `result = Rs;` (identity) — the image is sampled through `sColor0` like any
   other input. The `matrix` field (a 2×3 affine) is applied by transforming the
   sampled UV.

If image loading proves to be a multi-day subsystem, implement the **shader +
  node wiring** and leave a clear `// TODO` + log for the actual fetch/upload,
  but do not silently render magenta.

---

### Task F — per-primitive subregions + `primitiveUnits`

Currently every primitive uses the **whole filter region** as its `subregion`
(one `region` value passed everywhere in `filter.rs`). The spec allows each
primitive its own `x/y/width/height` and a `primitiveUnits` attribute.

**Solution:** this is Layers 1–3 only (no shader work):
1. Model: add optional per-primitive `x/y/width/height` + `primitive_units` to
   `FilterPrimitiveNode`.
2. Parser: parse those attrs on each primitive (like the `<filter>` region attrs
   are parsed, but per-primitive).
3. Render: compute each primitive's `subregion` (in `objectBoundingBox` or
   `userSpaceOnUse` units) instead of always passing `region`.

`FilterOpGraphNode.subregion` already exists and is respected by WebRender, so
this is purely computing the right rect per node.

---

### Task G — clipPath + mask: text/image content

clipPath/mask rasterize shapes today, but **text and image children are not
included** in the mask/clip rasterization.

**Solution:** find the shared rasterization routine used by mask/clipPath
(`components/svg/render/…/mask.rs`, `…/clip_path.rs`) and make it also emit
text glyphs and `<image>` children (mirroring how the main shape path already
rasterizes `SvgTag::Text` and `SvgTag::Image` — see `node_local_bounds` in
`filter.rs` for how text/image extent is already computed). This is a rendering
change in those two files, not a filter/shader change.

---

## 5. Order of work (recommended)

1. **Task A** — verify feDisplacementMap (quick sanity that the 4-layer pattern
   is sound).
2. **Task B** — feConvolveMatrix (CPU packing mostly exists; shader is contained).
3. **Task C** — feTurbulence (self-contained, no input; hardest shader).
4. **Task D** — lighting (6 variants; moderate shader, heavier model/parser).
5. **Task E** — feImage (needs the image-loading subsystem).
6. **Task F** — per-primitive subregions (pure model/parser/render, low risk).
7. **Task G** — clipPath/mask text/image (rendering, low risk).

---

## 6. Build & verify loop (Windows, Git Bash / PowerShell)

```bash
cd /d/Projects/servo
./mach build -p servo-svg        # fast: Rust only (model/parser/render)
./mach build -p servoshell       # slow: also recompiles the webrender fork + shader
```

Render a test to PNG and inspect pixels:

```bash
timeout 60 ./target/debug/servoshell.exe -z --window-size=WxH -o /d/tmp/out.png \
  file:///D:/Projects/svg-tests/NAME.html --pref layout_servo_svg_enabled
python /d/tmp/px.py /d/tmp/out.png x,y [x,y ...]
```

**Gotchas (important):**
- The URL must be `file:///D:/abs/path` — drive letter + colon. A lowercase
  `d/tmp` path renders a broken blank page.
- `-p X --features Y` silently drops the feature — always use `./mach build`.
- `servoshell.exe` in `-z` headless mode sometimes writes the PNG but hangs on
  exit (killed by `timeout`); the PNG is still valid — check pixels, not exit code.
- Test SVGs live in `D:\Projects\svg-tests\`. `px.py` prints
  `(x,y) -> (r,g,b,a)`.

---

## 7. Cross-cutting notes

- **`linear: false` everywhere.** The helper node constructors in `filter.rs`
  hardcode `linear: false`, so the whole filter graph runs in sRGB and the
  `_CONVERTSRGB` shader paths are never hit. This is a pre-existing
  simplification, not a regression. Leave it unless you are specifically fixing
  color-interpolation correctness (that is a separate, larger change and is NOT
  part of "complete the gaps").
- **Convolve kernel packing bug** (Task B): `render_task.rs` writes only
  `kernel[0..=20]`. Fix it when implementing feConvolveMatrix.
- **Channel selector values** (feDisplacementMap): R=1, G=2, B=3, A=4 (SVG IDL
  constants); default A. Already modeled as `ColorChannel` with `to_u32()`.
- **New enums must be re-exported in both `mod.rs` files** (see Layer 1), or the
  build fails with `unresolved import`.
- Do not commit unless asked. All current work is uncommitted on branch
  `svg-vello-one-commit`.
