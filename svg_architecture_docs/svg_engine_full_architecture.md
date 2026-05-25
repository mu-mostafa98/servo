# SVG Engine — Full Architecture & Roadmap

> **Vision:** Replace Servo's old serialize→rasterize→bitmap SVG pipeline with a native vector rendering engine that pushes WebRender display items directly from DOM + Stylo computed styles.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Complete Data Flow](#3-complete-data-flow)
4. [Component Architecture](#4-component-architecture)
5. [Phase Plan and Roadmap](#5-phase-plan-and-roadmap)
6. [Phase Details](#6-phase-details)
7. [Key Design Decisions](#7-key-design-decisions)
8. [Current Status](#8-current-status)

---

## 1. Executive Summary

### Problem

Servo's old SVG pipeline serialized the entire SVG DOM subtree to an XML string, base64-encoded it into a data URL, pushed it through the image cache, rasterized it to a bitmap, and displayed that bitmap as a flat image. This meant:

- **All CSS computed values were lost** — the rasterizer re-parsed XML from scratch
- **Bitmap-only output** — blurry at any scale beyond 1:1
- **No per-element interactivity** — the entire SVG was one flat image
- **Full re-render on any change** — attribute change on one `<rect>` re-serialized the entire subtree
- **Manual cache invalidation** — fragile, easy to miss cases

### Solution

Build a native SVG rendering engine that:

1. Reads **DOM attributes + Stylo computed values** directly (no serialization)
2. Pushes **vector WebRender display items** (resolution-independent)
3. Renders **per-element display items** (hit testing, animation-ready)
4. Is **stateless** (no cache to invalidate, re-render by calling again)

### Current Status

| Metric | Value |
|--------|-------|
| **Phase 1** | ✅ Complete |
| **Build status** | 0 errors, 0 warnings |
| **SVG CSS properties enabled** | 42 (all) |
| **SVG presentation attributes** | 50 (all) |
| **Basic shapes rendering** | Rect, Circle, Ellipse, Line |
| **Old serialization code** | ✅ Removed |

---

## 2. Architecture Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                           SVG ENGINE ARCHITECTURE                         │
│                                                                          │
│   DOM                                                                   │
│   ┌─────────────────────────────┐                                        │
│   │  SVGSVGElement              │                                        │
│   │    ├── SVGElement (rect)    │   Attributes: x, y, width, fill, etc. │
│   │    ├── SVGElement (circle)  │   Style: style="fill: red"             │
│   │    ├── SVGElement (g)       │   Children: flat_tree_children()       │
│   │    └── SVGElement (use)     │                                        │
│   └──────────┬──────────────────┘                                        │
│              │                                                           │
│              │ Stylo: ComputedValues per element                         │
│              │ Presentational hints: attrs → CSS declarations            │
│              ▼                                                           │
│   ┌────────────────────────────────────────────────────────────────┐     │
│   │                INTEGRATION LAYER (replaced.rs)                 │     │
│   │                                                                │     │
│   │  ┌──────────────────────────────────────────────────────────┐  │     │
│   │  │ build_svg_scene(): walk DOM, extract style + geometry    │  │     │
│   │  │   • extract_fill_params(style)   → FillParams            │  │     │
│   │  │   • extract_stroke_params(style) → StrokeParams          │  │     │
│   │  │   • extract_geometry(element)    → ParsedGeometry         │  │     │
│   │  │   • collect_defs()              → defs map               │  │     │
│   │  │   • resolve_references()        → resolve url(#id)       │  │     │
│   │  │   • compute_viewbox()           → viewBox transform      │  │     │
│   │  │   • handle_groups()             → push/pop transforms    │  │     │
│   │  │   Output: Vec<SvgRenderInput>                             │  │     │
│   │  └──────────────────────────┬───────────────────────────────┘  │     │
│   └─────────────────────────────┼──────────────────────────────────┘     │
│                                 │                                        │
│                                 ▼                                        │
│   ┌────────────────────────────────────────────────────────────────┐     │
│   │                    SVG ENGINE (svg_engine/)                     │     │
│   │                                                                │     │
│   │  render_svg_element(inputs, viewport, wr)                      │     │
│   │    for each SvgRenderInput:                                    │     │
│   │      resolve_geometry(geom, viewport) → ResolvedGeometry       │     │
│   │      match tag:                                                │     │
│   │        Rect    → render_rect()                                 │     │
│   │        Circle  → render_circle()                               │     │
│   │        Ellipse → render_ellipse()                              │     │
│   │        Line    → render_line()                                 │     │
│   │        Path    → render_path()         [Phase 2]              │     │
│   │        Poly*   → render_polygon()      [Phase 2]              │     │
│   │        Use     → resolve + render      [Phase 3]              │     │
│   │        G       → push_transform()      [Phase 3]              │     │
│   │        Text    → render_text()         [Phase 4]              │     │
│   │      apply clipping if clip_path       [Phase 5]              │     │
│   │      push gradient fill if gradient    [Phase 6]              │     │
│   │      push stacking context if opacity  [Phase 3]              │     │
│   └──────────────────────┬─────────────────────────────────────────┘     │
│                          │                                               │
│                          ▼                                               │
│   ┌────────────────────────────────────────────────────────────────┐     │
│   │               WEBRENDER DISPLAY ITEMS                          │     │
│   │                                                                │     │
│   │  push_rect()          — filled rectangles                      │     │
│   │  push_border()        — stroked rectangles / borders           │     │
│   │  define_clip_rounded_rect() — circle/ellipse/rounded rect clip │     │
│   │  define_clip_chain()  — combined clip regions (ring clips)     │     │
│   │  push_gradient()      — gradient fills            [Phase 6]   │     │
│   │  push_text()          — text rendering             [Phase 4]   │     │
│   │  push_reference_frame() — transforms / viewBox   [Phase 3]    │     │
│   │  push_stacking_context() — opacity / filters     [Phase 3]     │     │
│   │  push_image()         — fallback rasterization   [Phase 2]    │     │
│   └──────────────────────────┬─────────────────────────────────────┘     │
│                              │                                           │
│                              ▼                                           │
│   ┌────────────────────────────────────────────────────────────────┐     │
│   │  GPU — vector rendering, resolution-independent                 │     │
│   └────────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Complete Data Flow

### 3.1 End-to-End Pipeline

```
┌─────────────┐
│  HTML/XML   │   html5ever/xml5ever parser
│  Source     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  DOM Tree   │   create.rs dispatches SVG namespace → SVGElement subtypes
│             │   Attributes stored as AttrValue, style="" parsed to PDB
└──────┬──────┘
       │
       ├───────────────────────────────────────────────────┐
       │                                                   │
       ▼                                                   ▼
┌──────────────────────┐                     ┌──────────────────────────┐
│  Style Recalc (Stylo) │                     │  Presentational Hints    │
│                      │                     │  (element.rs)            │
│  • Selector matching  │                     │                          │
│  • Cascade (8 levels) │                     │  SVG attrs → CSS decls  │
│  • Value computation  │                     │  via parse_declared()   │
│  • Animation values   │                     │  50 attributes mapped   │
└──────┬───────────────┘                     └──────────┬───────────────┘
       │                                                │
       └──────────┬─────────────────────────────────────┘
                  │
                  ▼
┌──────────────────────────────────────────────────────┐
│  ComputedValues per element                          │
│                                                      │
│  • inherited_svg: Arc<InheritedSVG>                  │
│    — fill, fill_opacity, stroke, stroke_width, ...   │
│  • svg: Arc<SVG>                                     │
│    — cx, cy, r, rx, ry, x, y, d, ...                │
│  • box_, inherited_box, font, inherited_text, ...    │
└──────────────────┬───────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────┐
│  Layout — svg_kind_size() in replaced.rs              │
│                                                      │
│  node.as_svg().is_some() → detect SVG root           │
│    │                                                  │
│    ▼                                                  │
│  build_svg_scene(node, context) → Vec<SvgRenderInput> │
│    │                                                  │
│    │  for child in node.flat_tree_children():         │
│    │    read child's ComputedValues                   │
│    │    extract_fill_params(style)                    │
│    │    extract_stroke_params(style)                  │
│    │    extract_geometry(element, tag)                │
│    │    → SvgRenderInput { tag, geometry, fill,       │
│    │                       stroke }                   │
│    │                                                  │
│    ▼                                                  │
│  ReplacedContentKind::SVGElement { scene }           │
│    │                                                  │
│    ▼                                                  │
│  Fragment::Svg(Arc<SvgFragment>)                     │
└──────────────────┬───────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────┐
│  Display List Builder (display_list/mod.rs)           │
│                                                      │
│  Fragment::Svg → render_svg_element(scene, ...)      │
│    for each SvgRenderInput:                          │
│      resolve_geometry(geom, viewport) → f32 coords   │
│      match tag + fill/stroke:                        │
│        → push_rect / push_border / define_clip       │
└──────────────────┬───────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────┐
│  WebRender → GPU → Pixels on Screen                  │
└──────────────────────────────────────────────────────┘
```

### 3.2 Data Types Through the Pipeline

```
┌──────────────────────┬─────────────────────────┬──────────────────────────┐
│      Stage           │     Input Type           │     Output Type          │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ HTML Parser          │ Source bytes            │ DOM tree (SVGElement     │
│                      │                         │  nodes)                  │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ Style Recalc (Stylo)  │ DOM tree + stylesheets  │ ElementData w/          │
│                      │                         │  ComputedValues per node │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ Presentational Hints │ DOM attributes          │ PropertyDeclarations     │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ Scene Building       │ ComputedValues + DOM     │ Vec<SvgRenderInput>     │
│                      │  attrs                  │                          │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ Fragment Creation    │ Vec<SvgRenderInput>     │ Fragment::Svg            │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ Display List         │ Fragment::Svg           │ WebRender DL items       │
│                      │                         │  (push_rect, etc.)      │
├──────────────────────┼─────────────────────────┼──────────────────────────┤
│ WebRender            │ DisplayList             │ GPU commands             │
└──────────────────────┴─────────────────────────┴──────────────────────────┘
```

### 3.3 CSS Property Resolution Detail

```
DOM attribute: fill="red"
  │
  ▼
element.rs: synthesize_presentational_hints_for_legacy_attributes()
  │  svg_attr!("fill") → PropertyDeclaration::Fill(val)
  │  push into Vec<PropertyDeclaration>
  ▼
Stylo cascade at CascadeOrigin::PresHints level
  │  User Agent < User < Author < PresHints < StyleAttr < Animations < Transitions
  ▼
ComputedValues.inherited_svg.fill
  │  = SVGPaintKind::Color(AbsoluteColor::red)
  ▼
paint.rs: extract_fill_params(style)
  │  style.get_inherited_svg().fill → SVGPaintKind::Color(c)
  │  resolve_to_absolute() → sRGB ColorF
  ▼
FillParams { color: ColorF, opacity: 1.0 }
  │
  ▼
render.rs: render_rect()
  │  wr.push_rect(&common, fill_color)
  ▼
WebRender: GPU draws red rectangle
```

---

## 4. Component Architecture

### 4.1 Crate Dependency Graph

```
servo-svg-engine (separate crate at components/svg_engine/)
├── src/
│   ├── lib.rs                       ← Crate root: private modules, pub re-exports
│   ├── shapes.rs                    ← Data types (SvgTag, ParsedGeometry, etc.)
│   ├── paint.rs                     ← Stylo → plain struct extraction
│   ├── render.rs                    ← WebRender display item generation
│   ├── path.rs                      ← Path "d" attribute parser
│   ├── points.rs                    ← "points" attribute parser
│   ├── lengths.rs                   ← SVG length value parser
│   └── transform.rs                 ← Transform attribute parser
│
│   Dependencies: stylo, webrender_api, kurbo, euclid, app_units

servo-layout (crate at components/layout/)
├── depends on: servo-svg-engine
│
├── replaced.rs                      ← Scene building, fragment creation
├── fragment_tree/
│   └── fragment.rs                  ← SvgFragment, Fragment::Svg variant
├── display_list/
│   └── mod.rs                       ← Fragment::Svg → render dispatch
│
├── dom.rs                           ← SVG node detection (as_svg)
└── context.rs                       ← ImageResolver (image loading)

script (crate at components/script/)
├── dom/element/element.rs           ← Presentational hints (50 SVG attrs)
├── dom/svg/svgelement.rs            ← SVG element base
├── dom/svg/svgsvgelement.rs         ← SVG root element
├── dom/node/node.rs                 ← svg_data() marker
└── layout_dom/servo_layout_node.rs  ← LayoutNode trait impl

shared/layout (crate at components/shared/layout/)
├── lib.rs                           ← ReflowResult (cleaned)
└── layout_node.rs                   ← LayoutNode trait

stylo/style (external crate)
└── properties/longhands.toml        ← 42 SVG CSS property definitions
```

### 4.2 Key Layer Boundaries

```
Layer                │ Knows about              │ Does NOT know about
─────────────────────┼──────────────────────────┼─────────────────────────────
svg_engine crate     │ ParsedGeometry,          │ Stylo types, DOM, fragment
                     │ FillParams, WebRender    │ tree, layout system
                     │                         │
paint.rs             │ ComputedValues (stylo),  │ DOM, fragments, display
                     │  ColorF, SVG types       │ list building
                     │                         │
replaced.rs          │ DOM nodes, ComputedValues│ WebRender details,
                     │  Fragment types,         │ display item optimization
                     │  svg_engine::*           │
                     │                         │
fragment.rs          │ Fragment tree types      │ SVG parsing, WebRender
                     │                         │
display_list/mod.rs  │ Fragment tree, WebRender │ SVG parsing, DOM
                     │  DisplayListBuilder,     │
                     │  svg_engine::*           │
```

---

## 5. Phase Plan and Roadmap

```
Phase 1 ─── Phase 2 ─── Phase 3 ─── Phase 4 ─── Phase 5 ─── Phase 6
  │           │           │           │           │           │
  │           │           │           │           │           │
  ▼           ▼           ▼           ▼           ▼           ▼
BASIC       COMPLEX     STRUCTURE   TEXT        CLIPPING    GRADIENTS
SHAPES      SHAPES      & VIEWBOX              & MASKS     & FILTERS

  ◄─────── DONE ───────► ◄────────────────── PLANNED ──────────────────►
```

| Phase | Focus | Value |
|-------|-------|-------|
| **1** | Basic shapes + CSS styling | Replaces old serialize→bitmap with vector rects, circles, ellipses |
| **2** | Path, polyline, polygon | Complete shape coverage |
| **3** | Transforms, viewBox, `<g>`, groups | Nested SVG structure, correct coordinate systems |
| **4** | SVG text | `<text>`, `<tspan>`, web fonts, text-anchor |
| **5** | Clip paths and masks | `<clipPath>`, `<mask>` support |
| **6** | Gradients and filters | `<linearGradient>`, `<radialGradient>`, `<filter>` |

### Total Estimated Scope

| Metric | Phase 1 | Phase 2 | Phase 3 | Phase 4 | Phase 5 | Phase 6 |
|--------|---------|---------|---------|---------|---------|---------|
| New files | 8 | 0 | 1 | 1 | 1 | 1 |
| Files modified | ~15 | ~3 | ~5 | ~3 | ~3 | ~3 |
| Lines of code | ~800 | ~300 | ~400 | ~500 | ~300 | ~400 |
| SVG features | 4 shapes | 3 shapes | 5 features | 3 features | 2 features | 3 features |
| WR primitives | 3 | 1 | 2 | 1 | 1 | 2 |

---

## 6. Phase Details

### Phase 1 — Basic Shapes & CSS Styling ✅ (Complete)

**Deliverable:** SVG `<rect>`, `<circle>`, `<ellipse>`, `<line>` render with CSS fill and stroke colors via vector WebRender display items. Old serialize→rasterize pipeline removed.

**What was built:**

| Component | Details |
|-----------|---------|
| SVG CSS properties | 42 properties enabled in Stylo `longhands.toml` with `servo_restyle_damage = "repaint"` |
| Presentation hints | 50 SVG attributes → CSS declarations in `element.rs` using `svg_attr!` macro |
| Parser modules | `lengths.rs` (SvgLength parsing), `path.rs` (path d attribute), `points.rs` (points attribute), `transform.rs` (transform attribute) |
| Shape data types | `SvgTag` enum, `ParsedGeometry` enum (Rect/Circle/Ellipse/Line/Path/Polyline/Polygon/None), `FillParams`, `StrokeParams`, `SvgRenderInput` |
| Style extraction | `paint.rs`: `extract_fill_params()`, `extract_stroke_params()`, `extract_geometry()` from Stylo `ComputedValues` |
| WebRender rendering | `render.rs`: `render_rect()`, `render_circle()`, `render_ellipse()`, `render_line()` using `push_rect`, `push_border`, `define_clip_rounded_rect` |
| Fragment integration | `SvgFragment` struct, `Fragment::Svg(Arc<SvgFragment>)` variant, display list dispatch |
| Old code removed | `SVGElementData`, `cached_serialized_data_url`, `serialize_and_cache_subtree()`, `queue_svg_element_for_serialization()`, and all related cache/image pipeline code |

**Files created:** 8 (`src/lib.rs`, `shapes.rs`, `paint.rs`, `render.rs`, `path.rs`, `points.rs`, `lengths.rs`, `transform.rs`) + `Cargo.toml`

**Files modified:** ~15 across `layout/`, `script/`, and `shared/layout/`

**Build:** 0 errors, 0 warnings

**Test:** `svg_test.html` renders rects, circles, ellipses, lines with fill and stroke colors

**Rendering strategy:**

| SVG Shape | WebRender Primitive |
|-----------|-------------------|
| Rect (fill) | `push_rect()` |
| Rect (stroke) | `push_border()` with `BorderStyle::Solid` |
| Rect with rounded corners | `define_clip_rounded_rect()` + `push_rect()` / `push_border()` |
| Circle (fill) | `define_clip_rounded_rect(uniform r)` + `push_rect()` |
| Circle (stroke) | Ring clip (outer Clip + inner ClipOut) + `push_border()` |
| Ellipse (fill) | `define_clip_rounded_rect(elliptical rx, ry)` + `push_rect()` |
| Ellipse (stroke) | Ring clip + `push_border()` |
| Line | `push_border()` rotated rect approximation |

---

### Phase 2 — Complex Shapes (Path, Polyline, Polygon) 🔜

**Deliverable:** `<path>`, `<polyline>`, `<polygon>` elements render with fill and stroke. Paths use software rasterization via `resvg` or `tiny_skia` as a fallback since WebRender has no bezier path primitive.

**Key challenge:** WebRender has no native bezier path/curve API. Options:

| Option | Approach | Pro | Con |
|--------|----------|-----|-----|
| **A. Rasterize** | Render path to offscreen buffer via `tiny_skia`, push as `push_image()` | Simple, works for any path | Loses vector quality |
| **B. Clip-mask** | Push path as clip region, fill with color rect | Resolution-independent | WebRender may not support complex clip paths |
| **C. Tessellate** | Flatten bezier to triangles | Vector approach | WebRender has no triangle mesh API either |

**Recommendation:** Option A (rasterize via tiny_skia) for MVP. Can optimize later.

**Files to create/modify:**
- `src/render.rs` — add `render_path()`, `render_polyline()`, `render_polygon()`
- `src/raster.rs` (new) — software rasterization fallback
- `src/shapes.rs` — wire up Path/Polyline/Polygon in render dispatch

**Estimated effort:** ~300 LOC

---

### Phase 3 — Structure, Transforms, and viewBox 🔜

**Deliverable:** Support for `<g>` (group) elements, `transform` attribute, `viewBox` + `preserveAspectRatio`, `<use>` references, and opacity stacking contexts.

**Features:**

| Feature | Description | WebRender API |
|---------|-------------|---------------|
| `transform="translate(x,y)"` | Per-element affine transform | `push_reference_frame()` |
| `<g transform="...">` | Group transform — push/pop stack | Push/pop reference frame |
| `viewBox` + `preserveAspectRatio` | Coordinate system mapping | Initial reference frame on SVG root |
| `opacity="0.5"` | Per-element opacity | `push_stacking_context(opacity)` |
| `<use href="#id">` | Reference and render target in-place | Render target via same path |
| Defs collection | Collect `<defs>`, `<linearGradient>`, `<clipPath>` etc. before rendering | HashMap<String, Def> |

**The transform stack model:**

```
SVG viewport (from layout)
  ↓  viewBox + preserveAspectRatio (initial transform)
  ↓  <g transform="translate(10, 10)">
  ↓    <rect transform="rotate(45)" ... />
  ↓  </g>
  ↓
WebRender device pixels
```

Each transform pushes a `push_reference_frame()` / pops with `pop_reference_frame()`.

**Files to create/modify:**
- `src/context.rs` (new) — `SvgContext` with transform stack, clip stack, defs map
- `src/render.rs` — transform dispatch, `<g>` handling
- `src/transform.rs` — transform attribute parsing (already exists as parser)
- `replaced.rs` — viewBox parsing, defs collection
- `src/shapes.rs` — add transform field to `SvgRenderInput`

**Estimated effort:** ~400 LOC

---

### Phase 4 — SVG Text 🔜

**Deliverable:** `<text>` and `<tspan>` elements rendered with correct positioning, `text-anchor`, `letter-spacing`, `word-spacing`, using Servo's font system for glyph shaping and WebRender's `push_text()` for rendering.

**Key components:**

| Component | Responsibility |
|-----------|---------------|
| Font selection | Use Stylo's computed `font-family`, `font-size`, `font-weight`, `font-style` |
| Text shaping | Use Servo's `FontContext` to shape text → `Vec<GlyphInstance>` + `FontInstanceKey` |
| Positioning | `x`, `y` attributes, `text-anchor` (start/middle/end), `letter-spacing`, `word-spacing` |
| WebRender output | `wr.push_text()` with glyphs, font key, color, offset |

**Complexity factors:**
- Bidirectional text
- `direction` and `unicode-bidi` CSS properties
- Inline `<tspan>` with different styles
- White-space handling per SVG spec (`xml:space`)
- Text selection and accessibility

**Files to create/modify:**
- `src/text.rs` (new) — text positioning engine
- `src/render.rs` — `render_text()` dispatch
- `paint.rs` — `extract_text_params()` from Stylo
- `replaced.rs` — text node traversal
- `src/shapes.rs` — `TextParams` struct

**Estimated effort:** ~500 LOC

---

### Phase 5 — Clip Paths and Masks 🔜

**Deliverable:** `<clipPath>` elements define reusable clip regions. Elements reference them via `clip-path="url(#id)"`. WebRender clip chains handle the clipping natively.

**How it works:**

```
DOM tree
  │
  ▼
collect_defs(): walk DOM before rendering
  │  Find all <clipPath>, <mask> elements
  │  Parse their child geometry
  │  Store in HashMap<String, ClipDef>
  ▼
During rendering:
  element with clip-path="url(#c1)"
    │  resolve url(#c1) → ClipDef from defs map
    │  build WebRender clip chain from ClipDef geometry
    ▼
  wr.define_clip_chain() → ClipChainId
  include in CommonItemProperties for the element
```

**WebRender clip primitives:**

| Clip shape | WebRender API |
|-----------|---------------|
| Rect | `ClipId::clip_rect()` |
| Rounded rect | `ComplexClipRegion` with radii |
| Arbitrary path | Not supported natively — needs software rasterization |

**Files to create/modify:**
- `src/clip.rs` (new) — clip chain building from geometry
- `src/shapes.rs` — `ClipDef` type, clip path field on `SvgRenderInput`
- `replaced.rs` — `collect_defs()` for clip paths
- `paint.rs` — `resolve_clip_path()` from Stylo

**Estimated effort:** ~300 LOC

---

### Phase 6 — Gradients and Filters 🔜

**Deliverable:** `<linearGradient>` and `<radialGradient>` fills via WebRender's native `push_gradient()`. Basic SVG filters via `push_stacking_context_with_filters()`.

**Gradient rendering:**

```
fill="url(#g1)" where <linearGradient id="g1">
  │
  ▼
Stylo resolves fill to SVGPaintKind::Reference("g1")
  │
  ▼
Integration resolves "g1" from defs map → GradientDef
  │  Parse <stop> elements, gradientUnits, gradientTransform
  ▼
Engine pushes gradient:
  wr.push_gradient(rect, start, end, stops, extend_mode)
```

**Filter support:**
- WebRender has `push_stacking_context_with_filters()` for basic filters
- SVG filters (`<filter>`, `<feGaussianBlur>`, `<feOffset>`, etc.) map to WebRender filter types
- Complex filters (custom `feComponentTransfer`, `feColorMatrix`) may require software fallback

**Files to create/modify:**
- `src/gradient.rs` (new) — gradient data extraction + WebRender gradient items
- `src/filter.rs` (new) — SVG filters → WebRender filter types
- `src/paint.rs` — `resolve_paint_server()` for gradient fills
- `src/shapes.rs` — `GradientParams` type
- `replaced.rs` — gradient def collection

**Estimated effort:** ~400 LOC

---

## 7. Key Design Decisions

### 7.1 Why a Separate Crate?

The SVG engine lives as a standalone crate at `components/svg_engine/` (package: `servo-svg-engine`, lib: `svg_engine`). This follows the same pattern as other rendering subsystems in Servo (`canvas/`, `paint/`, `webgl/`, `webgpu/`).

Benefits:
- **Clean API boundary** — `lib.rs` re-exports only the public surface (`SvgRenderInput`, `render_svg_element()`, etc.); internal modules are private
- **Faster incremental compilation** — changing svg_engine code doesn't recompile the full layout crate
- **Discoverable** — `components/svg_engine/` is immediately visible at the workspace level
- **Enforced encapsulation** — no accidental access to layout internals from the engine

The crate depends only on external workspace crates (`stylo`, `webrender_api`, `kurbo`, `euclid`, `app_units`), with zero circular dependency on the `layout` crate.

### 7.2 Stateless Engine

The engine holds **no mutable state between calls**. Every `render_svg_element()` call is fully self-contained:

```rust
pub fn render_svg_element(
    scene: &[SvgRenderInput],
    viewport_bounds: LayoutRect,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut wr::DisplayListBuilder,
);
```

This means:
- **Re-rendering is free** — just call again with new values
- **No dirty tracking** — integration decides what needs re-render
- **No cache invalidation** — nothing to invalidate
- **Animation-ready** — Stylo updates values → engine renders with new values

### 7.3 Where Stylo Ends and the Engine Begins

```
Stylo responsibility (outside engine):
  • CSS selector matching and cascade
  • SVG attribute → CSS declaration conversion (presentational hints)
  • Value computation (specified → computed)
  • Animation value resolution
  • Font selection and text shaping

Engine responsibility (inside engine):
  • Geometry parsing from DOM attribute strings
  • Fill/stroke parameter extraction from computed values
  • Coordinate resolution (percentage → absolute)
  • WebRender display item generation
  • Clip chain construction
  • Transform stacking
```

### 7.4 Rectangle and Rounded Rectangle Strategy

WebRender has good support for rounded rectangles via `ComplexClipRegion`, so where possible we approximate circles as uniform-radius rounded rectangles and ellipses as elliptical-radius rounded rectangles. This keeps rendering purely vector with no software fallback needed.

### 7.5 Error Handling Philosophy

- **Parse errors:** Log a warning, skip the element, continue rendering the rest
- **Unsupported features:** Render nothing for that element, don't crash the page
- **Missing references** (`url(#missing)`): Silently ignored (per SVG spec)
- **The engine never panics** from invalid SVG input

---

## 8. Current Status

### 8.1 Implementation Status Matrix

| Feature | Phase | Status | Notes |
|---------|-------|--------|-------|
| CSS fill (color) | 1 | ✅ | Via `extract_fill_params()` |
| CSS stroke (color) | 1 | ✅ | Via `extract_stroke_params()` |
| CSS opacity | 1 | ⚠️ | Extracted but stacking context not pushed |
| `<rect>` | 1 | ✅ | Fill, stroke, rounded corners |
| `<circle>` | 1 | ✅ | Fill, stroke via rounded-rect clip |
| `<ellipse>` | 1 | ✅ | Fill, stroke via elliptical clip |
| `<line>` | 1 | ✅ | Stroke via border |
| `<path>` | 2 | ❌ | Needs software rasterization |
| `<polyline>` | 2 | ❌ | Same as path |
| `<polygon>` | 2 | ❌ | Same as path |
| `transform` attribute | 3 | ❌ | Needs reference frame support |
| `<g>` element | 3 | ❌ | Needs transform stack |
| `viewBox` / `preserveAspectRatio` | 3 | ❌ | Needs coordinate system mapping |
| `<use>` element | 3 | ❌ | Needs reference resolution |
| Opacity stacking | 3 | ❌ | Needs `push_stacking_context()` |
| Defs collection | 3 | ❌ | Pre-render pass for references |
| `<text>` / `<tspan>` | 4 | ❌ | Needs text shaping + positioning |
| `<clipPath>` | 5 | ❌ | Needs clip chain integration |
| `<mask>` | 5 | ❌ | Needs mask API |
| `<linearGradient>` | 6 | ❌ | Needs `push_gradient()` |
| `<radialGradient>` | 6 | ❌ | Needs `push_radial_gradient()` |
| `<filter>` | 6 | ❌ | Needs filter stack |

### 8.2 File Inventory

```
components/svg_engine/                     (9 files, ~800 LOC)
├── Cargo.toml                             Workspace crate manifest
└── src/
    ├── lib.rs                             Crate root: private modules, pub re-exports
    ├── shapes.rs                          SvgTag, ParsedGeometry, FillParams,
    │                                      StrokeParams, SvgRenderInput
    ├── paint.rs                           Style extraction functions
    ├── render.rs                          WebRender display item generation
    ├── path.rs                            Path "d" attribute parser
    ├── points.rs                          Points attribute parser
    ├── lengths.rs                         Length value parser
    └── transform.rs                       Transform attribute parser

Integration (layout/)                     (~200 LOC added to existing files)
├── replaced.rs                            Scene building, SVGElement fragment
├── fragment_tree/fragment.rs              SvgFragment, Fragment::Svg variant
└── display_list/mod.rs                    Fragment::Svg dispatch

Integration (script/)                     (~850 LOC added to existing file)
└── dom/element/element.rs                50 SVG presentation hints

Stylo (external)                          (~50 lines configuration)
└── style/properties/longhands.toml       42 SVG CSS properties
```

### 8.3 Files No Longer Needed (Removed or Cleaned)

The old serialize→rasterize pipeline was removed from these files:

| File | What Changed |
|------|--------------|
| `shared/layout/lib.rs` | Removed `SVGElementData` struct |
| `shared/layout/layout_node.rs` | `svg_data()` returns `Option<()>` |
| `layout/dom.rs` | `as_svg()` returns `Option<()>` |
| `layout/context.rs` | Removed serialization queue and fields |
| `layout/layout_impl.rs` | Removed serialization data passing |
| `script/dom/node/node.rs` | `svg_data()` simplified to marker |
| `script/dom/svg/svgsvgelement.rs` | Removed `data()`, serialization, cache, UUID, use-element processing |
| `script/dom/window.rs` | Removed serialization loop |
| `script/layout_dom/servo_layout_node.rs` | `svg_data()` simplified |
