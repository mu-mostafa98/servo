# SVG Engine — Architecture & Implementation Guide

> **Version:** 1.0  
> **Date:** July 2026  
> **Audience:** Engineering team, technical leads, management  
> **Status:** Active development — see [Known Limitations](#appendix-known-limitations-and-roadmap)

---

## 1. Executive Summary

The SVG Engine is a new, purpose-built rendering pipeline for Servo that converts SVG documents directly into WebRender display list commands, bypassing the legacy image-rasterization path. It adds approximately 8,500 lines of Rust across 98 files, spanning three architectural layers:

| Layer | Lines | Key Files |
|-------|-------|-----------|
| **Script/DOM** (~1,600) | 18 SVG element types + WebIDL bindings | `components/script/dom/svg/` |
| **Layout Integration** (~1,400) | Style resolution, definition collection, tree assembly | `components/layout/svg/` |
| **SVG Engine** (~5,200) | Shapes, styles, rendering pipeline, tessellation | `components/svg_engine/src/` |

The engine supports the full SVG basic shapes, gradients (linear & radial), patterns, clipping, masking, filters (blur, drop-shadow), transforms, dashed strokes, and opacity — all rendered natively through WebRender without intermediate rasterization.

---

## 2. Architecture Overview

### 2.1 Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     LAYER 1: DOM / SCRIPT                     │
│  components/script/dom/svg/                                   │
│                                                               │
│  SVGElement ← SVGGraphicsElement ← SVGGeometryElement         │
│    ├── SVGCircleElement         ├── SVGGElement               │
│    ├── SVGEllipseElement        ├── SVGDefsElement            │
│    ├── SVGRectElement           ├── SVGSymbolElement          │
│    ├── SVGLineElement           ├── SVGUseElement             │
│    ├── SVGPathElement           ├── SVGSVGElement             │
│    ├── SVGPolygonElement        └── SVGImageElement           │
│    └── SVGPolylineElement                                    │
│                                                               │
│  SVGElement ──→ synthesizes presentational hints (attributes  │
│                  → CSS property declarations)                  │
└──────────────────────────┬────────────────────────────────────┘
                           │ SVGElementData
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                  LAYER 2: LAYOUT INTEGRATION                    │
│  components/layout/svg/                                        │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   style.rs   │  │  collects.rs │  │  builder.rs  │        │
│  │              │  │              │  │              │        │
│  │ CSS rules    │  │ Gradients    │  │ Tree assembly│        │
│  │ ComputedVals │  │ ClipPaths    │  │ <use> resolve│        │
│  │ Presentation │  │ Patterns     │  │ Paint fixup  │        │
│  │ attributes   │  │ Masks        │  │              │        │
│  │              │  │ Filters      │  │              │        │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘        │
│         │                 │                  │                 │
│         └─────────────────┴──────────────────┘                 │
│                           │ SvgRenderTree                      │
└───────────────────────────┬────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                  LAYER 3: SVG ENGINE                            │
│  components/svg_engine/src/                                    │
│                                                               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │  shapes/ │  │  style/  │  │render_   │  │renderer/ │     │
│  │          │  │          │  │tree.rs   │  │          │     │
│  │ Rectangle│  │ Fill     │  │ Render   │  │fill.rs   │     │
│  │ Circle   │  │ Stroke   │  │ Tree     │  │stroke.rs │     │
│  │ Ellipse  │  │ Gradient │  │ Node     │  │gradient  │     │
│  │ Line etc.│  │ Transfrm │  │ Tree     │  │pattern   │     │
│  │          │  │          │  │ Walking  │  │transform │     │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘     │
│                                                               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                    │
│  │tessellator│ │ effects/ │  │ visitor/ │                    │
│  │          │  │          │  │          │                    │
│  │ Triangln │  │ clip.rs  │  │ Paint    │                    │
│  │ Scanline │  │ filter.rs│  │ Fixup    │                    │
│  │ RLE merge│  │          │  │          │                    │
│  └──────────┘  └──────────┘  └──────────┘                    │
│                                                               │
│  Output: WebRender DisplayListBuilder commands                 │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

```
SVG HTML/XML
     │
     ▼
DOM Tree (script::dom::svg)
     │  SVGElement stores presentational hints
     │  SVGSVGElement provides SVGElementData (viewport info, svg_id)
     ▼
Layout Tree (layout)
     │  Fragment tree with SvgRenderTree attached
     │  svg::build_svg_render_tree() called
     ▼
SvgRenderTree (svg_engine::render_tree)
     │  Tree of SvgRenderNode + definition maps
     │  Gradients, clip paths, patterns, masks, filters
     ▼
Render Traversal (svg_engine::traversal)
     │  transform → clip → mask → filter → shape
     │  Recursive walk, WebRender frame push/pop
     ▼
Shape Renderers (svg_engine::renderer)
     │  fill_rect / fill_polygon / stroke_rect / stroke_polyline
     │  Gradient/pattern/color dispatch
     ▼
WebRender DisplayList
     │  push_rect, push_border, push_reference_frame, etc.
     ▼
GPU Composition
```

---

## 3. Detailed Component Breakdown

### 3.1 Script/DOM Layer (`components/script/dom/svg/`)

**Purpose:** Define the DOM representation of SVG elements per the SVG 2 specification, binding them to the Servo script engine.

**Inheritance Hierarchy:**
```
Node → Element → SVGElement
  ├── SVGGraphicsElement (abstract — drawable elements)
  │   ├── SVGGeometryElement (abstract — shapes with geometry)
  │   │   ├── SVGCircleElement
  │   │   ├── SVGEllipseElement
  │   │   ├── SVGRectElement
  │   │   ├── SVGLineElement
  │   │   ├── SVGPathElement
  │   │   ├── SVGPolygonElement
  │   │   └── SVGPolylineElement
  │   ├── SVGGElement
  │   ├── SVGDefsElement
  │   ├── SVGSymbolElement
  │   ├── SVGUseElement
  │   └── SVGSVGElement
  ├── SVGGradientElement (abstract — paint servers)
  │   ├── SVGLinearGradientElement
  │   └── SVGRadialGradientElement
  └── SVGStopElement
```

**Key Responsibilities:**
- **`SVGElement::synthesize_presentational_hints()`** ([svgelement.rs](components/script/dom/svg/svgelement.rs)) — Converts SVG attributes (fill, stroke, opacity, etc.) into CSS property declarations for Servo's style system
- **`SVGSVGElement`** ([svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs)) — Provides `SVGElementData` to the layout layer (viewBox, width, height, svg_id) and DOM lifecycle hooks (`attribute_mutated`, `children_changed`, `unbind_from_tree`) that trigger re-layout when the SVG subtree changes.  
  *(Note: Subtree serialization and `<use>`-expansion via DOM cloning are legacy pipeline features — they are NOT used by the SVG engine path. The engine resolves `<use>` references directly in [`builder.rs`](components/layout/svg/builder.rs) via `find_element_by_id`.)*
- **WebIDL bindings** ([script_bindings/webidls/](components/script_bindings/webidls/)) — 18 SVG interface definitions with spec-accurate inheritance

**Design Notes:**
- All concrete element types follow a uniform pattern via the `#[dom_struct]` macro
- Element creation is dispatched through `create_svg_element()` in [create.rs](components/script/dom/element/create.rs)
- DOM lifecycle hooks are routed through the `VirtualMethods` trait in [virtualmethods.rs](components/script/dom/node/virtualmethods.rs)

---

### 3.2 Layout Integration Layer (`components/layout/svg/`)

**Purpose:** Bridge between Servo's style system and the SVG engine by constructing an `SvgRenderTree` from DOM data.

The layer is organized as a Rust module with four files:

#### 3.2.1 `mod.rs` — Public API
The module entry point. Re-exports `build_svg_render_tree()`, which is called from [replaced.rs](components/layout/replaced.rs) during layout construction.

#### 3.2.2 `style.rs` — Style Construction (~700 lines)
Connects Servo's computed CSS values and SVG presentation attributes to the engine's `NodeStyle` type.

**Key Functions:**
| Function | Purpose |
|----------|---------|
| `build_style()` | Main style constructor — merges computed values → presentation attributes → CSS class rules |
| `FromComputedValues` trait | Converts `ComputedValues` to `FillParams`/`StrokeParams`/`NodeStyle` |
| `apply_stroke_presentation_attrs()` | Field-level merge of stroke attributes (dasharray, linecap, etc.) into existing style |
| `apply_fill_presentation_attrs()` | Field-level merge of fill attributes (fill, fill-opacity, fill-rule) |
| `build_style_from_attrs()` | Standalone style builder for `<pattern>`/`<mask>` children (no computed style available) |
| `collect_svg_css_rules()` | Extracts CSS class rules from inline `<style>` elements in SVG namespace |
| `apply_css_property()` | Maps CSS property strings to `NodeStyle` fields |

#### 3.2.3 `collects.rs` — Definition Collection (~380 lines)
Extracts definitions from `<defs>` containers using the **Strategy** design pattern.

**Key Types:**
| Type | Pattern | Purpose |
|------|---------|---------|
| `DefinitionParser` trait | Strategy | Defines how to parse a definition type from DOM |
| `DefinitionCollector` | Strategy Context | Generic collection loop — walks `<defs>`, finds elements by tag, calls parser |
| `GradientParser` | Concrete Strategy | Parses `<linearGradient>` and `<radialGradient>` |
| `ClipPathParser` | Concrete Strategy | Parses `<clipPath>` with clipPathUnits |
| `PatternParser` | Concrete Strategy | Parses `<pattern>` with units, dimensions, content |
| `MaskParser` | Concrete Strategy | Parses `<mask>` content |
| `FilterParser` | Concrete Strategy | Parses `<filter>` primitives (feGaussianBlur, feDropShadow, feColorMatrix) |

#### 3.2.4 `builder.rs` — Tree Assembly (~160 lines)
Assembles the complete `SvgRenderTree` using the **Builder** design pattern.

**Key Types:**
| Type | Pattern | Purpose |
|------|---------|---------|
| `SvgRenderTreeBuilder` | Builder | Accumulates CSS rules, definitions, and recursively builds render nodes |
| `build()` | — | Finalizes: collects rules → builds nodes → collects definitions → applies `PaintServerFixupVisitor` |

**Flow:**
```
SvgRenderTreeBuilder::new(node, context)
  ├── collect_svg_css_rules()     — Phase 1: Parse inline <style>
  ├── build_render_node()         — Phase 2: Recursive DOM → SvgRenderNode (handles <use>)
  ├── extract_viewport_info()     — Phase 3: Viewbox, aspect ratio, overflow
  ├── DefinitionCollector×5       — Phase 4: Collect all definitions
  ├── PaintServerFixupVisitor     — Phase 5: Post-process gradient→pattern fixup
  └── Arc::new(tree)              — Return shared render tree
```

---

### 3.3 SVG Engine (`components/svg_engine/src/`)

**Purpose:** Pure SVG rendering engine with no Servo DOM dependency. Core rendering logic and data types.

#### 3.3.1 Shapes Module (`shapes/`)

**Data Types (all pure data, no WebRender dependency):**

| Type | SVG Element | Key Fields |
|------|-------------|------------|
| `Rectangle` | `<rect>` | x, y, width, height, rx, ry |
| `Circle` | `<circle>` | cx, cy, r |
| `Ellipse` | `<ellipse>` | cx, cy, rx, ry |
| `Line` | `<line>` | x1, y1, x2, y2 |
| `Polyline` | `<polyline>` | points: `Vec<kurbo::Point>` |
| `Polygon` | `<polygon>` | points: `Vec<kurbo::Point>` |
| `Path` | `<path>` | path: `kurbo::BezPath` |

**Design Pattern — Factory Method:**
Each shape implements `BuildFromElement::from_attrs(font_size, attrs)`, where `attrs` is any `AttrAccessor` implementor. This allows constructing shapes from real DOM elements or test doubles without coupling to Servo's layout DOM.

```rust
// Usage — works with any DOM backend:
let rect = Rectangle::from_attrs(16.0, &element)?;
// Usage — works with test mocks:
let rect = Rectangle::from_attrs(16.0, &mock_element)?;
```

**Design Pattern — Delegation Chain (Liskov Substitution):**
```
Circle → Ellipse (via equal rx=ry)
Ellipse → Rectangle (via 100% corner radii)
Polygon → Polyline (via first-point-closed)
Path → Polyline (via kurbo flatten)
Line → stroke_line_segment (no fill geometry)
```

All shapes ultimately reduce to two primitives: **Rectangle** (axis-aligned) and **Polyline** (tessellated).

**Clip Geometry:**
The `clip_info()` method on `Shape` converts each shape to `ClipGeometry::RoundedRect` (for circles, ellipses, rounded rects) or `ClipGeometry::Polygon` (for arbitrary polygons, paths), enabling participation in `<clipPath>` definitions.

#### 3.3.2 Style Module (`style/`)

**Data Types (all pure data, no WebRender dependency):**

| Type | SVG Properties |
|------|----------------|
| `NodeStyle` | visibility, display, transform, fill, stroke, render_hints, effects, opacity |
| `FillParams` | color, paint_server (gradient/pattern ref), opacity, fill_rule |
| `StrokeParams` | color, paint_server, opacity, width, line_cap, line_join, miter_limit, dash_array, dash_offset |
| `TransformOp` | Translate, Scale, Rotate, SkewX, SkewY, Matrix — parsed from transform attribute |
| `PaintServer` | Solid(color) | Gradient(id) | Pattern(id) |
| `GradientDef` | Linear(LinearGradient) | Radial(RadialGradient) |

**Key Design Pattern — Separated Interface:**
- `FillParams`/`StrokeParams` define what properties exist, not how they're computed
- Style construction (CSS → engine types) lives in the layout integration layer via `FromComputedValues`
- The engine crate is fully independent of Servo's CSS system

#### 3.3.3 Render Tree (`render_tree.rs`)

The core data structure passed from layout to the engine:

```rust
pub struct SvgRenderTree {
    pub root: SvgRenderNode,
    pub viewport: ViewportInfo,
    pub gradients: HashMap<String, GradientDef>,
    pub clip_paths: HashMap<String, ClipPathDef>,
    pub patterns: HashMap<String, PatternDef>,
    pub masks: HashMap<String, MaskDef>,
    pub filters: HashMap<String, FilterDef>,
}
```

Each `SvgRenderNode` has:
- Optional `id` for reference resolution
- `tag` — either `Shape(Shape)` or `Container(Container)`
- `style: NodeStyle` — resolved fill, stroke, transform, effects
- `children: Vec<SvgRenderNode>` — child nodes

**Design Pattern — Visitor:**
```rust
pub trait SvgRenderTreeVisitor {
    fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision;
}
pub trait SvgRenderTreeVisitorMut {
    fn visit_node_mut(&mut self, node: &mut SvgRenderNode) -> VisitDecision;
}
```
Used by the `PaintServerFixupVisitor` to post-process the tree (converting gradient references to pattern references where needed).

#### 3.3.4 Rendering Pipeline (`renderer/` + `traversal.rs`)

The rendering pipeline is a multi-stage process:

**Stage 1 — Entry Point ([`traversal::render_svg_tree`](components/svg_engine/src/traversal.rs)):**
1. Define SVG-level clip rect (unless `overflow: visible`)
2. Push reference frame for `viewBox` alignment
3. Call `render_node` on root

**Stage 2 — Node Traversal (`render_node`):**
1. Skip `display: none` elements
2. Apply node transforms (translate shifts origin; scale/rotate/skew/matrix push WebRender reference frames)
3. Resolve `clip-path` to WebRender clip chain
4. Resolve mask clips (one per mask shape for union behavior)
5. Resolve filter primitives to WebRender `FilterOp` list
6. Render shape (fill + stroke)
7. Recurse children
8. Pop pushed reference frames

**Stage 3 — Shape Rendering ([`Render` trait](components/svg_engine/src/renderer/mod.rs)):**

```rust
pub(crate) trait Render {
    fn render(&self, ctx: &mut RenderContext);
}
```

- **Rectangle**: Direct `push_rect` + `push_border` with optional rounded-rect clipping
- **Circle/Ellipse**: Delegate to Rectangle via 100% corner radii, with `push_border` for stroke
- **Line**: Only stroke (no fill per SVG spec) — rotated reference frame, dash decomposition, line cap handling
- **Polyline/Polygon/Path**: Tessellation for fill + per-segment decomposition for stroke

**Stage 4 — Fill/Stroke Pipeline:**

| Function | Purpose |
|----------|---------|
| `fill_rect()` | Paint server dispatch: solid color → `push_rect`; gradient → gradient fill; pattern → tile render |
| `fill_polygon()` | Tessellate polygon → scanline rasterize with per-pixel gradient evaluation |
| `fill_rect_with_gradient_by_id()` | Gradient fill for axis-aligned rects |
| `stroke_rect()` | WebRender border for solid colors; gradient fill + interior punch-out for gradient strokes |
| `stroke_polyline()` | Segment decomposition: per-segment solid stroke, or subdivided gradient evaluation along the polyline |
| `stroke_line_segment()` | Rotated reference frame, dash intervals, butt/round/square line caps |
| `dash_intervals()` | Stroke-dasharray decomposition with offset normalization |

**Design Pattern — Strategy (Gradients):**

```rust
trait GradientStrategy {
    fn compute_t(&self, x: f32, y: f32, bw: f32, bh: f32) -> f32;
}
struct LinearStrategy { gx1, gy1, gx2, gy2 }
struct RadialStrategy { cx, cy, fx, fy, r }
```

Both gradients share the same rendering loop; only the `t`-value computation differs.

#### 3.3.5 Tessellator (`tessellator.rs`)

Software rasterization for arbitrary polygon fills:

1. **Triangulation** — Uses `lyon` library's `FillTessellator` to decompose any polygon into triangles (supports `NonZero` and `EvenOdd` fill rules)

2. **Scanline Rasterization** — For each triangle, iterates Y-scanlines top-to-bottom, computing the horizontal span at each scanline via the standard triangle edge-walk algorithm

3. **Per-pixel Evaluation:**
   - **Solid**: Single `push_rect` per scanline
   - **Linear Gradient**: Evaluates gradient at 4px-cell midpoints, subdividing spans by color with **RLE optimization** (adjacent cells with the same color merge into a larger rect — reduces WebRender draw calls)
   - **Radial Gradient**: Same per-cell evaluation + RLE merge
   - **Pattern**: Groups cells by tile column, renders pattern shapes with proper clipping to the polygon boundary

4. **NaN Safety**: Explicit NaN handling in vertex sorting (NaN consistently floats to front, guard checks on span width)

#### 3.3.6 Effects Module (`effects/`)

**Clip Paths ([`clip.rs`](components/svg_engine/src/effects/clip.rs)):**
- Resolves `clip-path` URL references to `ClipPathDef` definitions
- Converts clip shapes to WebRender clip chains:
  - Circles/ellipses/rounded rects → `define_clip_rounded_rect`
  - Arbitrary polygons → bounding-rect clip (WebRender 0.69 limitation)
- `build_mask_clips()` produces one clip chain per mask shape for union rendering

**Filters ([`filter.rs`](components/svg_engine/src/effects/filter.rs)):**
- Resolves `filter` URL references to `FilterDef` definitions
- Converts filter primitives to WebRender `FilterOp`:
  - `feGaussianBlur` → `FilterOp::Blur`
  - `feDropShadow` → `FilterOp::DropShadow`
  - `feColorMatrix` → `FilterOp::ColorMatrix`

#### 3.3.7 Error Handling (`error.rs`)

```rust
pub enum SvgEngineError {
    MissingAttribute(String),
    ParseError(String),
    UnsupportedFeature(String),
}
```

A dedicated error type with:
- `Display` and `Debug` implementations for user-friendly and developer-friendly messages
- `std::error::Error` implementation for Rust integration
- Standard `SvgResult<T>` type alias throughout the engine
- Error propagation via `?` from parsing code to the layout integration layer

#### 3.3.8 Provider Traits

Three traits define the abstract interface between the renderer and the render tree:

| Trait | Methods | Implemented By |
|-------|---------|----------------|
| `PaintResourceProvider` | `gradient(id)`, `pattern(id)` | `SvgRenderTree` |
| `ClipMaskProvider` | `clip_path(id)`, `mask(id)` | `SvgRenderTree` |
| `FilterProvider` | `filter(id)` | `SvgRenderTree` |

This allows the traversal and effects modules to be generic over the provider — the same code works during test, with mock, or with the real tree.

#### 3.3.9 DomElement Trait (`domelement.rs`)

```rust
pub trait DomElement: AttrAccessor + Clone {
    type Child: DomElement;
    fn local_name(&self) -> &str;
    fn element_children(&self) -> Vec<Self::Child>;
    fn id(&self) -> Option<String>;
}
```

A **Separated Interface** pattern that keeps the `svg_engine` crate free of Servo layout-DOM dependencies. The trait enables:
- Unit testing shape construction with mock DOM data
- Testable definition collection
- Clear boundary between Servo-specific DOM access and engine-agnostic parsing

---

## 4. Design Patterns Summary

| Pattern | Where | Purpose | Benefit |
|---------|-------|---------|---------|
| **Visitor** | `render_tree.rs` + `visitor.rs` | Tree operations without ad-hoc recursion | PaintServerFixupVisitor replaces bespoke fixup_paint_servers |
| **Strategy** | `collects.rs` | Definition collection | `DefinitionCollector<T>` + `DefinitionParser` trait eliminates 5 near-identical functions |
| **Builder** | `builder.rs` | Multi-phase tree construction | `SvgRenderTreeBuilder` chains: CSS rules → definitions → tree → post-process |
| **Factory Method** | `shapes/*.rs` | Shape construction from DOM | `BuildFromElement::from_attrs()` is testable with mock data, lives in engine crate |
| **Strategy** | `renderer/gradient.rs` | Gradient coordinate computation | `GradientStrategy` with `LinearStrategy`/`RadialStrategy` — add new types without loop changes |
| **Separated Interface** | `domelement.rs` + provider traits | DOM abstraction | SVG engine has zero Servo layout dependency |
| **Context Object** | `renderer/mod.rs` — `RenderContext` | Bundle rendering parameters | No 6-parameter functions |
| **Delegation Chain** | `Renderer` impls | Shape rendering | Circle → Ellipse → Rectangle, Polygon/Path → Polyline |
| **Null Object** | `style/fill.rs`, `style/stroke.rs` `.none()` | Optional style fields | Reduces `Option<>` noise in renderer |
| **Template Method** | `svg/style.rs` — `build_style()` | Style resolution pipeline | ComputedValues → attributes → CSS rules: explicit stages |

---

## 5. Configuration & Feature Flags

### Preferences (`components/config/prefs.rs`)

| Pref | Default | Purpose |
|------|---------|---------|
| `layout_svg_engine_enabled` | `false` | Enable/disable the SVG engine path (requires restart) |

### Shell Defaults (`ports/servoshell/prefs.rs`)

The SVG engine is enabled by default in the servoshell, with additional rendering preferences:

| Pref | Default | Purpose |
|------|---------|---------|
| `layout_svg_engine_enabled` | `true` | SVG engine enabled in developer builds |
| `layout.unimplemented` | `false` | Experimental SVG-implemented features |
| `layout.variable_fonts.enabled` | `false` | Variable font support for SVG text |

---

## 6. Rendering Capabilities & Supported Features

### ✅ Supported (as of July 2026)

| Category | Features |
|----------|----------|
| **Basic Shapes** | `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>` (cubic/quadratic bezier) |
| **Structural** | `<g>`, `<defs>`, `<use>`, `<symbol>`, `<svg>` (nested) |
| **Paint Servers** | Solid colors, linear gradients, radial gradients, patterns |
| **Gradient Features** | ObjectBoundingBox / userSpaceOnUse units, gradient stops with opacity |
| **Strokes** | Solid, gradient (per-segment subdivided), dash arrays with offset, line caps (butt/round/square), line joins (miter/round/bevel) |
| **Transforms** | translate, scale, rotate (with center), skewX, skewY, matrix, CSS transform property |
| **ViewBox** | `viewBox` + `preserveAspectRatio` (meet/slice, all alignment types) |
| **Clipping** | `<clipPath>` with all shape types, `clipPathUnits`, objectBoundingBox |
| **Masking** | `<mask>` with multiple shapes, union rendering |
| **Filters** | `<feGaussianBlur>`, `<feDropShadow>`, `<feColorMatrix>` |
| **Styling** | Fill, stroke, opacity, visibility, display:none, inline CSS `<style>` class rules |
| **SVG Attributes** | `transform`, `fill`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `stroke-dasharray`, `stroke-dashoffset`, `stroke-opacity`, `fill-opacity`, `fill-rule`, `opacity`, `visibility`, `display`, `clip-path`, `mask`, `filter`, `class`, `id`, `style` |
| **Inheritance** | Style inheritance from `<g>` groups, currentColor fallback for fill |

### 🚧 Known Limitations

| Limitation | Impact | Target |
|------------|--------|--------|
| `<filter>` element's `stdDeviation` reads from parent filter (not primitive child) | Blur renders as solid | Fixed in upcoming commit |
| `rotate(a, cx, cy)` transform matrix order | Rotated elements may render offset | Under investigation |
| WebRender 0.69 no arbitrary polygon clip paths | `<clipPath>` with complex paths uses bounding-rect fallback | Needs WebRender upgrade |
| No `<text>` support | Text elements not rendered through engine (fall through to legacy path) | Future work |
| No SVG animation (SMIL) | Animated properties static on load | Future work |
| No incremental render tree updates | Full rebuild on each layout pass | Performance optimization |
| `<style>` CSS rule parsing limited to class selectors | No element selectors, no @media queries, no @import | Minimal |

---

## 7. Testing

### Unit Tests: 103 passing, 0 failing

| Test Suite | Tests | What's Tested |
|------------|-------|---------------|
| `render_tree` | 9 | ViewBox parsing, aspect ratio |
| `shapes::attr_parsers` | 18 | Length parsing (all SVG units), points parsing |
| `shapes::tests` | 21 | Factory construction for all 7 shapes with mock `AttrAccessor` |
| `style::color` | 13 | CSS/SVG color parsing (hex, named, rgb, hsl) |
| `style::transform_ops` | 16 | Transform string parsing, chained transforms, rotate with center |
| `renderer::stroke` | 15 | Stroke dash-interval decomposition |
| `error` | 4 | Error display, debug, type implementations |
| `tessellator` | 5 | Vertex sorting, NaN handling |
| `visitor` | 2 | PaintServer fixup visitor |

### Future Test Investments Needed
- Shape `Render` impls (require WebRender `DisplayListBuilder` mock)
- `render_svg_tree` end-to-end (require full tree construction)
- Style construction (svg/style.rs has 0 tests)
- Definition collection (svg/collects.rs has 1 test)

---

## 8. Performance Considerations

1. **Render Tree Rebuild** — The entire SVG render tree is reconstructed on every layout pass. No caching or incremental update mechanism exists yet.

2. **Scanline Rasterizer** — The tessellator evaluates gradient colors scanline-by-scanline with 4px cell subdivision. The **RLE optimization** (merged adjacent cells with the same color) reduces WebRender draw calls. For gradient-heavy SVGs, this still produces many primitives.

3. **Pattern Rendering** — Patterns re-render all child shapes per tile per scanline. For patterns with many shapes over large fill areas, this creates numerous primitives.

4. **CSS Rule Re-parsing** — Inline `<style>` content is parsed from scratch on every layout pass. A cache keyed on element identity would avoid redundant work.

5. **WebRender Primitive Count** — The engine primarily uses `push_rect`, which is well-optimized in WebRender. The full approach avoids intermediate rasterization to images, keeping the scene in GPU-friendly primitive form.

---

## 9. Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `svgtypes` | 0.16.1 | SVG attribute parsing (colors, transforms, viewBox, lengths) |
| `kurbo` | 0.11+ | Bezier path representation and flattening |
| `lyon` | 1.0+ | Polygon triangulation (FillTessellator) |
| `euclid` | 0.22+ | Geometry types (Transform2D, Point2D) |
| `webrender_api` | 0.69+ | Display list building, reference frames, clip chains |

---

## 10. Project Structure Reference

```
components/
├── svg_engine/                          ← NEW: The SVG rendering engine
│   └── src/
│       ├── lib.rs                       ← Public re-exports, cargo module declarations
│       ├── error.rs                     ← SvgEngineError type
│       ├── render_tree.rs               ← SvgRenderTree, SvgRenderNode, definitions
│       ├── traversal.rs                 ← Tree walk → WebRender display list
│       ├── tessellator.rs               ← Polygon triangulation + scanline rasterization
│       ├── visitor.rs                   ← NEW: PaintServerFixupVisitor
│       ├── domelement.rs                ← NEW: DomElement trait (Separated Interface)
│       ├── shapes/                      ← Pure data structs for geometric shapes
│       │   ├── mod.rs                   ← Shape enum, ClipGeometry, AttrAccessor, BuildFromElement
│       │   ├── rectangle.rs, circle.rs, ellipse.rs, line.rs
│       │   ├── polyline.rs, polygon.rs, path.rs
│       │   ├── attr_parsers.rs          ← Shared length/points parsing
│       │   └── tests.rs                 ← NEW: 21 factory method tests
│       ├── style/                       ← SVG property data types (no WR dep)
│       │   ├── mod.rs, fill.rs, stroke.rs, gradient.rs, hints.rs
│       │   ├── node_effects.rs, visibility.rs, color.rs, transform_ops.rs
│       ├── renderer/                    ← WebRender display list generation
│       │   ├── mod.rs                   ← Render trait, RenderContext, provider traits
│       │   ├── fill.rs, stroke.rs       ← Fill/stroke pipelines
│       │   ├── gradient.rs, pattern.rs  ← Paint server renderers
│       │   ├── transform.rs             ← Transform operation → WR reference frames
│       │   └── {rect,circle,ellipse,line,polyline,polygon,path}.rs
│       └── effects/                     ← Clip path and filter resolution
│           ├── mod.rs, clip.rs, filter.rs
│
├── layout/
│   └── svg/                             ← REFACTORED: Layout integration layer
│       ├── mod.rs                       ← NEW: Public API (build_svg_render_tree)
│       ├── style.rs                     ← REFACTORED: Style construction + presentation attrs
│       ├── collects.rs                  ← REFACTORED: Definition collection (Strategy pattern)
│       └── builder.rs                   ← NEW: SvgRenderTreeBuilder (Builder pattern)
│   ├── replaced.rs                      ← Modified: calls svg::build_svg_render_tree
│   ├── lib.rs                           ← Modified: pub mod svg (replaces 4 old modules)
│   ├── display_list/mod.rs              ← Modified: render_svg_tree integration
│   └── dom_traversal.rs                 ← Modified: SVG subtree walking
│
├── script/
│   └── dom/svg/                         ← NEW: SVG DOM element implementations
│       ├── mod.rs, svgelement.rs
│       ├── svgcircleelement.rs ... svgstopelement.rs (18 files)
│   ├── dom/element/create.rs            ← Modified: SVG element creation dispatch
│   ├── dom/node/virtualmethods.rs       ← Modified: SVG vtable dispatch
│   └── layout_dom/*.rs                  ← Modified: SVG layout data
│
└── script_bindings/webidls/             ← NEW: SVG WebIDL bindings (18 files)
```

---

## Appendix: Known Limitations & Roadmap

### Current (July 2026)
| Issue | Status |
|-------|--------|
| `rotate(a, cx, cy)` offset | Under investigation — see [#TODO] |
| Filter `stdDeviation` reads from parent | Incoming fix |
| Tessellator lacks unit tests | Not started |
| Style construction (svg/style.rs) lacks unit tests | Not started |

### Short-term
| Feature | Priority |
|---------|----------|
| Incremental render tree caching | Medium |
| CSS rule cache (avoid re-parsing `<style>`) | Low |
| Run-length encoding optimization already implemented | Done |
| `text` element support | Future |

### Long-term
| Feature | Notes |
|---------|-------|
| SVG Animation (SMIL) | Requires animation timing engine |
| SVG `<text>` rendering | Requires glyph/path integration |
| Conic gradients | Single GradientStrategy impl |
| SVG 2 full feature compliance | Ongoing |
| WebRender polygon clip path support | Requires WebRender upgrade |
