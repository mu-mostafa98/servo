# SVG Engine Architecture (A2b — Full Engine Crate)

> **Approach:** Workspace crate `components/layout/svg-engine/` that calls WebRender directly.
> **Integration:** Thin layer in `components/layout/svg/` that extracts Servo style → engine input.

---

## Table of Contents

1. [The 20 Problems With the Current Pipeline](#1-the-20-problems-with-the-current-pipeline)
2. [How the New Engine Fixes Them](#2-how-the-new-engine-fixes-them)
3. [Requirements Traceability](#3-requirements-traceability)
4. [File Structure](#4-file-structure)
5. [Type System](#5-type-system)
6. [Context and State](#6-context-and-state)
7. [Coordinate System](#7-coordinate-system)
8. [Dataflow](#8-dataflow)
9. [Reference Resolution and Defs](#9-reference-resolution-and-defs)
10. [Incremental Updates and Animation](#10-incremental-updates-and-animation)
11. [Rendering Strategy](#11-rendering-strategy)
12. [Integration Layer](#12-integration-layer)
13. [API Surface](#13-api-surface)
14. [Dependencies](#14-dependencies)
15. [Error Handling](#15-error-handling)
16. [Phased Build Plan](#16-phased-build-plan)
17. [Open Questions and Decisions](#17-open-questions-and-decisions)

---

## 1. The 20 Problems With the Current Pipeline

### How it works today

```
SVG DOM subtree
  ↓ serialize to XML string
XML string
  ↓ base64 encode
data:image/svg+xml;base64,...
  ↓ image cache
WebRender rasterizer
  ↓ bitmap
push_image() → one flat image on screen
```

### The issues

**🟡 1. Serialization round-trip** — The entire SVG subtree is serialized to XML → base64 → data URL → parsed again by the rasterizer. This is DOM-to-string-to-DOM: wasteful and lossy. Base64 adds 33% overhead.

**🔴 2. Bitmap-only output** — SVG becomes a flat pixel buffer. CSS transforms on parent elements (`scale(4)`, `translate(50%, 50%)`) scale the pre-rasterized bitmap — blurry at any size beyond 1:1. High-DPI displays need re-rasterization at full resolution. No way to keep vector crispness under any scaling.

**🔴 3. CSS inheritance broken** — Stylo computes all 11 style structs per element with full inheritance. But the rasterizer never sees them — it re-parses the XML from scratch with its own defaults. `fill="red"` as attribute works (it's in the XML). `fill: red` from a CSS stylesheet is **lost**.

**🔴 4. No CSS animations** — CSS `transition` and `@keyframes` on SVG properties (fill, stroke, transform) cannot work. The serialized data URL is static. Even when Stylo computes animated values, they never reach the rasterizer.

**🔴 5. No SMIL animations** — `<animate>`, `<set>`, `<animateTransform>`, `<animateMotion>` are completely unsupported. The serialize→bitmap path has no concept of time-varying attributes.

**🟡 6. No CSS stylesheet styling** — `<style>` blocks and external CSS files targeting SVG elements work in Stylo but are **lost in rendering**. The XML serialization only includes DOM attributes, not computed styles.

**🔴 7. No web fonts for SVG text** — SVG `<text>` can't use web fonts loaded by Servo. Font selection from `font-family` (computed by Stylo) is ignored.

**🔴 8. No SVG text layout** — `<text>` positioning (x, y, text-anchor, letter-spacing, word-spacing) not rendered. `<tspan>` not supported. No text selection, no copy-paste, no accessibility.

**🟡 9. `<use>` is fragile and expensive** — Currently: clone referenced subtree into DOM, serialize, remove clones. Fragile (clone/remove can cause side effects). Expensive (deep clones of large subtrees).

**🔴 10. No element-level interactivity** — One bitmap = one hit target. `pointer-events="none"` per element? Impossible. Hover effects? Impossible. Click handlers on specific elements? Impossible.

**🔴 11. No incremental updates** — Attribute change on one `<rect>` → invalidate entire SVG → re-serialize entire subtree → re-rasterize whole bitmap. Cannot say "just update this one rect's fill."

**🟡 12. No clip-path integration** — `clip-path` is computed by Stylo but not wired to WebRender's clip chains. No way to use WebRender's native clipping for SVG.

**🟡 13. No native gradient support** — WebRender has native `push_gradient()`, but SVG gradients go through the bitmap instead.

**🔴 14. No marker support** — `marker-start`, `marker-mid`, `marker-end` are registered in Stylo but the bitmap path ignores them.

**🟡 15. No filter support** — WebRender has filter APIs (`push_filters()`, `push_backdrop_filter()`), but SVG filters can't use them.

**🟢 16. No foreignObject support** — Can't embed HTML inside SVG. (Lower priority, rarely used.)

**🟡 17. No viewBox/preserveAspectRatio** — `ratio_from_view_box()` parses only basic integers. No `preserveAspectRatio`. SVG sizing defaults to 300×150 in the new engine.

**🟢 18. Cache invalidation is manual** — `invalidate_cached_serialized_subtree()` must be called manually in `attribute_mutated`, `children_changed`, `unbind_from_tree`. Easy to miss a case.

**🟢 19. No mask support** — `<mask>` and `mask` attribute not handled natively.

**🔴 20. Wrong architecture** — Treating `<rect fill="red">` as an image instead of "draw a red rectangle at (10,10)." The browser already has computed style, geometry, and layout — but throws it all away and starts over.

> Color key: 🔴 critical | 🟡 important | 🟢 nice-to-have

---

## 2. How the New Engine Fixes Them

| # | Problem | How the new engine solves it |
|---|---|---|
| 1 | Serialization round-trip | **Eliminated.** Engine reads DOM + Stylo directly. No XML, no base64, no data URL. |
| 2 | Bitmap-only output | **Vector display items.** Push shapes as `push_rect()`, `push_gradient()`, `push_text()`. No rasterization. Resolution-independent. |
| 3 | CSS inheritance broken | **Uses Stylo's computed values** for every element. All 11 style structs, fully resolved with inheritance. The log engine proved this works. |
| 4 | No CSS animations | Style changes trigger **per-element re-render**. Engine is stateless — call it again with new values. No cache to invalidate. |
| 5 | No SMIL animations | Future: engine accepts time-varying input. Attribute changes → re-render affected element. Same incremental model. |
| 6 | No CSS stylesheet styling | Same as #3. Stylo already handles this. Engine receives fully computed values including stylesheet rules. |
| 7 | No web fonts for SVG text | Integration layer uses Servo's font system to shape text → passes glyphs + font keys → engine calls `push_text()`. |
| 8 | No SVG text layout | Engine handles text positioning, text-anchor, letter-spacing. Integration provides shaped glyphs with font metrics. |
| 9 | `<use>` is fragile | Integration resolves `url(#id)` references **without DOM cloning**. Finds the referenced element and renders it in-place. |
| 10 | No element-level interactivity | Each shape becomes a **separate display item** with its own hit-test region. WebRender handles hit testing per item. |
| 11 | No incremental updates | Engine is **stateless**. Style changes → re-render only changed elements. No cache, no invalidation. |
| 12 | No clip-path integration | Engine receives resolved `ClipChainId` from integration. Uses WebRender's native clip chains. |
| 13 | No native gradient support | Engine calls `push_gradient()` / `push_radial_gradient()` directly. WebRender renders gradients natively. |
| 14 | No marker support | Future: engine builds marker geometry at path vertices. Integration resolves marker references. |
| 15 | No filter support | Future: engine wraps shapes in `push_stacking_context_with_filters()` with SVG filter data. |
| 16 | No foreignObject | Lower priority. Requires embedding Servo's HTML layout inside SVG viewport. |
| 17 | No viewBox/preserveAspectRatio | Integration computes viewBox transform + preserveAspectRatio → passes as initial transform. |
| 18 | Cache invalidation | **No cache.** Engine is stateless. Nothing to invalidate. |
| 19 | No mask support | Future: integration resolves mask → engine applies via WebRender clip/mask API. |
| 20 | Wrong architecture | **Fixed completely.** SVG elements become native display items in the pipeline, same as HTML. |

---

---

## 4. File Structure

```
components/layout/svg-engine/                ← workspace crate
├── Cargo.toml                               ← deps: webrender_api, kurbo, euclid, app_units, log
│
└── src/
    ├── lib.rs                                ← SvgEngine struct, pub re-exports
    │                                           Main entry point
    │
    ├── parser/                               ← Pure string → data (no Servo deps)
    │   ├── mod.rs                            ← re-exports
    │   ├── path.rs                           ← parse "d" attribute → Vec<PathCmd>
    │   ├── points.rs                         ← parse "points" → Vec<Point>
    │   ├── lengths.rs                        ← parse "10px", "5em", "50%", unitless → f64
    │   └── transform.rs                      ← parse "transform" → Transform2D
    │                                           No serialization, direct from DOM attrs
    │
    ├── shapes.rs                             ← SvgTag enum, ParsedGeometry, SvgRenderInput
    │                                          One enum variant per shape type
    │
    ├── render.rs                             ← SvgEngine::render_element() dispatcher
    │                                          Stateless — called fresh each frame
    │
    ├── paint.rs                              ← Shape → WebRender display item
    │                                           Vector output, no bitmap
    │                                           push_gradient() for gradient fills
    │
    ├── context.rs                            ← SvgContext: transform stack, clip stack
    │                                           Transform stack handles viewBox + attributes
    │
    ├── gradient.rs                           ← SVG gradients → WebRender gradient data
    │                                           Uses push_gradient() natively
    │
    ├── clip.rs                               ← clipPath → WebRender ClipChain
    │                                           Uses WR clip chains
    │
    └── text.rs                               ← Text positioning engine
                                              Position from coords, font from integration

components/layout/svg/                       ← integration (in layout crate)
├── mod.rs                                   ← re-exports
└── integration.rs                           ←
    ├── extract_style()                      ← Stylo ComputedValues → plain structs
    ├── resolve_references()                 ← url(#id) → target element (no DOM clone)
    ├── compute_viewbox()                    ← viewBox + preserveAspectRatio → transform
    ├── traverse_svg_tree()                  ← Walk DOM, call engine per element
    └── shape_svg_text()                     ←Servo fonts → glyphs + font key
```

---

## 5. Type System

### 5.1 SvgTag — Identify element type

```rust
/// Every SVG element tag the engine can render
pub enum SvgTag {
    // Basic shapes — each produces a different geometry type
    Rect, Circle, Ellipse, Line, Polyline, Polygon, Path,
    // Structure — affect traversal, not rendering
    Group, SvgRoot, Defs,
    // References
    Use,
    // Text
    Text, TSpan,
    // Paint servers
    LinearGradient, RadialGradient, Stop, Pattern,
    // Clipping / Masking
    ClipPath, Mask,
    // Other
    Image, Marker, Filter, FeGaussianBlur, FeOffset, /* ... more filter primitives */,
    // Unknown — skip gracefully, log warning
    Unknown,
}
```

> **Why:** Each tag maps to specific rendering logic. `Defs`, `LinearGradient`, `ClipPath` are never rendered directly — they provide data for other elements via references.

### 5.2 PathCmd — Geometry from `d` attribute

```rust
/// Parsed path command from d="M10 10 L20 20 Z"
#[derive(Clone, Debug)]
pub enum PathCmd {
    MoveTo          { x: f64, y: f64 },
    LineTo          { x: f64, y: f64 },
    HorizontalLineTo { x: f64 },
    VerticalLineTo   { y: f64 },
    CurveTo         { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    SmoothCurveTo   { x2: f64, y2: f64, x: f64, y: f64 },
    QuadTo          { x1: f64, y1: f64, x: f64, y: f64 },
    SmoothQuadTo    { x: f64, y: f64 },
    ArcTo           { rx: f64, ry: f64, x_axis_rotation: f64, large_arc: bool, sweep: bool, x: f64, y: f64 },
    ClosePath,
}
```

> **Why:** No XML serialization. Attributes parsed directly from the DOM.

### 5.3 ParsedGeometry — Shape-specific geometry

```rust
/// Parsed geometry — one variant per shape type
#[derive(Clone, Debug)]
pub enum ParsedGeometry {
    Rect     { x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64 },
    Circle   { cx: f64, cy: f64, r: f64 },
    Ellipse  { cx: f64, cy: f64, rx: f64, ry: f64 },
    Line     { x1: f64, y1: f64, x2: f64, y2: f64 },
    Polyline { points: Vec<Point2D<f64>> },
    Polygon  { points: Vec<Point2D<f64>> },
    Path     { commands: Vec<PathCmd> },
    None,
}
```

### 5.4 Style structs — Plain data from Stylo

```rust
/// Fill parameters — extracted from Servo's ComputedValues
/// Inherited values already resolved by Stylo
/// Animated values already computed by Stylo
#[derive(Clone, Debug)]
pub struct FillParams {
    pub color: ColorF,
    pub opacity: f32,
    pub rule: FillRule,          // NonZero / EvenOdd
}

/// Stroke parameters
#[derive(Clone, Debug)]
pub struct StrokeParams {
    pub color: ColorF,
    pub opacity: f32,
    pub width: f64,
    pub line_cap: LineCapKind,   // Butt / Round / Square
    pub line_join: LineJoinKind, // Miter / Round / Bevel
    pub miter_limit: f32,
    pub dash_array: Vec<f64>,
    pub dash_offset: f64,
}

/// Text parameters
#[derive(Clone, Debug)]
pub struct TextParams {
    pub position: Point2D<f32>,
    pub font_key: FontInstanceKey,
    pub glyphs: Vec<GlyphInstance>,
    pub color: ColorF,
    pub opacity: f32,
    pub text_anchor: TextAnchor, // Start / Middle / End
    pub letter_spacing: f32,
    pub word_spacing: f32,
}

/// Paint server reference
#[derive(Clone, Debug)]
pub enum PaintServer {
    Color(ColorF),
    LinearGradient(LinearGradientParams),
    RadialGradient(RadialGradientParams),
    None,
}
```

### 5.5 SvgRenderInput — One element's render data

```rust
/// Complete input for rendering one SVG element.
/// Stateless: engine doesn't store this. Caller builds fresh each frame.
///  Style can change between calls (animation).
/// One call per element, not one call per SVG.
pub struct SvgRenderInput<'a> {
    pub tag: SvgTag,
    pub geometry: ParsedGeometry,
    pub fill: Option<FillParams>,
    pub stroke: Option<StrokeParams>,
    pub transform: Option<Transform2D<f64>>,
    pub opacity: f32,
    pub text: Option<TextParams>,
    pub clip_path_id: Option<ClipChainId>,
}
```

### 5.6 Type Relationship Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│  Integration Layer (layout/svg/integration.rs)                     │
│  Walks DOM tree. Builds SvgRenderInput per element.                │
│  Extracts Stylo computed values                                │
│  Resolves url(#id) references                                  │
│  Computes viewBox transform                                   │
└──────────────────────┬─────────────────────────────────────────────┘
                       │  SvgRenderInput per element
                       ▼
┌────────────────────────────────────────────────────────────────────┐
│  SvgEngine                                                         │
│  Stateless — called fresh each frame                          │
│                                                                    │
│  render_element(input, ctx)                                        │
│       │                                                            │
│       ├── tag: Rect    → paint::fill_and_stroke_rect()             │
│       ├── tag: Circle  → paint::fill_and_stroke_circle()           │
│       ├── tag: Path    → paint::fill_and_stroke_path()             │
│       ├── tag: Text    → paint::render_text()                      │
│       ├── tag: Group   → (no rendering, traversal only)            │
│       └── tag: ...     →                                          │
│              │                                                     │
│              ├── Uses SvgContext for transform stack          │
│              ├── Calls webrender_api directly                  │
│              │   push_rect(), push_gradient(), push_text(),         │
│              │   push_stacking_context(), push_reference_frame()    │
│              └── Returns nothing (display list built in-place)     │
│                                                                    │
│  SvgContext (mutable, per-root-SVG)                                │
│  ├── dl: &mut DisplayListBuilder                output target │
│  ├── transform_stack: Vec<Transform2D<f64>>     nested xforms│
│  └── clip_stack: Vec<ClipChainId>               nested clips │
└────────────────────────────────────────────────────────────────────┘
```

### 5.7 Reference data — For url(#id) resolution

```rust
/// Collected defs — populated by integration before rendering
/// Stores references without DOM cloning
pub struct SvgDefs {
    pub gradients: HashMap<String, GradientDef>,
    pub clip_paths: HashMap<String, ClipPathDef>,
    pub markers: HashMap<String, MarkerDef>,
    pub masks: HashMap<String, MaskDef>,
}

pub struct GradientDef {
    pub kind: GradientKind, // Linear | Radial
    pub stops: Vec<GradientStop>,
    pub gradient_units: GradientUnits, // userSpaceOnUse | objectBoundingBox
    pub transform: Option<Transform2D<f64>>,
}

pub struct ClipPathDef {
    pub geometry: ParsedGeometry,
    pub clip_rule: FillRule,
}
```

> **Why:** The integration layer collects all `<defs>`, `<linearGradient>`, `<clipPath>`, etc. into `SvgDefs` BEFORE rendering. When an element references `url(#myGradient)`, the integration resolves it from this map — no DOM cloning, no serialization.

---

## 6. Context and State

```rust
/// Mutable context passed through rendering.
/// No cached state — just transform stack + clip stack.
/// Transform stack accumulates viewBox + attribute transforms.
pub struct SvgContext<'a> {
    /// The display list builder — engine pushes items directly
    pub dl: &'a mut DisplayListBuilder,

    /// Stack of transforms from parent <g>/<svg> elements.
    /// Applied as WebRender reference frames.
    pub transform_stack: Vec<Transform2D<f64>>,

    /// Stack of clip chains (for nested clip paths).
    pub clip_stack: Vec<ClipChainId>,

    /// Current viewport in SVG user units.
    /// Used for percentage-based coordinates.
    pub viewport: (f64, f64, f64, f64), // (x, y, w, h)

    /// Collected defs — gradients, clip paths, markers
    pub defs: &'a SvgDefs,
}
```

> **Why stateless?** The engine holds no persistent state. Every call to `render_element()` is self-contained. This means:
> - CSS animation changes a fill color → integration builds new `SvgRenderInput` → engine renders with new color
> - SMIL animation changes an attribute → same flow
> - No cache invalidation
> - No stale state

---

## 7. Coordinate System

SVG has a complex coordinate system. The engine handles it through a combination of engine and integration:

### 7.1 Transform hierarchy

```
SVG user units (from attributes: x="10", cx="50", d="M 10 10 ...")
  ↓  viewBox + preserveAspectRatio transform  (computed by integration)
  ↓  <g transform="...">                       (per-element attribute)
  ↓  element's own transform="..."             (per-element attribute)
  ↓
WebRender device pixels
```

### 7.2 Division of labor

| Responsibility | Handled by |
|---|---|
| Parse `viewBox="0 0 200 200"` | Integration → `compute_viewbox_transform()` |
| Apply `preserveAspectRatio` | Integration → part of the initial transform |
| Parse `transform="translate(10,5) scale(2)"` | Engine → `parser::parse_transform()` |
| Stack transforms | Engine → `SvgContext::push_transform()` |
| Push WebRender reference frames | Engine → `ctx.dl.push_reference_frame()` |
| All geometry in SVG user units | Engine → no conversion needed |

### 7.3 Percentage handling

```rust
/// Length value that may be percentage-based
#[derive(Clone, Debug)]
pub enum SvgLength {
    Absolute(f64),         // "10", "10px", "10pt"
    Percentage(f64),       // "50%" — resolved against viewport
}

impl SvgLength {
    /// Resolve against current viewport dimension
    pub fn resolve(&self, viewport_size: f64) -> f64 {
        match self {
            SvgLength::Absolute(v) => *v,
            SvgLength::Percentage(p) => viewport_size * p / 100.0,
        }
    }
}
```

---

## 8. Dataflow

### 8.1 Main flow

```
 ┌──────────────────────┐
 │  HTML Parser         │
 │  (html5ever)         │
 └──────────┬───────────┘
            │ DOM tree
            ▼
 ┌──────────────────────┐
 │  Stylo               │
 │  (CSS engine)        │
 │                      │
 │  ComputedValues  │
 │      per element     │
 │  Animation       │
 │      updates         │
 │  Stylesheet      │
 │      rules           │
 └──────────┬───────────┘
            │ ComputedValues + DOM tree
            ▼
 ┌──────────────────────────────────────────────────────────────┐
 │  Layout (replaced.rs → svg/integration.rs)                    │
 │                                                                │
 │  1. Detect <svg> element                                 │
 │  2. Parse viewBox + preserveAspectRatio                  │
 │  3. Collect defs (gradients, clip paths)                  │
 │  4. Walk DOM tree, for each element:                      │
 │     a. Get ComputedValues from Stylo               │
 │     b. Extract style fields → FillParams, StrokeParams        │
 │     c. Parse geometry attribute strings → ParsedGeometry      │
 │     d. Resolve url(#id) references → resolved data        │
 │     e. Build SvgRenderInput                                   │
 │     f. Call engine.render_element()                           │
 │  5. Handle <g>: push/pop transforms                      │
 │  6. Handle <use>: resolve target, render in-place         │
 └──────────────────────┬──────────────────────────────────────┘
                        │ SvgRenderInput per element
                        ▼
 ┌──────────────────────────────────────────────────────────────┐
 │  svg-engine Crate                                             │
 │                                                               │
 │  Engine::render_element(input, ctx)    stateless call    │
 │                                                               │
 │  1. Resolve transform (stack × input.transform)         │
 │  2. Push reference frame if transform                   │
 │  3. Push stacking context if opacity < 1.0                   │
 │  4. Apply clip chain if clip_path_id                    │
 │  5. Match tag:                                               │
 │     ┌──────────┬──────────────┬──────────────────┐            │
 │     │  Rect    │  push_rect   │  (native WR)     │       │
 │     │  Circle  │  tessellate  │  or rasterize    │  Q1
 │     │  Path    │  flatten+fill│  or rasterize    │  Q1
 │     │  Text    │  push_text   │  (glyphs from    │   │
 │     │          │              │   integration)   │           │
 │     │  with    │  push_gradient               │       │
 │     │  linear  │  + clip to shape              │           │
 │     │  gradient│                               │           │
 │     └──────────┴──────────────┴──────────────────┘            │
 │  6. Pop stacking context / reference frame                    │
 └──────────────────────┬──────────────────────────────────────┘
                        │ WebRender DisplayListBuilder calls
                        ▼
 ┌──────────────────────┐
 │  WebRender           │
 │  Compositor → GPU    │
 │                      │
 │  Vector rendering│
 │  Per-item hit   │
 │       testing        │
 └──────────────────────┘
```

### 8.2 Step-by-step for a real example

```xml
<svg viewBox="0 0 200 200" width="400" height="400">
  <defs>
    <linearGradient id="g1">
      <stop offset="0%" stop-color="red"/>
      <stop offset="100%" stop-color="blue"/>
    </linearGradient>
    <clipPath id="c1">
      <circle cx="100" cy="100" r="80"/>
    </clipPath>
  </defs>
  <g transform="translate(10, 10)">
    <rect x="0" y="0" width="50" height="50" fill="url(#g1)" clip-path="url(#c1)"/>
  </g>
</svg>
```

| Step | Layer | Action |
|---|---|---|
| 1 | Layout | Encounters `<svg>` → calls integration |
| 2 | Integration | Parses `viewBox="0 0 200 200"` → creates scale transform (2x for 400×400) |
| 3 | Integration | Collects defs: `g1` → `GradientDef`, `c1` → `ClipPathDef` |
| 4 | Integration | Walks children, calls `render_element()` for `<g>` with transform |
| 5 | Engine | Pushes `translate(10,10)` reference frame |
| 6 | Integration | Builds `SvgRenderInput` for `<rect>`: resolves `url(#g1)` → `LinearGradientParams`, resolves `url(#c1)` → `ClipChainId` |
| 7 | Engine | `render_rect()`: applies clip chain → pushes gradient rect via `push_gradient()` + shape clip |
| 8 | Integration | Returns from `<g>` children |
| 9 | Engine | Pops reference frame |

### 8.3 Animation update flow

```
Frame 1:                   Frame 2 (animated fill changes):
                           │
Stylo computes:            Stylo recomputes:
  rect.fill = red           rect.fill = blue
  │                         │
Integration builds:        Integration builds:
  input.fill = Fill(red)    input.fill = Fill(blue)
  │                         │
Engine renders:            Engine renders:
  push_rect(fill=red)       push_rect(fill=blue)
  │                         │
WebRender composits        WebRender composits

No cache to invalidate.
No serialization.
No full re-render.
Just the affected element's display item.
```

---

## 9. Reference Resolution and Defs

### 9.1 The problem with `url(#id)`

SVG elements frequently reference other elements:
```xml
<rect fill="url(#gradientId)" clip-path="url(#clipId)" marker-start="url(#markerId)"/>
```

In the current pipeline, these are handled by:
- Serializing the entire SVG including `<defs>` → data URL
- The rasterizer resolves the references internally
- `<use>` is handled by DOM cloning (fragile!)

### 9.2 New approach: two-pass resolution

**Pass 1 — Collect defs** (integration, before rendering):

```rust
fn collect_defs(node: ServoLayoutNode) -> SvgDefs {
    let mut defs = SvgDefs::default();
    for child in node.descendants() {
        match child.local_name() {
            "linearGradient" => { /* parse stops, units, transform */ },
            "radialGradient" => { /* parse stops, units, transform */ },
            "clipPath" => { /* parse child geometry */ },
            "marker" => { /* parse marker geometry + orient */ },
            "mask" => { /* parse mask content */ },
            _ => {}
        }
    }
    defs
}
```

**Pass 2 — Resolve during rendering** (integration, per element):

```rust
fn resolve_fill(svg: &InheritedSVG, defs: &SvgDefs) -> PaintServer {
    match &svg.fill {
        // url(#g1) → look up in defs
        PaintServer::Reference(id) => {
            if let Some(gradient) = defs.gradients.get(id) {
                PaintServer::LinearGradient(gradient.into())
            } else {
                PaintServer::None
            }
        }
        // solid color
        PaintServer::Color(c) => PaintServer::Color(c.into()),
        PaintServer::None => PaintServer::None,
    }
}
```

> **Why this is better than the current approach:**
> - No DOM cloning for `<use>` — the reference is resolved and the target element is rendered in its place with the current transform
> - No serialization — defs stay as DOM nodes, parsed once
> - References are resolved at render time — if Stylo updates a gradient's stop colors, the new values are used

### 9.3 `<use>` handling

```rust
// In integration layer, during tree traversal:
fn handle_use_element(
    use_node: ServoLayoutNode,
    engine: &SvgEngine,
    ctx: &mut SvgContext,
) {
    let href = use_node.get_attribute("href"); // or "xlink:href"
    let target_id = href.strip_prefix("#").unwrap();
    
    // Find referenced element in the DOM (no cloning!)
    let target = document.get_element_by_id(target_id);
    
    // Apply <use> x/y as additional transform
    let use_transform = extract_use_transform(use_node);
    ctx.push_transform(use_transform);
    
    // Render the target element in-place
    render_svg_element(target, engine, ctx);
    
    ctx.pop_transform();
}
```

---

## 10. Incremental Updates and Animation

### 10.1 Why the engine is stateless

The engine holds **no mutable state between calls**. Every `render_element()` call is fully self-contained:

```rust
impl SvgEngine {
    pub fn render_element(&self, input: &SvgRenderInput, ctx: &mut SvgContext) {
        // Reads input, writes to ctx.dl
        // Nothing is stored in self
    }
}
```

This means:
- **Re-rendering is free** — just call it again with new values
- **No dirty tracking** — the integration layer decides what needs re-render
- **No cache invalidation** — there's nothing to invalidate
- **Thread-safe** — engine can be called from any thread

### 10.2 Animation support path

```
Feature          │ How it works
─────────────────┼─────────────────────────────────────────────
CSS transition   │ Stylo computes animated value
                 │ Integration extracts it → SvgRenderInput
                 │ Engine renders with new value
                 │ (Same path as normal rendering!)

CSS @keyframes   │ Same as above — Stylo handles the timeline
                 │ Engine is called each animation frame

SMIL <animate>   │ Future: attribute changes → re-render
                 │ Engine doesn't need changes — just new input

SMIL             │ Future: same model
<animateTransform>│ Transform attribute in SvgRenderInput changes
```

### 10.3 What changes during animation

For an animation that changes `fill` from red to blue:

| Current pipeline | New engine |
|---|---|
| 1. Attribute changes on `<rect>` | 1. Stylo animates fill value |
| 2. Cache invalidated manually | 2. Layout re-encounters `<svg>` |
| 3. Whole SVG re-serialized to XML | 3. Integration extracts new fill |
| 4. Base64 encoded | 4. Engine renders rect with new fill |
| 5. Image cache re-rasterizes | 5. Only this rect's display item changes |
| 6. Whole bitmap pushed again | 6. WebRender patches the display list |

---

## 11. Rendering Strategy

### 11.1 Available WebRender primitives

| Primitive | Available | For SVG |
|---|---|---|
| `push_rect()` | ✅ | `<rect>` fill |
| `push_border()` with corner radii | ✅ | `<rect rx="...">` fill |
| `push_gradient()` / `push_radial_gradient()` | ✅ | Gradient fills on rect bounds |
| `push_text()` | ✅ | `<text>` (needs glyphs + font key) |
| `push_line()` | ✅ | Axis-aligned `<line>`, stroke segments |
| `push_stacking_context()` | ✅ | Opacity, blend modes |
| `push_reference_frame()` | ✅ | Transforms |
| `push_image()` | ✅ | Fallback for complex shapes |
| Arbitrary path (bezier, arc) | ❌ **NONE** | Circles, ellipses, `<path>`, `<polygon>` |

### 11.2 Challenge: No path API in WebRender 0.68

WebRender 0.68 has **no way to push an arbitrary bezier path**. This means for circles, ellipses, paths, polylines, and polygons, we need a strategy.

### 11.3 Strategy options

| Strategy | Description | Pro | Con |
|---|---|---|
| **(a) Clip-mask** | Push path as clip, fill with color/gradient rect | Pure vector, resolution-independent | WebRender may not support arbitrary path clips |
| **(b) Rasterize** | Render shape to offscreen bitmap via `tiny_skia`, push as `push_image()` | Simple, works for all shapes | Back to bitmap, loses resolution independence |
| **(c) Rounded-rect approximation** | Approximate circles/ellipses as rounded rects | Pure vector, uses native WR | Wrong geometry — can't do paths or polygons |
| **(d) Push transformed rect** | Use `push_rect()` with transforms for basic shapes | Simple vector approach | Limited — only works for rect-like shapes |
| **(e) Tessellate** | Flatten bezier to lines, triangulate, push as... | Vector approach | WR 0.68 has no `push_triangles()` either |

### 11.4 Recommended hybrid approach

```
For each shape, pick the best available primitive:

┌────────────────┬──────────────────────────────────┐
│ Shape          │ Rendering method                  │
├────────────────┼──────────────────────────────────┤
│ Rect (no rx)   │ push_rect() — native         │
│ Rect (with rx) │ push_border() — native       │
│ Line (h/v)     │ push_line() — native         │
│ Line (diag)    │ push_rect() rotated — approximate │
│ Circle         │ TBD (Q1)                         │
│ Ellipse        │ TBD (Q1)                         │
│ Path           │ TBD (Q1)                         │
│ Polyline       │ TBD (Q1)                         │
│ Polygon        │ TBD (Q1)                         │
│ Text           │ push_text() — native  │
│ Gradient fill  │ push_gradient() — native    │
└────────────────┴──────────────────────────────────┘
```

The final strategy for circles, ellipses, paths depends on **Q1 decision**.

---

## 12. Integration Layer

### 12.1 Integration responsibilities

The integration layer lives in `components/layout/svg/integration.rs` (~250-350 lines). It is the **bridge between Servo's types and the engine's plain structs**.

```
Integration task
──────────────────────────────────────────
Extract ComputedValues → FillParams        (CSS inheritance)
Extract ComputedValues → StrokeParams      (stylesheets + animations)
Parse geometry attribute strings           (no serialization)
Resolve url(#id) references                (no DOM cloning)
Compute viewBox + preserveAspectRatio      (correct sizing)
Traverse DOM tree, call engine per element (correct architecture)
Push/pop transform stack for <g>/<svg>     (scale + nesting)
Shape SVG text via Servo font system       (web fonts + layout)
Collect defs before rendering              (gradients, clips, markers)
```

### 12.2 Style extraction detail

```rust
/// Extract fill parameters from Servo's computed style.
/// Inheritance already resolved by Stylo.
/// Animated values already applied.
/// Stylesheet rules already cascaded.
fn extract_fill(svg: &style::structs::InheritedSVG) -> Option<FillParams> {
    let color = convert_color(&svg.fill);   // Servo color → ColorF
    let opacity = svg.fill_opacity;          // f32
    let rule = svg.fill_rule;               // NonZero / EvenOdd
    if color.a == 0.0 { return None; }
    Some(FillParams { color, opacity, rule })
}

/// Extract stroke parameters
fn extract_stroke(svg: &style::structs::InheritedSVG) -> Option<StrokeParams> {
    let color = convert_color(&svg.stroke);
    let opacity = svg.stroke_opacity;
    let width = svg.stroke_width.0;          // Au → f64
    let line_cap = convert_line_cap(&svg.stroke_linecap);
    let line_join = convert_line_join(&svg.stroke_linejoin);
    let miter_limit = svg.stroke_miterlimit;
    let dash_array = convert_dash_array(&svg.stroke_dasharray);
    let dash_offset = svg.stroke_dashoffset.0;
    if color.a == 0.0 || width == 0.0 { return None; }
    Some(StrokeParams { color, opacity, width, line_cap, line_join, miter_limit, dash_array, dash_offset })
}
```

### 12.3 Tree traversal

```rust
/// Walk SVG DOM subtree and call engine for each renderable element.
/// One call per element, not one call per SVG.
fn traverse_svg_tree(node: ServoLayoutNode, engine: &SvgEngine, ctx: &mut SvgContext, defs: &SvgDefs) {
    for child in node.flat_tree_children() {
        let style = child.style(&ctx.style_context);
        
        match child.local_name() {
            // Shape elements
            "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "path" => {
                let input = build_render_input(child, style, defs);
                engine.render_element(&input, ctx);
            },
            // Group with transform
            "g" => {
                if let Some(t) = parse_transform_attr(child) {
                    ctx.push_transform(t);   // push reference frame
                }
                traverse_svg_tree(child, engine, ctx, defs);
                if ctx.transform_stack.len() > prev_len {
                    ctx.pop_transform();
                }
            },
            // Use — resolve reference, render target
            "use" => {
                handle_use_element(child, engine, ctx, defs);
            },
            // Text — shape via Servo fonts
            "text" => {
                let text_params = shape_text(child, style);
                let input = build_text_input(child, text_params, defs);
                engine.render_element(&input, ctx);
            },
            // Non-rendering elements — skip
            "defs" | "linearGradient" | "radialGradient" | "clipPath" | "mask" | "marker" | "stop" | "filter" => {},
            // Unknown — log and skip
            _ => { log::warn!("Unsupported SVG element: {}", child.local_name()); }
        }
    }
}
```

### 12.4 What the integration layer does NOT do

- **No serialization**: Never converts DOM to string
- **No bitmap creation**: Never creates offscreen surfaces (unless Q1b is chosen)
- **No DOM manipulation**: Never clones or removes nodes
- **No caching**: Never stores rendered output

---

## 13. API Surface

### 13.1 Public API — what the integration layer calls

```rust
/// ─── Main engine API ───

impl SvgEngine {
    /// Create a new engine instance (stateless, lightweight).
    /// No initialization cost — engine holds no data.
    pub fn new() -> Self;

    /// Begin an SVG root element.
    /// Push viewBox transform onto the stack.
    /// viewport is needed for percentage resolution.
    pub fn begin_svg_root(
        &self,
        viewport: (f64, f64, f64, f64),      // x, y, w, h in SVG user units
        viewbox_transform: Option<Transform2D<f64>>,
        ctx: &mut SvgContext,
    );

    /// Render one SVG element.
    /// Called once per element by the integration layer.
    /// Stateless — same input always produces same output.
    /// Call again with new values for animation.
    pub fn render_element(
        &self,
        input: &SvgRenderInput,
        ctx: &mut SvgContext,
    );

    /// End an SVG root element.
    /// Pop viewBox transform.
    pub fn end_svg_root(&self, ctx: &mut SvgContext);
}

/// ─── SvgContext ───

impl SvgContext<'a> {
    pub fn new(dl: &'a mut DisplayListBuilder, defs: &'a SvgDefs) -> Self;

    /// Push/pop transforms for <g> and nested <svg>.
    /// Each transform becomes a WebRender reference frame.
    pub fn push_transform(&mut self, transform: Transform2D<f64>);
    pub fn pop_transform(&mut self);

    /// Push/pop stacking context for opacity and blend modes.
    pub fn push_stacking_context(&mut self, opacity: f32);
    pub fn pop_stacking_context(&mut self);
}

/// ─── Parsers (public for testing) ───

pub mod parser {
    /// Parse SVG path d attribute.
    pub fn parse_path_d(d: &str) -> Result<Vec<PathCmd>, ParseError>;
    /// Parse points attribute (polyline/polygon).
    pub fn parse_points(s: &str) -> Result<Vec<Point2D<f64>>, ParseError>;
    /// Parse a length value ("10", "10px", "50%").
    pub fn parse_length(s: &str) -> Result<SvgLength, ParseError>;
    /// Parse a transform attribute.
    pub fn parse_transform(s: &str) -> Result<Transform2D<f64>, ParseError>;
}

/// ─── Geometry helpers ───

impl ParsedGeometry {
    /// Build a kurbo BezPath from parsed geometry.
    pub fn to_bezpath(&self) -> Option<BezPath>;
}
```

### 13.2 What the integration layer calls (summary)

```
Integration calls these engine APIs:
  SvgEngine::new()                                      — once per layout thread
  SvgEngine::begin_svg_root(viewport, xform, ctx)       — once per <svg>
  SvgEngine::render_element(&input, ctx)                 — once per child element
  SvgEngine::end_svg_root(ctx)                           — once per <svg>
  SvgContext::new(&mut dl, &defs)                        — once per <svg>
  SvgContext::push_transform(xform)                      — per <g> with transform
  SvgContext::pop_transform()                             — per </g>
  SvgContext::push_stacking_context(opacity)             — per element with opacity≠1
  SvgContext::pop_stacking_context()                     — per element

Integration calls these engine parsers:
  parser::parse_path_d(d_str)                            — for <path d="...">
  parser::parse_points(points_str)                       — for <polyline/polygon points="...">
  parser::parse_length(len_str)                          — for x, y, width, height, cx, cy, r, etc.
  parser::parse_transform(transform_str)                 — for transform attributes
```

---

## 14. Dependencies

### 14.1 Direct (Cargo.toml)

| Crate | Version | Why |
|---|---|---|
| `webrender_api` | 0.68 | Display list building — `push_rect()`, `push_gradient()`, `push_text()`, `push_stacking_context()`, etc. |
| `kurbo` | 0.13 (euclid) | Bezier path representation (`BezPath`), path flattening, bounding boxes, shape math |
| `euclid` | 0.22 | 2D geometry types — `Point2D`, `Transform2D`, `Rect`, `Size2D` |
| `app_units` | 0.7 | CSS/layout unit types (`Au`) — for converting SVG lengths to device pixels | |
| `log` | workspace | Logging for parse errors, unsupported features | |

### 14.2 NOT dependencies (what we avoid)

| Crate | Reason excluded |
|---|---|
| `stylo` / `style` | Engine takes plain structs, not Servo's `ComputedValues`. Integration handles extraction. |
| `script` | Engine doesn't touch DOM. Integration handles DOM traversal. |
| `servo-layout` | Engine doesn't know about Servo's fragment tree or layout system. |
| `servo_arc` | No shared ownership. All data is owned or borrowed. |
| `fonts` / `fonts_traits` | Text shaping is integration's job. Engine receives pre-shaped glyphs. |
| `tiny_skia` | Avoid if possible (Q1)

### 14.3 Dependency graph

```
svg-engine
├── webrender_api    (display list → GPU)
├── kurbo            (bezier math)
├── euclid           (geometry types)
├── app_units        (unit conversion)
└── log              (diagnostics)

servo-layout crate
├── svg-engine       ← depends on the engine
├── stylo           ComputedValues
├── script           DOM tree traversal
├── fonts           Text shaping → glyphs + font keys
├── webrender_api    (already has it)
├── kurbo            (already has it)
└── ... (other layout deps)
```

---

## 15. Error Handling

```rust
/// Errors during SVG parsing/rendering.
/// Parse errors from attribute strings.
#[derive(Debug)]
pub enum SvgError {
    /// Invalid path data syntax (d="M10 10 L...")
    ParsePath(&'static str),
    /// Invalid points syntax (points="100,200 300,400")
    ParsePoints(&'static str),
    /// Invalid transform syntax (transform="translate(...)")
    ParseTransform(&'static str),
    /// Invalid length value ("10px", "50%")
    ParseLength(&'static str),
    /// Unsupported SVG feature
    Unsupported(&'static str),
}
```

**Error behavior rules:**
- Parse errors: log a warning, skip the element, don't crash the page
- Unsupported features (filters, markers initially): log + render nothing for that element
- The engine **never panics** from invalid SVG input
- Missing references (`url(#missing)`) are silently ignored

---

## 16. Phased Build Plan

### Phase 1 — Foundation (shapes without fill/stroke) [shortest path to visible output]

| Engine | Integration | Tests |
|---|---|---|
| `parser/path.rs` (minimal) | Geometry extraction for rect, circle, ellipse, line | Unit parser tests |
| `parser/points.rs` (minimal) | `extract_geometry()` function | Render a red rect |
| `parser/lengths.rs` | | |
| `shapes.rs` (all geometry variants) | | |
| `paint.rs` (fill rect only) | | |
| `render.rs` (dispatcher) | | |
| `lib.rs` (SvgEngine struct) | | |

**Success:** A `<rect fill="red">` appears on screen as a native rect.

### Phase 2 — Fill, stroke, opacity [visual completeness for shapes]

| Engine | Integration |
|---|---|
| `paint.rs` (fill all shapes + stroke) | `extract_fill()`, `extract_stroke()` |
| Gradient rendering | `convert_color()`, `convert_line_cap()`, etc. |

**Success:** All shapes render with correct fill, stroke, and opacity.

### Phase 3 — Transforms, viewBox, <g> [structure]

| Engine | Integration |
|---|---|
| `context.rs` (transform stack) | `parse_viewbox()`, `compute_viewbox_transform()` |
| `parser/transform.rs` | `traverse_svg_tree()` with <g> handling |
| Render transform support | |

**Success:** `<g transform="scale(2)"><rect .../></g>` works correctly.

### Phase 4 — Gradients [paint servers]

| Engine | Integration |
|---|---|
| `gradient.rs` | `collect_defs()` for gradients |
| Gradient fill in `paint.rs` | `resolve_fill()` with url(#id) lookup |

**Success:** `<rect fill="url(#g)"/>` renders with gradient.

### Phase 5 — Path rendering [complex shapes]

| Engine | Integration |
|---|---|
| Full path d-attribute parser | |
| Circle/ellipse/path rendering strategy (Q1) | |

**Success:** `<path d="M10 10 C20 20 30 30 40 10 Z"/>` renders correctly.

### Phase 6 — Clip paths

| Engine | Integration |
|---|---|
| `clip.rs` — clip chain integration | `collect_defs()` for clip paths |
| | `resolve_clip_path()` |
| | WebRender clip chain creation |

**Success:** `<rect clip-path="url(#c)"/>` clips correctly.

### Phase 7 — Text

| Engine | Integration |
|---|---|
| `text.rs` — positioning, text-anchor | `shape_text()` — Servo font → glyphs + font key |
| | Text layout (x, y, letter-spacing, word-spacing) |

**Success:** `<text x="50" y="50" text-anchor="middle">Hello</text>` renders correctly.

### Phase map to issues

| Phase | Issues resolved |
|---|---|
| P1 | |
| P2 | |
| P3 | |
| P4 | |
| P5 | Q1 |
| P6 | |
| P7 | |
| Future | |

---

## 17. Open Questions and Decisions

Open questions grouped by architectural area. Each has options and a recommendation. Decisions to be filled as we finalize.

### 17.1 Rendering (shapes, paint)

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q1** | **Path rendering strategy?** | (a) Clip-mask — render path as clip then fill a rect. Works with any shape but expensive for complex paths. (b) Rasterize via tiny_skia — render path to small bitmap, push as image. Simple, but loses vector quality. (c) Investigate WR internals — look for undocumented path API or extension. | **(b) Rasterize** — simplest for MVP. Can optimize later to (a) or (c) without API change. |
| **Q3** | **Gradients in MVP?** | (a) Include in Phase 2 (basic shapes). (b) Defer to Phase 5. | **(a) Include** — WebRender has native `push_gradient()`, very low effort. |

### 17.2 Text

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q2** | **Text in MVP?** | (a) Include in Phase 2 alongside shapes. (b) Defer to dedicated Phase 7. | **(b) Defer to P7** — text positioning, font shaping, and text-anchor make this the hardest SVG feature. Get shapes, gradients, and clipping working first. |

### 17.3 References (url(#id)) and &lt;use&gt;

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q4** | **Reference resolution (url(#id))?** | (a) Two-pass — first walk collects defs into a map, second pass resolves references during rendering. (b) One-pass lazy resolve — search DOM on the fly when a reference is encountered. | **(a) Two-pass** — cleaner, avoids `<use>` DOM cloning, predictable O(n) behavior. |
| **Q5** | **&lt;use&gt; element?** | (a) Include in Phase 3 alongside reference system. (b) Defer to later phase. | **(b) Defer** — needs reference resolution from Q4 first. |

### 17.4 Coordinate system

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q6** | **Who handles viewBox?** | (a) Engine — engine computes viewBox into transform internally. (b) Integration — integration layer pre-computes viewBox transform and passes to engine. | **(b) Integration** — keeps engine simpler. Integration already walks the DOM, can compute transform upfront. |

### 17.5 Error handling

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q7** | **How to handle errors?** | (a) Panic — unwrap/expect on malformed SVG. (b) Log + skip — log warning, skip bad element, continue rendering rest. | **(b) Log + skip** — browser must never crash on bad SVG. Graceful degradation is the web standard. |

### 17.6 Architecture and integration

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q8** | **Animation support?** | (a) Natural — stateless engine design enables CSS animation for free. No extra code. (b) Explicit animation API — add animation-specific hooks and state. | **(a) Natural** — stateless design (Section 10) already solves this. Stylo computes animated values per frame, engine renders same as static values. |
| **Q9** | **SVG as fragment or direct display items?** | (a) New `Fragment::SVG` variant — SVG becomes a first-class fragment type. (b) Build display items from replaced.rs — reuses existing SVG replaced element path. | **(b) Direct from replaced.rs** — fewer code changes, simpler integration. Engine emits display items, replaced.rs calls it during display list building. |

### 17.7 Decision log

| # | Decision | Date | Reason |
|---|---|---|---|
| Q1 | | | |
| Q2 | | | |
| Q3 | | | |
| Q4 | | | |
| Q5 | | | |
| Q6 | | | |
| Q7 | | | |
| Q8 | | | |
| Q9 | | | |
