# SVG Rendering Engine Architecture

I am developing a new SVG rendering engine for Servo and welcome community feedback on the architecture proposed below.

Two main areas:

- **SVG Engine** (`components/svg_engine/`) — standalone rendering crate (pure data in, WebRender display list out)
- **Integration Layer** (`components/layout/svg/`) — DOM → render tree bridge, lives in the existing layout crate

---

## 1. Architecture Overview

[INSERT ARCHITECTURE DIAGRAM IMAGE]

### Integration Layer
Converts `ServoLayoutNode` → `SvgRenderTree` (pure data, no DOM references):

- Collect `<defs>` definitions into lookup maps
- Resolve styles — Stylo `ComputedValues` + presentation attributes → `NodeStyle`
- Parse geometry attributes into shape structs
- Extract viewport / viewBox / preserveAspectRatio
- Convert transforms into `TransformOp`s

### SVG Engine
Consumes `SvgRenderTree` → WebRender display list. Two sub-layers:

- **Traversal** — recursive tree walk: applies transforms, resolves effects
  (clip-path / mask / filter), dispatches to renderer, recurses (skips `<defs>` and `<symbol>`)
- **Renderer Layer** — per-shape rendering via a `Render` trait. Each shape implements
  `Render::render()` — e.g. `rect::render()` generates display list commands for a rectangle

### WebRender
Already exists in Servo. Takes the display list, handles GPU compositing.

---

## 2. Data Model

[INSERT DATA MODEL DIAGRAM IMAGE]

`SvgRenderTree` — pure, owned tree, no DOM back-references.
- built in the integration layer, consumed by the engine
- Definition maps (`HashMap<String, …>`) — gradients, clip-paths, patterns, masks, filters,
  keyed by element `id`, used by `<use>` and `url(#…)` references

---

## 3. Rendering Pipeline

[INSERT RENDERING PIPELINE DIAGRAM IMAGE]

Two stages:

1. **Tree construction** — `build_svg_render_tree()` (`components/layout/svg/mod.rs`), called from `ReplacedContents::svg_kind_size()` in `components/layout/replaced.rs`
2. **Rendering** — `render_svg_tree()` (`components/svg_engine/src/traversal.rs`), called from `DisplayListBuilder::visit_image()` in `components/layout/display_list/mod.rs`
















WebRender doesn't natively support arbitrary paths or curved polygons. The approach uses two render terminals — every shape delegates to one of them:

### Rectangle — the GPU-clip terminal

Define a WebRender rounded-rect clip, draw a plain rectangle inside it. The GPU cuts the shape per-pixel. Gradient and pattern fills go through the same clip.

Line strokes use this same mechanism: a rotated reference frame + a rectangle whose length matches the segment and height matches the stroke width. Dashes are individual rectangles. No native line API — just `push_rect`.

### Polyline — the CPU-tessellation terminal

Lyon triangulates the polygon into triangles. A scanline rasterizer walks each triangle row-by-row, interpolates solid/gradient/pattern color per pixel, emits one `push_rect` per horizontal span. WebRender only sees rectangles.

### Delegation

| Shape | Delegates to | Cost |
|---|---|---|
| `Circle` | `Ellipse` → `Rectangle` | data conversion |
| `Ellipse` | `Rectangle` | data conversion |
| `Path` | `Polyline` | bezier flattening (sub-pixel tolerance) |
| `Polygon` | `Polyline` | loop closure |

Circle and ellipse are zero-cost — just different numbers flowing into Rectangle. Line is a rotated rect — same `push_rect` path. Path and polygon are cheap — flattening/closure, then Polyline does the real work.




WebRender doesn't natively support arbitrary paths or curved polygons. The approach uses two render terminals — every shape delegates to one of them:

### Rectangle — the GPU-clip terminal

Define a WebRender rounded-rect clip, draw a plain rectangle inside it. 

`Line` - strokes use this same mechanism: a rotated reference frame + a rectangle whose length matches the segment and height matches the stroke width. 

### Polyline — the CPU-tessellation terminal

Lyon triangulates the polygon into triangles. The engine walks each triangle row-by-row and emits one `push_rect` per row.


### Delegation

| Shape | Delegates to | Cost |
|---|---|---|
| `Ellipse` | `Rectangle` | (100% corner radii = visually an ellipse) |
| `Circle` | `Ellipse` |  (rx = ry = r) |
| `Path` | `Polyline` | flatten beziers to line segments (sub-pixel tolerance) |
| `Polygon` | `Polyline` | close the loop (append points[0]) |
