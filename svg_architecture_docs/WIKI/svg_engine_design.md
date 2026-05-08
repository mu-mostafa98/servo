# SVG Engine — Design Proposal

> A native SVG rendering engine that replaces the current serialize-as-image approach. Keeps the SVG subtree as part of the document tree, applies the full CSS cascade, and produces fragment trees / display lists directly.

---

## Current Architecture (Problem)

```
SVG DOM subtree → XML serialize → base64 → data URL → Image Cache → usvg parse → resvg rasterize → ImageKey → Fragment::Image
```

Information is lost at every step:
- **Serialization**: computed styles, scripting context, resource references
- **usvg parse**: `<foreignObject>`, animations, external resources stripped
- **resvg rasterize**: fixed resolution, no integration with WebRender font/rendering system
- **Image cache**: treats SVG as a static bitmap, no animation or interaction

## Proposed Architecture

```
Styled SVG subtree ─→ SVG Engine ─→ Fragment Tree ─→ Display List ─→ WebRender
                          │
                    (keeps subtree
                     in document tree)
```

The SVG engine receives a fully styled DOM subtree and outputs a fragment tree that feeds into Servo's existing display list pipeline — no serialization, no image cache indirection.

---

## Architectural Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    SVG Engine                                     │
│                                                                   │
│  Input:                                                           │
│    Styled SVG DOM subtree                                         │
│    (Arc<ComputedValues> on each element,                          │
│     resolved by Stylo cascade)                                    │
│         │                                                         │
│         ▼                                                         │
│  ┌─────────────┐                                                  │
│  │ Style       │  Apply parent-inherited CSS properties            │
│  │ Resolver    │  Resolve SVG-specific properties (fill, stroke,   │
│  │             │  mask, clip-path, filter, marker, etc.)           │
│  └──────┬──────┘                                                  │
│         ▼                                                         │
│  ┌─────────────┐                                                  │
│  │ Layout      │  Compute geometry from SVG attributes + CSS       │
│  │ Engine      │  Handle viewBox, preserveAspectRatio,             │
│  │             │  intrinsic sizing, nested viewports               │
│  └──────┬──────┘                                                  │
│         ▼                                                         │
│  ┌─────────────┐                                                  │
│  │ Paint       │  Convert styled + laid-out tree into a            │
│  │ Builder     │  flat sequence of paint commands (fills,          │
│  │             │  strokes, masks, filters, clips)                  │
│  └──────┬──────┘                                                  │
│         ▼                                                         │
│  Output:                                                          │
│    Fragment Tree / Paint Commands                                  │
│    (consumed by Servo's display list builder)                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Sub-component Design

### 1. Style Resolver

**Responsibility:** Take a DOM subtree where Stylo has already computed base styles, and resolve SVG-specific presentation attributes and cascaded properties.

**Input:**
- SVG DOM subtree with `Arc<ComputedValues>` on each node (from Stylo)
- Inherited parent styles (fill, stroke, opacity, etc. cascade into SVG)

**Key SVG properties to resolve:**

| Property | Source | Notes |
|----------|--------|-------|
| `fill` | CSS / presentation attribute | Can be paint server URL (`url(#gradient)`) |
| `stroke` | CSS / presentation attribute | Same — can reference gradients/patterns |
| `stroke-width`, `stroke-linecap`, etc. | CSS / presentation attribute | Vector rendering parameters |
| `opacity`, `fill-opacity`, `stroke-opacity` | CSS | Compositing |
| `clip-path`, `mask` | CSS | Can reference SVG elements |
| `filter` | CSS | SVG filter primitives |
| `marker-*` | CSS | Marker rendering at path vertices |
| `visibility`, `pointer-events` | CSS | Hit-testing |
| `font-*` | CSS | Text rendering (must use Servo's font system) |

**Challenge — paint servers:** SVG allows `fill="url(#myGradient)"` to reference a `<linearGradient>` defined elsewhere in the SVG. The style resolver must track these references across the subtree and defer paint server resolution until layout produces the bounding box that gradients depend on.

**Architecture options:**

A) **Stylo extension** — Add SVG property support directly in Stylo (Blink approach). Requires upstream changes to the CSS engine but gives the most accurate cascade.

B) **SVG-specific style layer** — A thin resolver that runs after Stylo and translates SVG presentation attributes into an internal style representation. Lighter integration but may diverge from CSS spec behavior.

### 2. Layout Engine

**Responsibility:** Given a styled SVG element tree, compute the geometry (bounding boxes, viewport transforms) of every element.

**Input:** Styled element tree from the Style Resolver.

