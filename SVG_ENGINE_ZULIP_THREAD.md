# SVG Rendering Engine — Architecture Discussion

Proposal for a new SVG rendering engine for Servo. Feedback welcome on the architecture
and data model.

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
- Shape text via the font subsystem
- Resolve `<use>` references (clone the referenced subtree)

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
- Definition maps (`HashMap<String, …>`) — gradients, clip-paths, patterns, masks, filters,
  keyed by element `id`, used by `<use>` and `url(#…)` references
- Owned by `Arc<SvgRenderTree>` — built in the integration layer, consumed by the engine

---

## 3. Rendering Pipeline

[INSERT RENDERING PIPELINE DIAGRAM IMAGE]

Two stages:

1. **Tree construction** — `build_svg_render_tree()` (`components/layout/svg/mod.rs`), called from `ReplacedContents::svg_kind_size()` in `components/layout/replaced.rs`
2. **Rendering** — `render_svg_tree()` (`components/svg_engine/src/traversal.rs`), called from `DisplayListBuilder::visit_image()` in `components/layout/display_list/mod.rs`
