# SVG Rendering Engine — Proposed Architecture

> **Status:** Draft for discussion — [Zulip thread](TODO)
> **Author:** Mohamed Mostafa (@mu-mostafa98)

---

## 1. Problem Statement

Servo currently routes SVG elements through the CSS box-model layout path. This
fundamentally mismatches how SVG works:

- **SVG is coordinate-based, not flow-based.** Elements are positioned by
  `x`, `y`, `cx`, `cy`, `d` (path data), transforms, and viewports — not by
  margins, padding, or flex/grid flows.
- **SVG has its own styling model.** Properties like `fill`, `stroke`,
  `stroke-width`, `stroke-dasharray`, and `paint-order` don't map to CSS box
  properties.
- **SVG has paint servers.** Gradients, patterns, filters, clip-paths, and
  masks are defined in `<defs>` and referenced by `url(#id)` — there is no CSS
  equivalent for this indirection.
- **SVG needs software tessellation.** Filled polygons with non-zero/even-odd
  fill-rule require triangulation that CSS layout doesn't provide.
- **SVG has its own coordinate transformations.** `viewBox`,
  `preserveAspectRatio`, and `transform="..."` create a nested coordinate
  system stack unlike CSS transforms.

**Goal:** A dedicated SVG rendering pipeline that understands SVG semantics
natively, without forcing SVG through the CSS box-model path.

---

## 2. Proposed Architecture (High-Level)

The architecture has four main layers, with the **SVG Engine** internally split
into two sub-layers: **Script** → **Integration Layer** → **SVG Engine** (Traversal → Render) → **WebRender**.

```
┌──────────────────────────────────────┐
│  Script                              │
│  DOM SVG elements (already: #46558)  │
└────────────────┬─────────────────────┘
                 │ ServoLayoutNode
                 ▼
┌──────────────────────────────────────┐
│  Integration Layer                   │
│  components/layout/svg/              │
│  DOM → Render Tree                   │
│  Produces: Arc<SvgRenderTree>        │
└────────────────┬─────────────────────┘
                 │ pure data (no DOM refs)
                 ▼
┌──────────────────────────────────────┐
│  SVG Engine                          │
│  components/svg_engine/              │
│  ┌────────────────────────────────┐  │
│  │ Traversal                      │  │
│  │ walk tree, apply transforms,   │  │
│  │ clips, masks, filters, opacity │  │
│  │ → dispatch to Render           │  │
│  └───────────────┬────────────────┘  │
│                  │                   │
│  ┌───────────────▼────────────────┐  │
│  │ Render                         │  │
│  │ per-shape Render trait impls   │  │
│  │ fill, stroke, gradient, tess.  │  │
│  │ → push_rect, push_line, etc.   │  │
│  └────────────────────────────────┘  │
└────────────────┬─────────────────────┘
                 │ WebRender commands
                 ▼
┌──────────────────────────────────────┐
│  WebRender                           │
│  GPU compositing & rendering         │
└──────────────────────────────────────┘
```

### Layer Details