**Key layout operations:**

```
svg_kind_size() ──→ viewBox transform ──→ viewport clipping ──→ child layout
                                                        │
                                                        ▼
                                               nested <svg> / <symbol>
                                               (recursive viewport)
```

**Core concepts:**

- **Viewport** — The region of the SVG canvas defined by `width`, `height`, and `viewBox`. The `preserveAspectRatio` attribute controls how content maps into the viewport.
- **ViewBox transform** — The affine transform that maps the SVG coordinate system to the viewport pixel space. Includes `translate`/`scale` from `viewBox` alignment, plus any CSS `transform` on the `<svg>` element.
- **Bounding boxes** — Each element has a logical bounding box computed from its geometry (path length, text extents, child bounding boxes). These are needed for filter regions, clip paths, and paint server gradient bounds.
- **Nested viewports** — `<svg>` inside an SVG creates a new viewport. `<symbol>` acts as a viewport template instantiated by `<use>`.

**Coordinate system layers:**

```
Screen space  (CSS pixels of the document)
      ↑
CSS transform  (transform, scale, rotate on <svg>)
      ↑
SVG viewport   (viewBox + preserveAspectRatio → affine transform)
      ↑
Element space  (local coordinates: cx, cy, r, d, x, y, etc.)
```

**Output:** A tree of laid-out SVG elements, each with:
- Local bounding box (in element coordinate space)
- Viewport-adjusted bounding box (in screen pixel space)
- Computed affine transform stack (element → screen)

### 3. Paint Builder

**Responsibility:** Walk the laid-out SVG tree and produce a flat, ordered sequence of paint commands that can be converted into Servo display list items.

**Input:** Laid-out SVG element tree.

**Output:** Flat list of paint commands:

```rust
enum PaintCommand {
    Fill {
        path: PathData,
        paint: PaintServer,     // color, gradient, pattern
        fill_rule: FillRule,    // nonzero / evenodd
        opacity: f32,
        transform: AffineTransform,
    },
    Stroke {
        path: PathData,
        paint: PaintServer,
        stroke: StrokeParams,   // width, cap, join, miter, dash
        opacity: f32,
        transform: AffineTransform,
    },
    ClipPath {
        id: ClipId,
        paths: Vec<PathData>,
        fill_rule: FillRule,
    },
    Mask {
        id: MaskId,
        content: Vec<PaintCommand>,
    },
    Filter {
        id: FilterId,
        primitives: Vec<FilterPrimitive>,
        input: Box<PaintCommand>,
    },
    Image {
        data: ImageData,        // for <image> element
        rect: Rect,
    },
    Text {
        runs: Vec<TextRun>,     // glyphs + font references
        fill: PaintServer,
        stroke: Option<StrokeParams>,
        transform: AffineTransform,
    },
    Group {
        children: Vec<PaintCommand>,
        transform: AffineTransform,
        opacity: f32,
    },
}
```

**Mapping to Servo display list:**

The display list builder already supports `push_image()`, `push_text()`, `push_clip()`, `push_gradient()`, etc. The SVG engine's paint commands map 1:1 or composably to these:

| SVG Paint Command | Servo Display Item |
|-------------------|-------------------|
| `Fill { path, color }` | `push_rect()` or `push_mesh()` with colored fill |
| `Fill { path, gradient }` | `push_gradient()` with clip mask of the path |
| `Text { runs }` | `push_text()` with font + glyphs |
| `Image { data }` | `push_image()` with decoded image |
| `Group { opacity }` | `push_stacking_context()` with opacity |
| `ClipPath` | `push_clip() / pop_clip()` |

**Challenge — filters:** SVG filters (`<feGaussianBlur>`, `<feDropShadow>`, `<feColorMatrix>`, etc.) require offscreen rendering passes. The engine must allocate temporary render targets, apply filter primitives, and composite the result. WebRender's filter support can handle some cases natively; complex filter graphs may need intermediate surfaces.

### 4. Animation Engine

**Responsibility:** Drive SVG animations (SMIL: `<animate>`, `<set>`, `<animateTransform>`, `<animateMotion>`) and CSS animations on SVG properties.

**Integration with Servo's rendering update:**

```
update_the_rendering() ──→ SVG Animation Engine ──→ update element state
                                        │
                                        ▼
                              needs_rendering_update() = true
                                        │
                                        ▼
                              reflow() → repaint()
```

**Key components:**

