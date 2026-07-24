# SVG Rendering Engine — Architecture Discussion

This is a proposal for a new SVG rendering engine for Servo. I'd like to get feedback on the
architecture and data model.

The proposal involves two main areas:

- **SVG Engine** (`components/svg_engine/`) — a standalone rendering crate (pure data in, WebRender display list out)
- **Integration Layer** (`components/layout/svg/`) — the DOM → render tree bridge that lives in the existing layout crate

---

## 1. Architecture Overview

[INSERT ARCHITECTURE DIAGRAM IMAGE]

### Script Layer
Standard Servo SVG DOM elements. No rendering logic lives here — elements carry their
attributes and computed styles.

### Integration Layer
Takes a `ServoLayoutNode` and produces an `SvgRenderTree` (pure data, no DOM references):

- Collect `<defs>` definitions (gradients, clip-paths, patterns, masks, filters) into lookup maps
- Resolve computed styles — Stylo `ComputedValues` + SVG presentation attributes → `NodeStyle`
- Parse geometry attributes into shape structs (x, y, width, height, cx, cy, r, points, d, etc.)
- Extract viewport/viewBox/preserveAspectRatio info
- Convert CSS/SVG transforms into `TransformOp`s
- Shape text via the font subsystem for glyph positioning
- Resolve `<use>` references by cloning the referenced subtree

### SVG Engine
Takes an `SvgRenderTree` and produces WebRender display list commands. Two main sub-layers:

- **Traversal** — recursive tree walk: applies transforms, resolves effects
  (clip-path/mask/filter), dispatches to the renderer, recurses into children,
  skipping `<defs>` and `<symbol>`.
- **Renderer Layer** — per-shape rendering via a `Render` trait. Each shape implements
  `Render::render()` — e.g. `rect::render()` generates display list commands for a rectangle.

### WebRender
Already exists in Servo. Takes the display list and handles GPU compositing.

---

## 2. Data Model

[INSERT DATA MODEL DIAGRAM IMAGE]

`SvgRenderTree` — a pure, owned tree with no DOM back-references.
- Definition maps (`HashMap<String, …>`) — gradients, clip-paths, patterns, masks, filters,
  keyed by element `id`, used by `<use>` and `url(#…)` references
- Owned by `Arc<SvgRenderTree>` — built in the integration layer, consumed by the engine

---

## 3. Rendering Pipeline

[INSERT RENDERING PIPELINE DIAGRAM IMAGE]

The pipeline has two stages:

1. **Tree construction** — `build_svg_render_tree()` (`components/layout/svg/mod.rs`), called from `ReplacedContents::svg_kind_size()` in `components/layout/replaced.rs`
2. **Rendering** — `render_svg_tree()` (`components/svg_engine/src/traversal.rs`), called from `DisplayListBuilder::visit_image()` in `components/layout/display_list/mod.rs`

---