**Script Layer** — the input: DOM SVG element types that feed into the pipeline.
- `SVGRectElement`, `SVGCircleElement`, `SVGPathElement`, etc.
- Container elements: `<g>`, `<defs>`, `<use>`, `<symbol>`
- Text elements: `<text>`, `<tspan>`
- Already created in [#46558](https://github.com/servo/servo/pull/46558).

**Integration Layer** (`components/layout/svg/`) — translates Servo DOM/style into
the engine's pure-data model. Feature-gated behind `svg-engine`.
- Walks the DOM subtree and resolves computed styles, geometry, transforms, and
  `<defs>` into a flat `SvgRenderTree`.
- No DOM references remain in the output — the tree is pure data.

**SVG Engine** (`components/svg_engine/`) — pure rendering; zero dependencies on
Servo's DOM, layout, or style crates. Internally split into two sub-layers:

- **Traversal**: walk the render tree, manage coordinate system state, then call the
    render layer.
  - Steps:
    1. Skip nodes with `display: none`
    2. Apply per-node transforms (translate, scale, rotate, skew, matrix) by
       pushing WebRender reference frames
    3. Resolve clip-path, mask, and filter from the definition maps and push
       corresponding WebRender clips and stacking contexts
    4. Call the render layer for the current node
    5. Recurse into children (skipping `<defs>` and `<symbol>` — rendered only
       via `<use>`)
    6. Pop transform reference frames

- **Render**: called by traversal; dispatch to the correct per-shape renderer and
    emit WebRender display list commands.
  - Steps:
    1. Dispatch by shape variant (Rect, Circle, Ellipse, Line, Polyline, Polygon,
       Path) to the matching renderer
    2. Apply fill (solid color, gradient, or pattern)
    3. Apply stroke (width, dasharray, linecap, linejoin, miterlimit) respecting
       paint-order
    4. Tessellate filled polygons (triangulation + scanline rasterization) for
       non-zero/even-odd fill-rule
    5. Emit WebRender commands (`push_rect`, `push_line`, etc.)

**WebRender** — existing GPU rendering pipeline. Receives standard
`DisplayListBuilder` commands with no SVG-specific awareness.

---

## 3. Data Flow

Rendering an SVG element happens in two phases, triggered at different points
in the layout pipeline.

```
1. Layout encounters <svg> element
   │
   ▼
2. Build (Integration Layer)
   build_svg_render_tree()
   DOM → SvgRenderTree (pure data)
   tree stored in fragment as Arc<SvgRenderTree>
   │
   ▼
3. Display list construction
   display_list/mod.rs calls render_svg_tree()
   │
   ▼
4. Render (SVG Engine)
   ├─ Traversal: walk tree, push transform/clip/filter/opacity
   └─ Render: dispatch to per-shape impls → push_rect, push_line, …
   │
   ▼
5. WebRender composites & draws
```

### Step 1 — Trigger: layout encounters `<svg>`

When the layout system processes a replaced element that is an SVG, it calls
`build_svg_render_tree()` from `components/layout/replaced.rs`. This triggers
the build phase.

### Step 2 — Build: DOM → SvgRenderTree

**Entry point:** `layout::svg::build_svg_render_tree(node, context)`

A single pass over the DOM subtree (see Integration Layer in Section 2).

**Output:** `Arc<SvgRenderTree>` — stored in the layout fragment. No DOM
references remain; the tree is pure data.

### Step 3 — Trigger: display list construction

During display list building, `components/layout/display_list/mod.rs` retrieves
the `Arc<SvgRenderTree>` from the fragment and calls `render_svg_tree()`.

### Step 4 — Render: SvgRenderTree → WebRender commands

**Entry point:** `svg_engine::render_svg_tree(tree, origin, size, spatial_id,
clip_chain_id, wr)`

Traversal walks the tree and calls the render layer (see SVG Engine sub-layers
in Section 2). Shapes are dispatched to per-shape renderers; fill, stroke, and
tessellation produce WebRender display list commands.

**Output:** WebRender `DisplayListBuilder` commands.

### Step 5 — WebRender composites

WebRender receives standard display list primitives (rects, clips, stacking
contexts) with no SVG-specific awareness and renders them on the GPU.

---

## 4. Key Data Models

All types described below live in `components/svg_engine/` with no dependencies
on Servo's DOM, layout, or style crates.

### 4.1 SvgRenderTree — the top-level render tree

The `SvgRenderTree` is what the Integration Layer produces and the SVG Engine
consumes. It contains:

- **root** — the root `SvgRenderNode` of the tree.
- **viewport** — viewport information: width, height, optional `viewBox`,
  `preserveAspectRatio`, and `overflow: visible` flag.
- **gradients** — map of gradient definitions (linear/radial) keyed by ID,
  collected from `<linearGradient>` and `<radialGradient>` in `<defs>`.
- **clip_paths** — map of clip-path definitions keyed by ID, collected from
  `<clipPath>`. Each entry holds the clipping shapes and the coordinate system
  (`objectBoundingBox` or `userSpaceOnUse`).
- **patterns** — map of pattern definitions keyed by ID, collected from
  `<pattern>`. Each entry holds the tile dimensions, coordinate systems, and
  the tile's content shapes with their styles.
- **masks** — map of mask definitions keyed by ID, collected from `<mask>`.
  Each entry holds the mask content as styled shapes.
- **filters** — map of filter definitions keyed by ID, collected from
  `<filter>`. Each entry holds an ordered list of filter primitives and the
  filter bounds.

The tree itself serves as the provider for all paint/clip/filter resources
during rendering — lookup is a flat hash-map lookup, no DOM walks.

### 4.2 SvgRenderNode — a single node in the tree

Each node represents one SVG element in the render tree:

- **id** — the element's `id` attribute, used for `url(#id)` references from
  other elements (e.g., `<use href="#myShape">`).
- **tag** — what kind of node this is: a `Shape`, `Text` span, `Image`, or
  `Container`.
- **style** — resolved paint-level styling (`NodeStyle`, see below).
- **transforms** — ordered list of SVG transform operations (translate, scale,
  rotate, skewX, skewY, matrix). These are structural — they affect the
  coordinate system, not just paint — so they live on the node rather than
  inside `NodeStyle`.
- **children** — child nodes, forming the tree structure.

### 4.3 SvgTag — what kind of element a node is

Four variants:

- **Shape(Shape)** — a geometric primitive (rect, circle, ellipse, line,
  polyline, polygon, path).
- **Text(TextSpan)** — a `<text>` or `<tspan>` span with shaped glyphs,
  positioning, and text-anchor.
- **Image(SvgImage)** — an `<image>` element with position, size, and href.
- **Container(Container)** — a grouping element: `Group` (`<g>`), `Svg`
  (nested `<svg>`), `Defs` (`<defs>` — children not rendered directly),
  `Use` (`<use>` — references another element by ID), or `Symbol`
  (`<symbol>` — reusable viewBox'd container).

### 4.4 Shape — geometric primitives (pure data)

Seven geometry variants, each as a simple data struct:

| Shape | Fields |
|-------|--------|
| `Rectangle` | x, y, width, height, rx, ry (corner radii) |
| `Circle` | cx, cy, r |
| `Ellipse` | cx, cy, rx, ry |
| `Line` | x1, y1, x2, y2 |
| `Polyline` | points (list of (x,y)) |
| `Polygon` | points (list of (x,y)) |
| `Path` | BezPath (Bezier path data from the `d` attribute) |

Shapes carry no rendering logic — they are pure geometric data constructed by
the Integration Layer from DOM element attributes.

### 4.5 NodeStyle — paint-level styling

Resolved per-node style produced by the Integration Layer from CSS computed
values and SVG presentation attributes:

- **visibility** — `visible` or `hidden`.
- **display** — `inline`, `block`, or `none`.
- **fill** — optional fill parameters: base color, paint server reference
  (solid color / gradient ID / pattern ID), opacity, and fill-rule
  (`nonzero` or `evenodd`).
- **stroke** — optional stroke parameters: base color, paint server reference,
  opacity, width, linecap (`butt` / `round` / `square`), linejoin
  (`miter` / `round` / `bevel`), miter limit, dash array, and dash offset.
- **render_hints** — quality/behavior hints: `shape-rendering`,
  `color-interpolation` (`sRGB` / `linearRGB`), `paint-order`
  (`normal` / `stroke-then-fill`), and `vector-effect`
  (`non-scaling-stroke`).
- **effects** — references to clip-path, mask, and filter definitions
  (each is an ID string like `"url(#myFilter)"`).
- **opacity** — element-level opacity (CSS `opacity` property), applied as a
  multiplier on top of fill/stroke opacity.

Transform-related properties are NOT in `NodeStyle` — they live directly on
`SvgRenderNode` because they affect the coordinate system structurally.

### 4.6 Definition types — paint servers, clip, mask, and filter

**GradientDef** — either a `LinearGradient` or `RadialGradient`:
- Gradient units (`objectBoundingBox` or `userSpaceOnUse`), spread method
  (`pad` / `reflect` / `repeat`), gradient transform, and an ordered list of
  color stops (color + offset).
- Linear gradients have x1, y1, x2, y2 endpoints.
- Radial gradients have cx, cy, r (and optional fx, fy focal point).

**PatternDef** — tile-based paint server:
- Tile dimensions (width, height, x, y), separate coordinate systems for tile
  sizing (`patternUnits`) and content (`patternContentUnits`), and the list of
  styled shapes that form the tile content.

**ClipPathDef** — clipping region:
- A list of Shapes defining the clip area, plus the coordinate system
  (`objectBoundingBox` or `userSpaceOnUse`).

**MaskDef** — alpha mask:
- A list of styled shapes whose luminance defines the mask alpha channel.

**FilterDef** — ordered list of filter primitives applied in sequence:
- `GaussianBlur` (std deviation x/y), `DropShadow` (dx, dy, blur, color),
  `ColorMatrix` (20-value matrix or saturate), `LuminanceToAlpha`, `Offset`
  (dx, dy), `Flood` (solid RGBA), `Composite` (arithmetic or
  Porter-Duff operators), `Tile`, and `Image` (external URL or fragment
  reference).
- Plus filter bounds (x, y, width, height) that may extend beyond the
  element's bounding box (e.g., for drop-shadow).

### 4.7 TransformOp — SVG transform operations

Ordered list of transform operations applied to a node. Matches the SVG
`transform` attribute model:

- `Translate(tx, ty)` — 2D translation.
- `Scale(sx, sy)` — 2D scale.
- `Rotate(angle, cx, cy)` — rotation around an optional center point.
- `SkewX(angle)` / `SkewY(angle)` — skew transforms.
- `Matrix(a, b, c, d, e, f)` — arbitrary 2D affine matrix.

These are structural transforms that affect the coordinate system. CSS
transforms (from Stylo computed values) are also converted into this same
representation by the Integration Layer.

### 4.8 Text and Image types

**TextSpan** — a single `<text>` or `<tspan>` element:
- Text content, x/y positioning, per-character dx/dy offsets, pre-shaped
  glyphs (from the font subsystem), text-anchor (`start` / `middle` / `end`),
  and a WebRender `FontInstanceKey` for glyph rendering.

**SvgImage** — an `<image>` element:
- x, y, width, height, and the image href (URL or data URI).

---
