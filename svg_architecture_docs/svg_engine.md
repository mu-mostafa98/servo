# SVG Engine Design

## Introduction

The SVG engine is a self-contained rendering crate that integrates with the Servo layout system, converting SVG document fragments into WebRender display items for the GPU compositor.

## Table of Contents

1. [System Context](#1-system-context)
2. [Architecture Overview](#2-architecture-overview)
3. [Data Model](#3-data-model)
4. [Crate Structure](#4-crate-structure)
5. [Development Timeline](#5-development-timeline)

## 1. System Context

The SVG engine occupies a defined position in the rendering pipeline, acting as a transformation layer between layout and GPU compositing.

### Components

![Components](svg-engine-system-context.jpg)

### Boundaries

- **Layout Integration** — provides fully resolved computed styles from Servo's style system (Stylo). Iterates the DOM tree to build an `SvgRenderTree`, using helper functions from the SVG engine to extract styles and geometry from Stylo `ComputedValues` and DOM attributes.
- **svg_engine** — owns the rendering tree and the complete render pipeline. Has no direct access to the DOM or style system. All input arrives as typed data structures.
- **WebRender** — receives only display primitives. The engine never inspects WebRender internals or manages GPU resources directly.

## 2. Architecture Overview

The SVG engine is organized around a core tree-walk loop that traverses the rendering tree, resolves geometry values, manages inherited render state, and dispatches display commands. The architecture separates concerns into four layers:

### Architecture

![Architecture](svg-engine-architecture.jpg)

### Tree Walker (Core)

The tree walker is the central loop of the engine. It receives an `SvgRenderTree` and processes nodes in depth-first order:

### Pseudocode
```
render_node(node, render_state):
    apply node.styles.effects → render_state    // push transform, clip, mask
    resolve node.geometry → concrete values
    dispatch node.tag → render function
    for each child:
        render_node(child, render_state)
    revert node.styles.effects → render_state   // pop

render_svg_element(tree):
    render_node(tree.root, RenderState::default())
```

### render_node Flowchart

![render_node Flowchart](svg-engine-tree-walker.jpg)

## 3. Data Model

The data model is organized as a rendering tree that mirrors the hierarchical structure of SVG elements. Each node in the tree stores only what is unique to its element, while inherited state (transform, clip path, mask) is tracked externally during tree traversal.

### Class Diagram

![Class Diagram](svg-engine-class-diagram.jpg)

### SvgRenderTree

`SvgRenderTree` is the top-level container for a single SVG document fragment. It holds the root of the rendering tree and viewport information derived from the element's attributes and the `viewBox`.

```
struct SvgRenderTree {
    root: SvgRenderNode,
    viewport: ViewportInfo,
}
```

| Field | Type | Description |
|---|---|---|
| `root` | `SvgRenderNode` | Root of the rendering tree, corresponding to the outermost `<svg>` element |
| `viewport` | `ViewportInfo` | Resolved viewport dimensions from `width`, `height`, and `viewBox` attributes |

### SvgRenderNode

`SvgRenderNode` represents a single element instance in the rendering tree. Nodes form a recursive tree structure through the `children` field. Each node stores only the data intrinsic to its element — element type, a bundled `NodeStyles` struct with all rendering parameters, and child nodes.

```
struct SvgRenderNode {
    tag: SvgTag,
    styles: NodeStyles,
    children: list of SvgRenderNode,
}
```

| Field | Type | Description |
|---|---|---|
| `tag` | `SvgTag` | Element type discriminant — for shapes, carries the geometry data inline |
| `styles` | `NodeStyles` | Bundled rendering parameters — fill, stroke, effects, opacity, hints, visibility, display, and paint order |
| `children` | `list of SvgRenderNode` | Child nodes in the rendering tree |

## 4. Crate Structure

The `svg_engine` crate (`components/svg_engine/`) is organized into focused modules, each with a single responsibility. The crate root (`src/lib.rs`) declares all modules and re-exports only the types and functions needed by external consumers.

```
svg_engine/
├── src/
|   ├── lib.rs          Crate root — module declarations, public re-exports
│   ├── extract.rs      Bridge layer — public extract_styles() and extract_geometry() convert Stylo types to engine types
│   ├── lengths.rs      SvgLength type — pixel and percentage resolution
│   ├── path.rs         SVG path data (d attribute) parser
│   ├── points.rs       SVG points attribute parser
│   ├── render.rs       Tree walk, geometry resolution, shape rendering, WebRender dispatch
│   ├── shapes.rs       Element types (SvgTag, Geometry), tree types (SvgRenderNode, SvgRenderTree)
│   ├── styles.rs       Style parameter types (FillParams, StrokeParams, NodeStyles, etc.)
│   └── transform.rs    SVG transform attribute parser
└── Cargo.toml          Crate manifest
```

## 5. Development Timeline

**Start date**: Jun 1, 2026 (Monday)  
**End date**: Jul 10, 2026 (Friday)  
**Total**: 30 working days (Mon–Fri)

---

| Phase | Duration | Dates |
|-------|----------|-------|
| **Phase 1** — Stylo PR + Presentation Attributes + Core Shapes (rect, circle, ellipse, line) | 5 days | Jun 1 (Mon) → Jun 5 (Fri) |
| **Phase 2** — Path, Polygon, Polyline | 5 days | Jun 8 (Mon) → Jun 12 (Fri) |
| **Phase 3** — Groups, Transforms, viewBox | 5 days | Jun 15 (Mon) → Jun 19 (Fri) |
| **Phase 4** — SVG Text | 5 days | Jun 22 (Mon) → Jun 26 (Fri) |
| **Phase 5** — ClipPath, Mask | 5 days | Jun 29 (Mon) → Jul 3 (Fri) |
| **Phase 6** — Gradients & Filters | 5 days | Jul 6 (Mon) → Jul 10 (Fri) |

---

