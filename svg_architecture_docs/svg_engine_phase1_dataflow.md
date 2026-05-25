# SVG Engine Phase 1 — Architecture & Data Flow

> **Status:** Complete (Phase 1)
> **Approach:** Direct WebRender display items from parsed SVG data, using computed CSS styles from Stylo.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Data Flow: End-to-End](#2-data-flow-end-to-end)
3. [File Map and Responsibilities](#3-file-map-and-responsibilities)
4. [Key Types](#4-key-types)
5. [Rendering Strategy](#5-rendering-strategy)
6. [How It Differs From the Old Approach](#6-how-it-differs-from-the-old-approach)
7. [Current State and Limitations](#7-current-state-and-limitations)
8. [Stylo SVG CSS Properties](#8-stylo-svg-css-properties)

---

## 1. Architecture Overview

The new SVG engine replaces the old **serialize → base64 → image cache → rasterize → bitmap** pipeline with a direct **DOM → Stylo → WebRender display items** flow. SVG shapes become native vector display items in the same display list as HTML content.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PHASE 1 ARCHITECTURE                              │
│                                                                     │
│  DOM Tree                                                    │
│  (SVGSVGElement, SVGElement for rect/circle/etc)                   │
│       │                                                             │
│       ├── Stylo computes ComputedValues per element                 │
│       │   (fill, stroke, opacity — all from CSS or attributes)      │
│       │                                                             │
│       ├── Presentational hints (element.rs)                         │
│       │   DOM attributes → CSS PropertyDeclarations                 │
│       │   50 SVG attributes mapped to CSS longhands                 │
│       │                                                             │
│       ▼                                                             │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │  Layout (replaced.rs)                                    │        │
│  │                                                          │        │
│  │  1. Detect <svg> element → svg_kind_size()              │        │
│  │  2. build_svg_scene(): walk children, build             │        │
│  │     Vec<SvgRenderInput> for each shape                   │        │
│  │  3. Store scene in ReplacedContentKind::SVGElement       │        │
│  │  4. make_fragments() → Fragment::Svg { scene }          │        │
│  └──────────────────────┬──────────────────────────────────┘        │
│                         │                                            │
│                         ▼                                            │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │  Display List Builder (display_list/mod.rs)             │        │
│  │                                                          │        │
│  │  Fragment::Svg → render_svg_element()                   │        │
│  │    → For each SvgRenderInput:                            │        │
│  │      resolve SvgLength values to pixels                  │        │
│  │      match shape type + fill/stroke params               │        │
│  │      emit WebRender display items                        │        │
│  └──────────────────────┬──────────────────────────────────┘        │
│                         │                                            │
│                         ▼                                            │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │  WebRender                                               │        │
│  │  push_rect / push_border / define_clip_rounded_rect     │        │
│  │  → GPU renders vector shapes directly                    │        │
│  └─────────────────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Data Flow: End-to-End

### 2.1 Complete Pipeline

```
HTML Parser (html5ever/xml5ever)
  │  creates SVG DOM nodes via create.rs
  ▼
DOM Tree
  │  SVGSVGElement (root)
  │  SVGElement (rect, circle, ellipse, line, path, etc.)
  │  style="..." stored in element.style_attribute
  │  width/height stored as DOM attributes
  ▼
Style Recalc (Stylo traversal)
  │  - Selector matching + cascade for each element
  │  - Presentational hints: DOM attrs → CSS PropertyDeclarations
  │  - ComputedValues::inherited_svg (fill, stroke, etc.)
  │  - ComputedValues::svg (cx, cy, r, rx, ry, x, y, d)
  ▼
Layout (replaced.rs)
  │  svg_kind_size() for <svg> element:
  │    1. Get scene data: build_svg_scene(node, context)
  │       → walk flat_tree_children
  │       → for each SVG child:
  │          a. Extract ComputedValues
  │          b. paint::extract_fill_params(style)
  │          c. paint::extract_stroke_params(style)
  │          d. paint::extract_geometry(element, tag)
  │          e. Build SvgRenderInput { tag, geometry, fill, stroke }
  │    2. Return ReplacedContentKind::SVGElement { scene }
  │
  │  make_fragments():
  │    → Fragment::Svg(Arc<SvgFragment>)
  ▼
Fragment Tree
  │  Fragment::Svg { base, scene: Arc<Vec<SvgRenderInput>> }
  ▼
Display List Building (display_list/mod.rs)
  │  Fragment::Svg → svg_engine::render::render_svg_element()
  │    for each SvgRenderInput:
  │      resolve_geometry(geom, viewport) → ResolvedGeometry
  │      match tag:
  │        Rect    → render_rect(rect, fill, stroke)
  │        Circle  → render_circle(circle, fill, stroke)
  │        Ellipse → render_ellipse(ellipse, fill, stroke)
  │        Line    → render_line(line, stroke)
  │        Path/Polyline/Polygon → skip (Phase 2)
  ▼
WebRender Display Items
  │  push_rect() / push_border() / define_clip_rounded_rect()
  │  define_clip_chain() for ring clips (circle/ellipse strokes)
  ▼
GPU → Pixels on Screen
```

### 2.2 Detailed Step-by-Step for `<rect fill="red" />`

| Step | Component | Action |
|------|-----------|--------|
| 1 | HTML Parser | Creates `SVGElement` for `<rect>`, `SVGSVGElement` for `<svg>` |
| 2 | Style Recalc | Stylo computes `ComputedValues` for the rect. `fill` from attribute becomes presentational hint → `InheritedSVG::fill = red` |
| 3 | `svg_kind_size()` | Detects SVG root via `node.as_svg().is_some()`, calls `build_svg_scene()` |
| 4 | `build_svg_scene()` | Walks children of `<svg>`, finds `<rect>`, reads its style |
| 5 | `extract_fill_params()` | Reads `style.get_inherited_svg().fill`, converts to `FillParams { color: red, opacity: 1.0 }` |
| 6 | `extract_geometry()` | Parses `x="10"`, `y="10"`, `width="180"`, `height="80"` → `ParsedGeometry::Rect { x:10, y:10, w:180, h:80, rx:0, ry:0 }` |
| 7 | Scene built | `SvgRenderInput { tag: Rect, geometry, fill: Some(FillParams { red }), stroke: None }` |
| 8 | `ReplacedContentKind` | `SVGElement { scene: Some(Arc<Vec<[...]>>) }` |
| 9 | `make_fragments()` | `Fragment::Svg(Arc::new(SvgFragment { base, scene }))` |
| 10 | `build_display_list()` | Matches `Fragment::Svg`, calls `render_svg_element()` |
| 11 | `render_rect()` | Computes clip rect, calls `wr.push_rect()` with fill color |
| 12 | WebRender | GPU renders a red rectangle at the computed position |

### 2.3 CSS Property Resolution Chain

```
DOM attribute: fill="red"
  │
  ▼
element.rs: synthesize_presentational_hints_for_legacy_attributes()
  │  parse fill="red" as CSS PropertyDeclaration::Fill
  │  push into Vec<PropertyDeclaration>
  ▼
Stylo cascade: CascadeOrigin::PresHints level
  │  style attribute < author stylesheet < pres hints < transition
  ▼
ComputedValues::inherited_svg.fill = SVGPaint { kind: Color(Color::red), ... }
  │
  ▼
paint.rs: extract_fill_params(style)
  │  style.get_inherited_svg() → &InheritedSVG
  │  svg.fill → SVGPaintKind::Color(c)
  │  c.resolve_to_absolute() → AbsoluteColor
  │  → FillParams { color: ColorF, opacity: f32 }
  ▼
render.rs: render_rect() → wr.push_rect(fill_color)
```

---

## 3. File Map and Responsibilities

### 3.1 SVG Engine Crate (`components/layout/svg_engine/`)

| File | Purpose | Key Types/Functions |
|------|---------|-------------------|
| `mod.rs` | Module declarations | Re-exports all submodules |
| `shapes.rs` | Data types for SVG shape representation | `SvgTag`, `ParsedGeometry`, `FillParams`, `StrokeParams`, `SvgRenderInput`, `SvgLineCap`, `SvgLineJoin` |
| `paint.rs` | Extract fill/stroke/geometry from Stylo `ComputedValues` and DOM attributes | `extract_fill_params()`, `extract_stroke_params()`, `extract_geometry()`, `extract_opacity()`, `resolve_svg_paint_color()` |
| `render.rs` | Dispatch each `SvgRenderInput` to WebRender display items | `render_svg_element()`, `render_rect()`, `render_circle()`, `render_ellipse()`, `render_line()`, `resolve_geometry()`, `make_shape_clip()` |
| `path.rs` | Parse SVG path `d` attribute | `parse_path_d()` → `Vec<PathCmd>` |
| `points.rs` | Parse `points` attribute for polyline/polygon | `parse_points()` → `Vec<Point2D<f64>>` |
| `lengths.rs` | Parse SVG length values | `parse_svg_length()` → `SvgLength` |
| `transform.rs` | Parse SVG `transform` attribute | `parse_transform()` → `Transform2D<f64>` |

### 3.2 Layout Integration

| File | Purpose | Key Integration Points |
|------|---------|----------------------|
| `components/layout/replaced.rs` | SVG scene building, fragment creation | `build_svg_scene()`, `svg_kind_size()`, `ReplacedContentKind::SVGElement`, `Fragment::Svg` creation |
| `components/layout/fragment_tree/fragment.rs` | Fragment type definitions | `SvgFragment` struct, `Fragment::Svg(Arc<SvgFragment>)` variant |
| `components/layout/display_list/mod.rs` | Display list building from fragments | `Fragment::Svg` dispatch → `render_svg_element()` |
| `components/layout/dom.rs` | SVG node detection | `as_svg() → Option<()>`, `NodeExt` trait |

### 3.3 Script / DOM

| File | Purpose | Key Functions |
|------|---------|---------------|
| `components/script/dom/element/element.rs` | SVG presentation hints | `synthesize_presentational_hints_for_legacy_attributes()` — 50 SVG attrs → CSS declarations |
| `components/script/dom/svg/svgelement.rs` | Base SVG element | `attribute_affects_presentational_hints()` — routes to all shape attributes |
| `components/script/dom/svg/svgsvgelement.rs` | SVG root element | `attribute_affects_presentational_hints()` for width/height |
| `components/script/dom/node/node.rs` | DOM node bridge | `svg_data() → Option<()>` (boolean marker) |

### 3.4 Shared Layout API (`components/shared/layout/`)

| File | Purpose |
|------|---------|
| `lib.rs` | Layout data types, `ReflowResult` (no longer carries `SVGElementData`) |
| `layout_node.rs` | `LayoutNode` trait with `svg_data() → Option<()>` |

### 3.5 Stylo Integration

| File | Purpose |
|------|---------|
| `stylo/style/properties/longhands.toml` | SVG CSS property definitions (42 properties enabled) |
| `stylo/style/servo/attr.rs` | `AttrValue` types, `from_declaration` for SVG attribute parsing |

---

## 4. Key Types

### 4.1 Shape Data (`shapes.rs`)

```rust
/// Identifies which SVG element type
pub enum SvgTag {
    Rect, Circle, Ellipse, Line, Polyline, Polygon, Path, Unknown,
}

/// Parsed geometry — one variant per shape type
pub enum ParsedGeometry {
    Rect     { x: SvgLength, y: SvgLength, w: SvgLength, h: SvgLength, rx: Option<SvgLength>, ry: Option<SvgLength> },
    Circle   { cx: SvgLength, cy: SvgLength, r: SvgLength },
    Ellipse  { cx: SvgLength, cy: SvgLength, rx: SvgLength, ry: SvgLength },
    Line     { x1: SvgLength, y1: SvgLength, x2: SvgLength, y2: SvgLength },
    Polyline { points: Vec<KurboPoint> },
    Polygon  { points: Vec<KurboPoint> },
    Path     { commands: BezPath },
    None,
}

/// Fill parameters extracted from Stylo's InheritedSVG
pub struct FillParams {
    pub color: ColorF,
    pub opacity: f32,
}

/// Stroke parameters extracted from Stylo's InheritedSVG
pub struct StrokeParams {
    pub color: ColorF,
    pub opacity: f32,
    pub width: f32,
    pub line_cap: SvgLineCap,
    pub line_join: SvgLineJoin,
    pub miter_limit: f32,
}

/// One element's complete render data — built in replaced.rs, consumed in render.rs
pub struct SvgRenderInput {
    pub tag: SvgTag,
    pub geometry: ParsedGeometry,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
}
```

### 4.2 Fragment Types (`fragment.rs`)

```rust
pub struct SvgFragment {
    pub base: BaseFragment,
    pub scene: Arc<Vec<SvgRenderInput>>,
}

pub enum Fragment {
    // ... existing variants ...
    Svg(#[conditional_malloc_size_of] Arc<SvgFragment>),
}
```

### 4.3 Layout Types (`replaced.rs`)

```rust
pub enum ReplacedContentKind {
    // ... existing variants ...
    SVGElement {
        #[ignore_malloc_size_of = "Arc does not implement MallocSizeOf"]
        scene: Option<Arc<Vec<SvgRenderInput>>>,
    },
}
```

### 4.4 SVG Length Types (`lengths.rs`)

```rust
pub enum SvgLength {
    Value(f32),
    Percent(f32),
}
```

---

## 5. Rendering Strategy

### 5.1 WebRender Primitive Mapping

| SVG Shape | WebRender Primitive | Clip Method |
|-----------|-------------------|-------------|
| **Rect (fill)** | `push_rect()` | None |
| **Rect (stroke)** | `push_border()` with `BorderStyle::Solid` | None |
| **Rect with rounded corners** | `define_clip_rounded_rect()` + `push_rect()` / `push_border()` | Rounded rect clip |
| **Circle (fill)** | `define_clip_rounded_rect(uniform)` + `push_rect()` | Uniform corner radii on 2r×2r rect |
| **Circle (stroke)** | Outer `Clip` + inner `ClipOut` (ring) + `push_border()` | Ring clip |
| **Ellipse (fill)** | `define_clip_rounded_rect(elliptical)` + `push_rect()` | Elliptical corner radii |
| **Ellipse (stroke)** | Outer `Clip` + inner `ClipOut` (ring) + `push_border()` | Ring clip |
| **Line** | `push_border()` rotated rect approximation | None |
| **Path** | Not implemented (Phase 2) | — |
| **Polyline/Polygon** | Not implemented (Phase 2) | — |

### 5.2 Resolved Geometry

SvgLength values are resolved against the SVG viewport size:

```rust
pub struct ResolvedGeometry {
    pub rect: LayoutRect,     // bounding box in layout pixels
    pub radii: BorderRadius,  // for rounded rect clip
}
```

### 5.3 Stroke Rendering for Circles and Ellipses

Since WebRender has no native circle/ellipse stroke, we use a **ring clip** approach:

1. Define an outer clip rounded rect (the circle's bounding box)
2. Define an inner clip-out rounded rect (slightly smaller = stroke width inside)
3. Create a clip chain combining outer Clip + inner ClipOut
4. Push a border/rect filling the annular region

---

## 6. How It Differs From the Old Approach

### Old Approach (Removed)

```
SVG DOM subtree
  │  serialize_and_cache_subtree()
  ▼
XML string → base64 data: URL
  │
  ▼
Image cache (VectorImageData)
  │
  ▼
rasterize_vector_image() → ImageKey (bitmap)
  │
  ▼
Fragment::Image { image_key }
  │
  ▼
WebRender push_image() → flat bitmap on screen
```

**Problems:**
- DOM-to-string-to-DOM round-trip (serialize → base64 → re-parse)
- All CSS computed values lost — rasterizer re-parsed XML with defaults
- Bitmap-only output — blurry at any scale beyond 1:1
- No per-element hit testing or interactivity
- Full re-serialize + re-rasterize on any change
- Cache invalidation needed manual calls in `attribute_mutated`, `children_changed`, `unbind_from_tree`

### New Approach (Phase 1)

```
SVG DOM subtree
  │  Stylo ComputedValues (CSS already applied!)
  │  extract_fill_params(), extract_stroke_params()
  │  extract_geometry()
  ▼
Vec<SvgRenderInput>
  │
  ▼
render_svg_element() — per-element dispatch
  │
  ▼
WebRender push_rect() / push_border() — vector display items
```

**Advantages:**
- No serialization — reads DOM attributes + Stylo computed values directly
- CSS inheritance works — Stylo resolves all values with full cascade
- Vector output — resolution-independent, no bitmap scaling
- Per-element display items — enables hit testing per shape
- Stateless — no cache to invalidate, re-render by calling again
- 0 warnings, 0 panics — clean integration

### Removed Code

The old approach was removed from:

| File | What Was Removed |
|------|------------------|
| `shared/layout/lib.rs` | `SVGElementData` struct, `ratio_from_view_box()`, unused imports |
| `shared/layout/layout_node.rs` | `SVGElementData` return type → `Option<()>` |
| `layout/dom.rs` | `SVGElementData` return type → `Option<()>` |
| `layout/context.rs` | `queue_svg_element_for_serialization()`, `pending_svg_elements_for_serialization` field |
| `layout/layout_impl.rs` | `pending_svg_elements_for_serialization` handling |
| `layout/replaced.rs` | `vector_image: Option<VectorImage>`, `has_viewbox: bool` from `SVGElement`, old fallback path |
| `script/dom/svg/svgsvgelement.rs` | `data()` method, `cached_serialized_data_url` field, `uuid`, `serialize_and_cache_subtree()`, `invalidate_cached_serialized_subtree()`, `process_use_elements()` |
| `script/dom/node/node.rs` | `SVGElementData` return type → `Option<()>` |
| `script/dom/window.rs` | `serialize_and_cache_subtree()` loop in reflow handler |

---

## 7. Current State and Limitations

### ✅ Phase 1 — Complete

- CSS styling for all 50 SVG presentation attributes (via Stylo + presentational hints)
- SVG `<rect>` rendering with fill and stroke (including rounded corners)
- SVG `<circle>` rendering with fill and stroke
- SVG `<ellipse>` rendering with fill and stroke
- SVG `<line>` rendering with stroke
- Fragment tree integration (`Fragment::Svg`)
- Display list building integration
- All old serialization/cache code removed
- 0 build errors, 0 warnings

### ❌ Phase 2 — Not Yet Implemented

- `<path>` rendering (requires software rasterization or tessellation)
- `<polyline>` / `<polygon>` rendering
- SVG `<g>` group element and transform support
- `viewBox` / `preserveAspectRatio` handling
- `<use>` element
- Gradient fills
- Clip paths
- SVG `<text>` rendering
- Opacity in the styling (currently opacity is extracted but not pushed as stacking context)
- `fill-rule` / `clip-rule` support (parsing exists but evenodd not wired to WR)

### Known Limitations

- Circle/ellipse strokes use a ring-clip approximation — may have edge artifacts at extreme sizes
- `fill="none"` is handled by returning `None` for fill params
- Stroke rendering pushes borders — stroke is inside the bounding box, not center-aligned
- All SVG viewport sizing defaults to 300×150 (the CSS spec default)

---

## 8. Stylo SVG CSS Properties

All 42 SVG CSS properties are enabled in Stylo's `longhands.toml`:

### Inherited (InheritedSVG struct)
`fill`, `fill-opacity`, `fill-rule`, `stroke`, `stroke-width`, `stroke-opacity`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit`, `stroke-dasharray`, `stroke-dashoffset`, `marker-start`, `marker-mid`, `marker-end`, `paint-order`, `text-anchor`, `color-interpolation`, `color-interpolation-filters`, `shape-rendering`, `clip-rule`

### Non-Inherited (SVG struct)
`cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`, `d`, `vector-effect`, `flood-color`, `flood-opacity`, `lighting-color`, `stop-color`, `stop-opacity`, `clip-path`, `mask-image`, `mask-type`, `mask-mode`, `mask-clip`, `mask-origin`, `mask-composite`, `mask-position`, `mask-repeat`, `mask-size`

### Presentation Hints (element.rs)
50 SVG presentation attributes mapped to CSS declarations via the `svg_attr!` macro: `fill`, `stroke`, `stroke-width`, `stroke-opacity`, `fill-opacity`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit`, `stroke-dasharray`, `stroke-dashoffset`, `fill-rule`, `clip-rule`, `opacity`, `visibility`, `color`, `cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`, `width`, `height`, `d`, `dx`, `dy`, `rotate`, `text-anchor`, `transform`, `transform-origin`, `vector-effect`, `flood-color`, `flood-opacity`, `lighting-color`, `stop-color`, `stop-opacity`, `clip-path`, `clip-rule`, `mask`, `marker-start`, `marker-mid`, `marker-end`, `paint-order`, `shape-rendering`, `color-interpolation`, `color-interpolation-filters`, `text-rendering`, `image-rendering`

---

## Appendix: File Change Summary

### New Files
- `components/layout/svg_engine/mod.rs`
- `components/layout/svg_engine/shapes.rs`
- `components/layout/svg_engine/paint.rs`
- `components/layout/svg_engine/render.rs`
- `components/layout/svg_engine/path.rs`
- `components/layout/svg_engine/points.rs`
- `components/layout/svg_engine/lengths.rs`
- `components/layout/svg_engine/transform.rs`

### Modified Files (New SVG Engine)
- `components/layout/replaced.rs` — scene building, SVGElement fragment creation
- `components/layout/fragment_tree/fragment.rs` — SvgFragment, Fragment::Svg variant
- `components/layout/display_list/mod.rs` — Fragment::Svg dispatch
- `components/script/dom/element/element.rs` — 50 SVG pres hints
- `components/script/dom/svg/svgelement.rs` — attribute_affects_presentational_hints

### Modified Files (Old Approach Cleanup)
- `components/shared/layout/lib.rs` — removed SVGElementData
- `components/shared/layout/layout_node.rs` — svg_data returns Option<()>
- `components/layout/dom.rs` — as_svg returns Option<()>
- `components/layout/context.rs` — removed serialization fields
- `components/layout/layout_impl.rs` — removed serialization pipeline
- `components/script/dom/node/node.rs` — simplified svg_data()
- `components/script/dom/svg/svgsvgelement.rs` — removed serialization/cache code
- `components/script/layout_dom/servo_layout_node.rs` — simplified svg_data()
- `components/script/dom/window.rs` — removed serialization loop