- **Animation clock** — Synchronized with Servo's refresh driver / rAF timer. Each tick advances the active animation timeline.
- **Interpolation engine** — Resolves keyTimes, keySplines, calcMode (discrete/linear/spline/paced) for each animating property.
- **Override layer** — Temporarily overrides the base style / attribute value of animated properties. Restored when the animation ends.
- **Event dispatch** — Fires `beginEvent`, `endEvent`, `repeatEvent` for SMIL animations.

**Supported animation types (priority order):**

| Priority | Type | Examples | Complexity |
|----------|------|----------|------------|
| P0 | CSS transitions/animations | `transition: fill 0.3s` | Uses existing Servo animation infrastructure |
| P1 | `<animate>` (scalar) | radius, x, y, opacity | Linear interpolation of numeric attributes |
| P2 | `<animateTransform>` | rotation, scaling, translation | Matrix interpolation/decomposition |
| P3 | `<animateMotion>` | path following | Path interpolation + rotation |
| P4 | `<set>` | instant attribute changes | Trivial — no interpolation |
| P5 | `keyTimes`/`keySplines` | custom easing curves | Bezier curve evaluation |

---

## Fragment Tree Integration

The SVG engine's output is consumed by Servo's existing fragment tree and display list pipeline. The integration point is in `make_fragments()` (currently in `layout/replaced.rs`).

### Current flow:

```rust
// svg_kind_size() → source=Some(url) → cache lookup
// serialize-as-image path:
make_fragments()
  → rasterize_vector_image() → RasterImage
  → Fragment::Image { image_key }
```

### Proposed flow:

```rust
// svg_kind_size() → source=Some(url) → svg_engine.process()
// native SVG engine path:
make_fragments()
  → svg_engine.process(styled_subtree, viewport_size)
    → Vec<PaintCommand>
  → Fragment::Svg {
        children: display_items,  // converted from PaintCommands
        viewport: rect,
        animating: bool,          // true if animation is running
    }
```

The new `Fragment::Svg` variant carries paint commands directly rather than an image key. This means:
- No rasterization step — vector commands go to WebRender directly
- Resolution-independent — CSS transforms don't cause pixelation
- Animations re-execute paint commands each frame without re-rasterizing

---

## Styling Integration

**Key requirement:** CSS properties from parent elements must cascade into the SVG subtree, and SVG-specific properties must be resolved correctly.

**Parent → SVG inheritance:**

```
<div style="fill: green">
  <svg>              ← inherits fill: green from parent
    <text>           ← inherits fill: green inside SVG
  </svg>
</div>
```

**Current state:** Stylo already computes styles on SVG elements. The computed `fill` value is available in `Arc<ComputedValues>`. What's missing:

1. SVG presentation attributes (`fill="green"`) take lowest priority in the cascade (below UA rules). Stylo must be taught about them, or the SVG engine must merge them into the cascade manually.
2. Some SVG properties (`stroke-dasharray`, `marker-*`, `color-interpolation-filters`) don't exist in Stylo's property set yet.

**Approach — minimal Stylo extension:**

Add SVG property definitions to Stylo's property database (following Servo's existing pattern for CSS properties). Stylo's parallel engine computes them naturally, and SVG presentation attributes are converted to inline style rules.

---

## `<foreignObject>` Integration

`<foreignObject>` allows HTML content inside SVG. This is the hardest case because it requires compositing HTML layout output inside an SVG viewport.

**Approach:**

1. When the SVG engine encounters `<foreignObject>`, it extracts the HTML subtree and passes it to Servo's existing layout infrastructure as a child fragment.
2. The layout output (fragment tree) is clipped to the `<foreignObject>`'s bounding box and composited into the SVG paint command stream at the correct position.
3. The HTML fragment inherits the CSS cascade from the parent document (not the SVG), matching spec behavior.

```rust
// In the paint command stream:
PaintCommand::ForeignObject {
    rect: Rect,                               // position in SVG viewport
    html_fragment: Box<FragmentTree>,         // from Servo's layout
    clip_to_rect: bool,
}
```

**Challenge — coordinate systems:** The HTML fragment is laid out in CSS pixel space, but the SVG may be scaled/rotated by `viewBox` + `transform`. The `<foreignObject>` content must be rasterized at the SVG's effective resolution, which requires either:
- Render-to-texture at the scaled resolution (expensive for animated transforms)
- Keep HTML fragment vector and let WebRender composite (requires WebRender to understand nested document spaces)

---

## Modularization (Taffy-like Approach)

Per the Zulip discussion: implement the SVG engine as a separate library that takes a fully styled input tree and returns an output tree convertible to fragment trees / display lists.

