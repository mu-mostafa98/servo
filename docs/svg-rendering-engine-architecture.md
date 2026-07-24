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
- `builder.rs` — orchestrates tree construction (Builder pattern)
- `geometry.rs` — DOM element → `Shape` struct
- `style.rs` — Servo `ComputedValues` → SVG `NodeStyle`
- `defines.rs` — collect `<defs>` elements (Strategy pattern for each definition type)
- `css.rs` — inline `<style>` CSS rule parsing
- `viewport.rs` — `viewBox` / `preserveAspectRatio` extraction
- `transforms.rs` — CSS/SVG transform conversion

**SVG Engine** (`components/svg_engine/`) — pure rendering; zero dependencies on
Servo's DOM, layout, or style crates. Internally split into two sub-layers:

*Traversal sub-layer* — walks the render tree, manages coordinate system state,
then dispatches each node to the render layer.
- `traversal/` — recursive tree walk: viewport clip → viewBox frame → per-node transform/clip/mask/filter/opacity → dispatch to `Render`
- `effects/` — clip-path, mask, and filter resolution
- `visitor.rs` — Visitor pattern for tree post-processing (e.g., paint-server fixups)

*Render sub-layer* — receives dispatch calls for specific shapes/text/images,
emits WebRender display list commands via `push_rect`, `push_line`, etc.
- `renderer/` — `Render` trait + per-shape implementations (circle, rect, path, text, image)
- `renderer/fill.rs` — solid color, gradient, and pattern fill pipelines
- `renderer/stroke.rs` — stroking pipeline (width, dasharray, linecap, linejoin, gradient strokes)
- `renderer/gradient.rs` — linear/radial gradient stop evaluation
- `renderer/pattern.rs` — pattern tile rendering
- `tessellator/` — polygon triangulation (lyon) + scanline rasterization via `push_rect`

*Shared types* (used by both sub-layers):
- `render_tree/` — `SvgRenderTree`, `SvgRenderNode`, `SvgTag` enum, and definition types (ClipPathDef, MaskDef, FilterDef, PatternDef, GradientDef)
- `shapes/` — pure geometric data structs (Rect, Circle, Ellipse, Line, Polyline, Polygon, Path)
- `style/` — SVG property types (Fill, Stroke, Gradient, hints, visibility, transform_ops)
- `attr_parsers.rs` — SVG attribute value parsing utilities
- `error.rs` — error types for malformed SVG

**WebRender** — existing GPU rendering pipeline. Receives standard
`DisplayListBuilder` commands (rects, clips, stacking contexts, etc.) with no
SVG-specific awareness.

---

## 3. Data Flow

