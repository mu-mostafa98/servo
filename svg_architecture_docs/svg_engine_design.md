# SVG Engine Design

## Introduction

The SVG engine is a self-contained rendering crate that integrates with the Servo layout system, converting SVG document fragments into WebRender display items for the GPU compositor. It implements the SVG 2 rendering model as defined in the [Rendering Model](https://www.w3.org/TR/SVG2/render.html) chapter of the specification.

## Table of Contents

1. [System Context](#1-system-context)
2. [Data Model](#2-data-model)
3. [Element Types](#3-element-types)
4. [Architecture Overview](#4-architecture-overview)
5. [Crate Structure](#7-crate-structure)
6. [Implementation Scope](#8-implementation-scope)
7. [Implementation Roadmap](#9-implementation-roadmap)
8. [Architectural Decisions](#10-architectural-decisions)

## 1. System Context

The SVG engine occupies a defined position in the rendering pipeline, acting as a transformation layer between layout and GPU compositing.

### Components

```
┌──────────────────────────┐     ┌──────────────────────────┐     ┌──────────────────────────┐
│    Layout Integration    │     │       svg_engine         │     │        WebRender         │
│      (replaced.rs)       │     │                          │     │                          │
│                          │     │  Resolves geometry       │     │  GPU compositor          │
│  Walks SVG DOM tree      │ ──► │  Manages render tree     │ ──► │  Renders pixels          │
│  Extracts attributes     │     │  Dispatches commands     │     │  to screen               │
│  Reads ComputedValues    │     │                          │     │                          │
│                          │     │                          │     │                          │
└──────────────────────────┘     └──────────────────────────┘     └──────────────────────────┘
```

### Data Flow

1. **Layout Integration → svg_engine** — DOM attributes (coordinates, radii, path data) and resolved `ComputedValues` (fill, stroke, opacity, stroke-width, etc.)
2. **svg_engine → WebRender** — Display primitives (`push_rect`, `push_border`, `push_reference_frame`, clip definitions)

### Boundaries

- **Layout Integration** — provides fully resolved computed styles from Servo's style system (Stylo). All style values have been computed and cascaded before reaching this layer.
- **svg_engine** — owns the rendering tree and the complete render pipeline. Has no direct access to the DOM or style system. All input arrives as typed data structures.
- **WebRender** — receives only display primitives. The engine never inspects WebRender internals or manages GPU resources directly.

## 2. Data Model

The data model is organized as a rendering tree that mirrors the hierarchical structure of SVG elements. Each node in the tree stores only what is unique to its element, while inherited state (transform, clip path, mask) is tracked externally during tree traversal.

### 2.1 SvgRenderTree

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

### 2.2 SvgRenderNode

`SvgRenderNode` represents a single element instance in the rendering tree. Nodes form a recursive tree structure through the `children` field. Each node stores only the data intrinsic to its element — element type, geometry, fill, and stroke parameters.

```
struct SvgRenderNode {
    tag: SvgTag,
    effects: NodeEffects or null,
    fill: FillParams,
    stroke: StrokeParams,
    hints: RenderHints,
    opacity: float,
    visibility: Visibility,
    display: Display,
    paint_order: PaintOrder,
    children: list of SvgRenderNode,
}
```

| Field | Type | Description |
|---|---|---|
| `tag` | `SvgTag` | Element type discriminant — for shapes, carries the geometry data inline |
| `effects` | `NodeEffects or null` | Per-element transform, clip-path, and mask; `null` when none are set |
| `fill` | `FillParams` | Fill color, opacity, and fill rule |
| `stroke` | `StrokeParams` | Stroke color, width, opacity, linecap, linejoin, dash parameters |
| `hints` | `RenderHints` | Vector effect and rendering quality flags |
| `opacity` | `float` | Object opacity for this element (0.0–1.0) |
| `visibility` | `Visibility` | Whether this element is painted; element remains in rendering tree even when hidden |
| `display` | `Display` | Whether this element is rendered; `none` skips rendering, but element remains available for `url(#id)` resolution |
| `paint_order` | `PaintOrder` | Order of fill, stroke, and markers painting operations |
| `children` | `list of SvgRenderNode` | Child nodes in the rendering tree |

### 2.3 RenderState

Inherited rendering parameters (transform, clip path, mask) are not stored on individual nodes. Instead, they are accumulated in a `RenderState` struct that the renderer maintains as it walks the tree. When the renderer enters a container node (such as `<g>` or `<svg>`), it pushes the node's effect onto the render state. When it exits, it pops the previous state.

```
struct RenderState {
    current_transform: Transform3D,
    current_clip_path: ClipPathId or null,
    current_clip_rule: ClipRule,
    current_mask: MaskId or null,
    current_filter: FilterId or null,
}
```

| Field | Description |
|---|---|
| `current_transform` | Accumulated transform matrix applied to all child geometry |
| `current_clip_path` | Active clip path, if any, inherited by child elements |
| `current_clip_rule` | Fill rule for the active clip path (`nonzero` or `evenodd`) |
| `current_mask` | Active mask, if any, inherited by child elements |
| `current_filter` | Active filter effect, if any, applied to the current compositing group |

This separation keeps each node lightweight and makes inheritance logic explicit in a single location (the tree walk loop) rather than duplicated across every renderer.

## 3. Element Types

This section defines the value types stored in the rendering tree nodes. Element types fall into three categories: type discriminants (SvgTag), shape geometry (Geometry), and rendering parameters (FillParams, StrokeParams, RenderHints, NodeEffects).

### 3.1 SvgTag

`SvgTag` is a hierarchical enum that identifies the SVG element type and determines how the renderer processes each node.

```
enum SvgTag {
    Shape(Geometry),
    Container(ContainerTag),
    Text,
    Use,
    ClipMask,
    PaintServer(PaintServerTag),
    Defs,
    Unknown,
}
```

#### Geometry

`Geometry` stores shape-specific coordinate data carried by the `Shape` variant of `SvgTag`. The renderer matches on the geometry variant to determine both the element type and its coordinates in a single dispatch. All numeric fields use `SvgLength` (pixel or percentage), resolved to concrete values during the tree walk using the viewport dimension as the reference length.

```
enum Geometry {
    Rect    { x, y, width, height, rx, ry: optional SvgLength },
    Circle  { cx, cy, r: SvgLength },
    Ellipse { cx, cy, rx, ry: SvgLength },
    Line    { x1, y1, x2, y2: SvgLength },
    Polyline { points: list of Point },
    Polygon  { points: list of Point },
    Path    { segments: list of PathSegment },
}
```

| Variant | Used by | Description |
|---|---|---|
| `Rect` | `<rect>` | Rectangle with optional corner radii |
| `Circle` | `<circle>` | Circle defined by center and radius |
| `Ellipse` | `<ellipse>` | Ellipse defined by center and two radii |
| `Line` | `<line>` | Line segment defined by two endpoints |
| `Polyline` | `<polyline>` | Connected line segments from a point list |
| `Polygon` | `<polygon>` | Closed shape from a point list |
| `Path` | `<path>` | Arbitrary path composed of segments (moves, lines, curves) |

#### ContainerTag

Grouping elements that hold child nodes and establish rendering context boundaries. Children inherit the container's `transform`, `clip-path`, and `mask` through the RenderState.

```
enum ContainerTag {
    // Foundation
    G, Svg,
    // Enhancement
    A, Switch, ForeignObject,
}
```

#### Other variants

| Variant | Phase | Description |
|---|---|---|
| `Text` | Enhancement | Text content elements (`text`, `tspan`, `textPath`) |
| `Use` | Enhancement | Re-used graphics via `url(#id)` reference |
| `ClipMask` | Enhancement | Clip path and mask definitions |
| `PaintServer` | Enhancement | Gradient and pattern definitions (`linearGradient`, `radialGradient`, `pattern`) |
| `Defs` | Foundation | Definition container — children are stored for `url(#id)` resolution, not rendered |
| `Unknown` | Future | Elements that have no rendering effect and are silently skipped |

### 3.2 FillParams

Fill parameters control how the interior of a shape is painted.

```
struct FillParams {
    color: Color or null,
    opacity: float,
    fill_rule: FillRule,
}
```

| Field | Description |
|---|---|
| `color` | Fill paint color. `null` means no fill operation. |
| `opacity` | Fill opacity in the range 0.0–1.0. Multiplied with `color.alpha` at render time. |
| `fill_rule` | Determines interior determination for complex shapes (`nonzero` or `evenodd`). |

### 3.3 StrokeParams

Stroke parameters control how the outline of a shape is painted.

```
struct StrokeParams {
    color: Color or null,
    width: float,
    opacity: float,
    linecap: LineCap,
    linejoin: LineJoin,
    miterlimit: float,
    dasharray: list of float or null,
    dashoffset: float,
}
```

| Field | Description |
|---|---|
| `color` | Stroke paint color. `null` means no stroke operation. |
| `width` | Stroke width in user units. |
| `opacity` | Stroke opacity in the range 0.0–1.0. Multiplied with `color.alpha` at render time. |
| `linecap` | Shape of line endpoints (`butt`, `round`, `square`). |
| `linejoin` | Shape of line join corners (`miter`, `round`, `bevel`). |
| `miterlimit` | Maximum miter length as a multiple of stroke width. |
| `dasharray` | Dash pattern definition. `null` means solid stroke. |
| `dashoffset` | Offset into the dash pattern at the start of the stroke. |

### 3.4 RenderHints

Rendering hints control quality-versus-performance tradeoffs. They do not affect correctness but may influence anti-aliasing, color precision, and image resampling quality.

```
struct RenderHints {
    vector_effect: VectorEffect,
    color_interpolation: ColorInterpolation,
    color_rendering: ColorRendering,
    shape_rendering: ShapeRendering,
    text_rendering: TextRendering,
    image_rendering: ImageRendering,
}
```

| Field | Description |
|---|---|
| `vector_effect` | Controls stroke behavior under transform (`none` or `non-scaling-stroke`). |
| `color_interpolation` | Colorspace for gradient interpolation and compositing operations. |
| `color_rendering` | Quality versus speed for color precision. |
| `shape_rendering` | Quality versus speed for shape anti-aliasing and geometry precision. |
| `text_rendering` | Quality versus speed for text glyph rendering. |
| `image_rendering` | Quality versus speed for image resampling algorithms. |

### 3.5 NodeEffects

Per-element effect parameters that modify how an element is rendered. These are the source values carried by each node, distinct from the accumulated `RenderState` (Section 2.3) which tracks the active effects during tree traversal.

```
struct NodeEffects {
    transform: Transform or null,
    clip_path: ClipPathId or null,
    mask: MaskId or null,
}
```

| Field | Description |
|---|---|
| `transform` | Element-local transform matrix. `null` when no `transform` attribute is set. When present, this matrix is composed with the parent `RenderState.current_transform` during tree walk. |
| `clip_path` | Reference to a `<clipPath>` element definition. `null` when no `clip-path` property is set. |
| `mask` | Reference to a `<mask>` element definition. `null` when no `mask` property is set. |

These three properties are stored as per-element source values rather than being resolved by the style system because they require tree-walk accumulation. The `transform` attribute on a `<g>` element must be composed with child transforms. The `clip-path` and `mask` properties reference definitions by `url(#id)`, resolved during rendering rather than style computation.

## 4. Architecture Overview

The SVG engine is organized around a core tree-walk loop that traverses the rendering tree, resolves geometry values, manages inherited render state, and dispatches display commands. The architecture separates concerns into four layers, each with a single responsibility.

```
┌──────────────────────────────────────────────────────────────┐
│                   Tree Walker (render.rs)                      │
│  Recursively traverses SvgRenderTree, maintains RenderState   │
│  push/pop per container node                                  │
├──────────────────────────────────────────────────────────────┤
│  ┌────────────────────┐  ┌──────────────────────────────────┐ │
│  │ Geometry Resolution│  │     Render Dispatch              │ │
│  │ Resolves SvgLength │  │  Matches tag → render function   │ │
│  │ → pixel values     │  │  Calls push_rect, push_border... │ │
│  │ per viewport       │  │  Delegates to shape renderers    │ │
│  └────────────────────┘  └──────────────────────────────────┘ │
├──────────────────────────────────────────────────────────────┤
│                  RenderState Management                        │
│  current_transform, current_clip_path, current_mask,          │
│  current_filter — pushed on container enter, popped on exit  │
├──────────────────────────────────────────────────────────────┤
│                  Shape Renderers (render.rs)                   │
│  render_rect(), render_circle(), render_ellipse(),             │
│  render_line(), render_path(), render_polygon()               │
└──────────────────────────────────────────────────────────────┘
```

### 4.1 Tree Walker

The tree walker is the central loop of the engine. It receives an `SvgRenderTree` and processes nodes in depth-first order:

```
walk_node(node, render_state):
    apply node.effects → render_state    // push transform, clip, mask
    resolve node.geometry → concrete values
    dispatch node.tag → render function
    for each child:
        walk_node(child, render_state)
    revert node.effects → render_state   // pop

walk_tree(tree):
    walk_node(tree.root, RenderState::default())
```

For container nodes, the walker:
1. Pushes the container's `transform` (if any) onto `RenderState.current_transform`
2. Pushes the container's `clip-path` and `mask` onto the corresponding state fields
3. Recursively walks children with the updated state
4. Pops all changes when exiting the container

For shape nodes, the walker resolves geometry and dispatches to the matching render function. The current `RenderState` values are passed through to each render call.

### 4.2 Geometry Resolution

Geometry fields use `SvgLength`, which represents either a pixel value or a percentage. Resolution converts these to concrete pixel values using the viewport dimension as the reference length:

```
resolve_length(SvgLength::Px(px), reference) → px
resolve_length(SvgLength::Percent(p), reference) → p * reference
```

Each shape variant determines which dimensions serve as the reference:
- Width-based geometry (`x`, `width`, `rx` for rects) resolves against `viewport.width`
- Height-based geometry (`y`, `height`, `ry` for rects) resolves against `viewport.height`
- Circle radius resolves against `min(viewport.width, viewport.height)`

### 4.3 Render Dispatch

The render dispatch matches the element tag to the appropriate rendering function. Shape elements select a renderer based on the `Geometry` variant:

```
match tag {
    Shape(Geometry::Rect { ... })    → render_rect(...)
    Shape(Geometry::Circle { ... })  → render_circle(...)
    Shape(Geometry::Ellipse { ... }) → render_ellipse(...)
    Shape(Geometry::Line { ... })    → render_line(...)
    Shape(Geometry::Path { ... })    → render_path(...)
    Shape(Geometry::Polyline { ... })→ render_polyline(...)
    Shape(Geometry::Polygon { ... }) → render_polygon(...)
    Container(_)                     → children are walked recursively
    Defs                             → children not rendered, available for url(#id)
    _                                → silently skipped (Enhancement/Future)
}
```

### 4.4 Extensibility

Adding a new element type follows a defined pattern across the architecture:

| Step | Component | Action |
|---|---|---|
| 1 | SvgTag hierarchy | Add the variant to the appropriate sub-enum |
| 2 | Geometry (if shape) | Add the variant with coordinate fields |
| 3 | Render dispatch | Add the match arm in the dispatch function |
| 4 | Render strategy | Implement the WebRender primitive mapping |

This pattern allows Foundation shapes (rect, circle, line) to be implemented first, with Enhancement elements (text, use, gradients) added later by repeating steps 1–4 without modifying existing code.