```
┌──────────────┐    Styled tree     ┌──────────────┐    Paint commands   ┌──────────────┐
│  Stylo       │ ────────────────→  │  svg-engine   │ ────────────────→  │  Servo       │
│  (CSS)       │                    │  (library)    │                    │  (layout)    │
└──────────────┘                    └──────────────┘                    └──────────────┘
```

**Library interface:**

```rust
// svg-engine crate (public API)

pub struct SvgEngine {
    styles: StyleRegistry,     // resolved style data
    fonts: FontContext,        // font loading + shaping
    animations: AnimationClock,
}

impl SvgEngine {
    /// Process a styled SVG subtree for a single frame.
    /// `root` is the SVG DOM node with computed styles attached.
    /// `viewport` is the CSS pixel size of the SVG element.
    pub fn process(
        &mut self,
        root: &StyledSvgNode,
        viewport: Size2D<Au>,
        tick: AnimationTick,
    ) -> SvgOutput;
}

pub struct SvgOutput {
    pub paint_commands: Vec<PaintCommand>,
    pub is_animating: bool,           // needs re-render next frame
    pub viewport_rect: Rect<Au>,
    pub intrinsic_size: Option<Size2D<Au>>,
}

pub struct StyledSvgNode {
    pub element: SvgElementType,      // rect, circle, path, text, etc.
    pub styles: Arc<ComputedValues>,  // from Stylo
    pub children: Vec<StyledSvgNode>,
}
```

**Benefits of library approach:**

| Concern | Current (serialize) | Proposed (library) |
|---------|-------------------|-------------------|
| Testing | Requires full Servo browser | Unit-testable with mock style data |
| Reuse | Tied to Servo's image cache | Usable by other Rust projects |
| CI speed | Full browser build | Compile only `svg-engine` + deps |
| Experimentation | Must modify Servo internals | Iterate in isolation |
| Fuzzing | Hard to isolate SVG parsing | Direct XML+style fuzzing |

---

## Implementation Roadmap

### Phase 1 — Basic shapes + styling (minimal viable engine)

- Style resolver for core SVG properties (fill, stroke, opacity, transform)
- Path rendering for `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>`
- Layout: viewBox, preserveAspectRatio, intrinsic sizing
- Paint: solid fills and strokes → `Fragment::Svg` in make_fragments()
- Replace the serialize-as-image path for simple cases

### Phase 2 — Text + gradients + paint servers

- `<text>` element with proper font integration (use Servo's font system)
- Linear/radial gradients and pattern fills
- Markers (`<marker>` on path vertices)
- Opacity and group compositing

### Phase 3 — Filters, clipping, masking

- `<clipPath>` and `<mask>` elements
- SVG filter primitives (blur, color matrix, drop shadow, composite)
- Offscreen render surface management

### Phase 4 — Animation

- CSS transitions on SVG properties
- SMIL `<animate>` and `<animateTransform>`
- Animation clock integration with Servo's refresh driver
- Dirty → re-render loop for animated content

### Phase 5 — Advanced features

- `<foreignObject>` — HTML compositing inside SVG
- `<use>` element with shadow DOM semantics
- `<switch>` with `requiredFeatures` evaluation
- Color-interpolation, writing-mode, direction
- Scripting support (JS event handlers inside SVG)

---

## Risks and Open Questions

1. **Performance of vector display lists** — For SVGs with thousands of elements, a `Fragment::Svg` with individual paint commands may produce a much larger display list than a single `push_image()`. Need tiling / flattening strategies.

2. **Filter rendering** — WebRender supports a subset of SVG filters natively. Complex filter DAGs may require intermediate surfaces, which are expensive. Need a fallback strategy.

3. **`<foreignObject>` resolution** — Compositing HTML inside a transformed SVG viewport requires either render-to-texture (loses resolution on scale) or nested document spaces in WebRender (not currently supported).

4. **Animation timing** — SMIL has a complex timing model (indefinite, syncbase, event-based start times). A full implementation is a significant effort on its own.

5. **Scripting security** — SVG `<script>` elements and event handlers need the same security model as HTML scripts. CSP, document origin, sandboxing must apply.

6. **Library boundary — how much style data crosses the boundary?** If the SVG engine is a separate library, it needs access to computed style values. The exact API surface for `StyledSvgNode.styles` needs careful design to avoid tight coupling with Stylo internals.

---

## Related Documents

- [SVG Known Limitations](svg_known_limitations.md) — Cases that fail under the current serialize-as-image approach
- [SVG Rendering Pipeline (Current)](svg_rendering_pipeline.md) — Full walkthrough of the current pipeline
- [SVG Rendering Overview](svg_rendering_overview.md) — High-level summary of the current 6-stage pipeline