Rendering an SVG element happens in two phases, triggered at different points
in the layout pipeline. The structure of each layer is described in
[Section 2](#2-proposed-architecture-high-level); this section covers sequence.

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

The build is a single pass over the DOM subtree. Execution order:

1. Collect inline `<style>` CSS rules
2. Walk DOM depth-first — for each element, call `geometry::build_shape()`
   and `style::build_style()`, then recurse into children
3. Scan `<defs>` containers and collect definition maps (gradients, clip-paths,
   masks, filters, patterns)
4. Extract viewport parameters (`viewBox`, `preserveAspectRatio`, width/height)
5. Run post-processing visitors (e.g., fix paint-server references that
   were misclassified as gradients but actually point to patterns)

**Output:** `Arc<SvgRenderTree>` — stored in the layout fragment. No DOM
references remain; the tree is pure data.

### Step 3 — Trigger: display list construction

During display list building, `components/layout/display_list/mod.rs` retrieves
the `Arc<SvgRenderTree>` from the fragment and calls `render_svg_tree()`.

### Step 4 — Render: SvgRenderTree → WebRender commands

**Entry point:** `svg_engine::render_svg_tree(tree, origin, size, spatial_id,
clip_chain_id, wr)`

This is where the SVG Engine's two sub-layers run:

**Traversal** walks the tree depth-first. At each node it:
1. Pushes a transform reference frame (CSS transform + `transform` attribute)
2. Resolves clip-path, mask, and filter from the definition maps
3. Pushes an opacity stacking context
4. Dispatches to **Render** based on the node's `SvgTag` variant

**Render** receives the dispatch and emits display list commands:
- `Shape` → `shape.render(ctx)` — trait dispatch to the correct renderer,
  which calls fill/stroke helpers, tessellates polygons via lyon, and emits
  `push_rect` / `push_line` commands
- `Text` → shaped glyph positioning and emission
- `Image` → raster image emission
- `Container` → handled by Traversal (stacking context push/pop); children
  are visited recursively

**Output:** WebRender `DisplayListBuilder` commands.

### Step 5 — WebRender composites

WebRender receives standard display list primitives (rects, clips, stacking
contexts) with no SVG-specific awareness and renders them on the GPU.

---

## 4. Key Design Decisions

### 4.1 Separate `svg_engine` crate

The engine is a standalone crate (`components/svg_engine/`) with zero
dependencies on Servo's DOM, layout, or style crates. Its only external
dependencies are:

| Crate | Purpose |
|-------|---------|
| `webrender_api` | Emit display list commands |
| `euclid` | Geometry primitives (Rect, Point, etc.) |
| `kurbo` | Bezier curve / path operations |
| `lyon` | Polygon tessellation |
| `svgtypes` | SVG type parsing (viewBox, etc.) |

**Rationale:** Clean API boundary, testable in isolation, feature-gatable.

### 4.2 Intermediate Render Tree (`SvgRenderTree`)

The layout bridge produces a pure-data `SvgRenderTree` — no DOM references, no
style system references, just geometry and resolved property values:

```rust
struct SvgRenderTree {
    root: SvgRenderNode,
    viewport: ViewportInfo,
    gradients: HashMap<String, GradientDef>,
    clip_paths: HashMap<String, ClipPathDef>,
    patterns: HashMap<String, PatternDef>,
    masks: HashMap<String, MaskDef>,
    filters: HashMap<String, FilterDef>,
}

struct SvgRenderNode {
    id: Option<String>,
    tag: SvgTag,
    style: NodeStyle,
    transforms: Vec<TransformOp>,
    children: Vec<SvgRenderNode>,
}

enum SvgTag {
    Shape(Shape),          // rect, circle, ellipse, line, polyline, polygon, path
    Text(TextSpan),
    Image(SvgImage),
    Container(Container),  // Group, Svg, Defs, Use, Symbol
}
```

**Rationale:** Decouples DOM resolution from rendering. The tree can be unit-tested
without a full browser. Render-time lookups are flat hash-map lookups (no DOM
walks).

### 4.3 Shape-as-Data Enum

Shapes are plain data — no behavior, no rendering logic:

```rust
enum Shape {
    Rect(Rectangle),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polyline(Polyline),
    Polygon(Polygon),
    Path(Path),
}
```

Each variant is a simple struct (e.g., `Circle { cx, cy, r }`).

### 4.4 Render Trait (per-shape dispatch)

Each shape implements the `Render` trait, which emits WebRender display list
commands:

```rust
trait Render {
    fn render(&self, ctx: &mut RenderContext);
}

// One blanket impl on Shape does the match ONCE:
impl Render for Shape {
    fn render(&self, ctx: &mut RenderContext) {
        match self {
            Shape::Rect(r) => r.render(ctx),
            Shape::Circle(c) => c.render(ctx),
            // …
        }
    }
}
```

**Rationale:** Single match point — traversal code calls `shape.render(ctx)`
without knowing the concrete shape type. Adding a new shape means adding one
variant + one `Render` impl. No giant central match to update across the
codebase.

### 4.5 Software Tessellation for Fills

For filled polygons (especially with non-zero/even-odd fill-rule), use **lyon**
for triangulation, then scanline rasterize each triangle with `push_rect`:

```
Polygon vertices → lyon tessellation → triangles → scanline per triangle →
WebRender push_rect per scanline span
```

**Rationale:** WebRender's `define_clip_image_mask` requires a valid `ImageKey`,
which isn't always available. `push_rect` is known to work reliably. Alternative
would be to add proper polygon clipping support to WebRender — open to feedback.

### 4.6 Definition Collection via Strategy Pattern

`<defs>` elements are collected using a trait-based Strategy pattern:

```rust
trait DefinitionParser {
    type Definition;
    fn tag_names() -> &'static [&'static str];
    fn parse(node: ServoLayoutNode, ctx: &LayoutContext) -> Option<(String, Self::Definition)>;
}
```

Each definition type (gradient, clip-path, mask, filter, pattern) implements
`DefinitionParser`. A generic `DefinitionCollector` handles the common
recursion and deduplication logic.

**Rationale:** No per-type duplicated traversal code. Adding a new definition
type means implementing one trait.

### 4.7 Visitor Pattern for Tree Post-Processing

The render tree supports the Visitor pattern for post-processing passes:

```rust
trait SvgRenderTreeVisitor {
    fn visit_node(&mut self, node: &SvgRenderNode) -> VisitDecision;
}
```

Example use: `PaintServerFixupVisitor` — after tree construction, converts
`PaintServer::Gradient` to `PaintServer::Pattern` when the referenced ID is
actually a pattern definition (not a gradient).

---

## 5. Component Map

```
svg_engine/
├── shapes/           Pure geometry, no rendering logic
│   ├── circle.rs, ellipse.rs, line.rs
│   ├── path.rs, polygon.rs, polyline.rs
│   └── rectangle.rs
│
├── style/            SVG-specific property types
│   ├── fill.rs, stroke.rs, gradient.rs
│   ├── color.rs, hints.rs
│   ├── visibility.rs, node_effects.rs
│   └── transform_ops.rs
│
├── render_tree/      Tree structure + definition types
│   └── (SvgRenderTree, SvgRenderNode, SvgTag,
│        ClipPathDef, MaskDef, FilterDef, PatternDef)
│
├── renderer/         Per-shape rendering + paint pipelines
│   ├── render_trait.rs   ← Render trait + Shape dispatch
│   ├── providers.rs      ← Paint/Clip/Filter resource traits
│   ├── fill.rs, stroke.rs, gradient.rs, pattern.rs
│   ├── circle.rs, rect.rs, path.rs, text.rs, …
│   └── helpers.rs
│
├── traversal/        Tree walk → DisplayListBuilder
├── tessellator/      lyon triangulation + scanline raster
├── effects/          clip-path, mask, filter resolution
│
├── attr_parsers.rs   SVG attribute value parsing
├── error.rs          Error types
├── text.rs           Text types (TextSpan, TextAnchor)
└── visitor.rs        Visitor pattern traits

layout/svg/           (integration bridge — feature-gated)
├── builder.rs        SvgRenderTreeBuilder (Builder pattern)
├── geometry.rs       DOM → Shape construction
├── style.rs          ComputedValues → NodeStyle
├── defines.rs        <defs> collection (Strategy pattern)
├── css.rs            Inline <style> CSS parsing
├── viewport.rs       viewBox / preserveAspectRatio
└── transforms.rs     CSS/SVG transform conversion
```

---

## 6. Integration Points

| Point | Location | What happens |
|-------|----------|-------------|
| **DOM element creation** | `components/script/dom/element/create.rs` | SVG elements created (already done: #46558) |
| **Render tree construction** | `components/layout/replaced.rs` | Calls `layout::svg::build_svg_render_tree()` |
| **Fragment storage** | `components/layout/fragment_tree/fragment.rs` | Fragment holds `Arc<SvgRenderTree>` |
| **Display list emission** | `components/layout/display_list/mod.rs` | Calls `svg_engine::render_svg_tree()` |
| **Feature gate** | `components/layout/Cargo.toml` | `svg-engine` feature flag |

---

## 7. SVG Features — Scope

### In scope (initial implementation)

| Category | Features |
|----------|----------|
| **Shapes** | `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>` |
| **Fill** | Solid color, linear/radial gradients, patterns |
| **Stroke** | Width, dasharray, linecap, linejoin, miterlimit, gradient strokes |
| **Containers** | `<g>`, `<svg>`, `<defs>`, `<use>`, `<symbol>` |
| **Text** | `<text>`, `<tspan>` with text-anchor |
| **Viewport** | viewBox, preserveAspectRatio, overflow:visible |
| **Transforms** | translate, scale, rotate, skewX, skewY, matrix |
| **Clip/Mask** | clip-path (objectBoundingBox + userSpaceOnUse), mask |
| **Filters** | GaussianBlur, DropShadow, ColorMatrix, Saturate, LuminanceToAlpha, Offset, Flood, Composite, Tile, feImage |
| **Rendering hints** | shape-rendering, color-interpolation, paint-order, vector-effect:non-scaling-stroke |
| **Opacity/visibility** | CSS opacity, SVG visibility, display:none |

### Out of scope (future work)

- Animation (SMIL / CSS animations on SVG attributes)
- Markers (`<marker>`, arrowheads)
- `<foreignObject>`
- `textPath`
- Full SVG font support
- `getBBox` / `getCTM` / SVG DOM measurement API
- Hit testing / `pointer-events`
- `mix-blend-mode` / `isolation` on SVG elements

---

## 8. Open Questions for Reviewers

1. **Crate placement:** Should `svg_engine` be a top-level `components/` crate,
   or live inside `components/layout/`?

2. **Shape model:** Is the enum-based `Shape` the right fit, or would a
   trait-object approach scale better?

3. **Software tessellation:** Is lyon + scanline `push_rect` acceptable, or
   should we invest in proper polygon clipping support in WebRender?

4. **Text integration:** Should SVG `<text>` go through the engine entirely, or
   should it integrate with Servo's existing text layout pipeline for font
   selection, shaping, and bidirectional support?

5. **Feature gate:** Should this be behind a compile-time feature flag
   (`svg-engine`), a runtime preference, or just always-on?

6. **Incremental landing strategy:** What's the smallest mergeable unit?
   Proposal:
   - Phase 1: Shapes-only (no fill/stroke, just bounding boxes)
   - Phase 2: Solid fills + strokes
   - Phase 3: Gradients + patterns
   - Phase 4: Clip-paths, masks, filters
   - Phase 5: Text

7. **Hit testing:** SVG `pointer-events` requires geometry-based hit testing —
   should this live in the engine or in layout?

8. **Testing strategy:** What's the expectation? WPT reftests only, or also
   engine-internal unit tests?

---

## 9. Alternatives Considered

### A. Extend existing layout path
- Add SVG awareness to fragment construction, flow layout, display list building
- **Rejected:** Fundamentally different coordinate models; would add SVG-specific
  branches throughout layout code rather than isolating SVG concerns.

### B. Render SVG entirely in WebRender
- Pass raw SVG data to WebRender, let it handle rendering
- **Rejected:** WebRender has no SVG awareness; would require significant
  WebRender changes and break the abstraction layer.

### C. Single crate (no separate svg_engine)
- Put everything in `layout/svg/`
- **Rejected:** Harder to test in isolation, no clear API boundary, can't
  feature-gate cleanly.

---

## 10. References

- [SVG 2 Specification](https://www.w3.org/TR/SVG2/)
- [WebRender Documentation](https://github.com/servo/webrender)
- [lyon tessellation library](https://docs.rs/lyon/)
- [kurbo curve library](https://docs.rs/kurbo/)
- Prior PR: [Create DOM element types for SVG shapes (#46558)](https://github.com/servo/servo/pull/46558)
