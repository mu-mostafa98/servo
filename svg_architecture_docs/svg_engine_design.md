# SVG Engine Design

## Introduction

The SVG engine is a self-contained rendering crate that integrates with the Servo layout system, converting SVG document fragments into WebRender display items for the GPU compositor. It implements the SVG 2 rendering model as defined in the [Rendering Model](https://www.w3.org/TR/SVG2/render.html) chapter of the specification.

## Table of Contents

1. [System Context](#1-system-context)
2. [Data Model](#2-data-model)
3. [Element Types](#3-element-types)
4. [Architecture Overview](#4-architecture-overview)
5. [Crate Structure](#5-crate-structure)
6. [Implementation Scope](#6-implementation-scope)
7. [Implementation Roadmap](#7-implementation-roadmap)
8. [Architectural Decisions](#8-architectural-decisions)

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

### 2.1 Class Diagram

A diagram showing the relationships between the core data classes will be inserted here.

<!-- TODO: Replace with hosted image URL — ![Class Diagram](https://example.com/path/to/svg_engine_class_diagram.png) -->

### 2.2 SvgRenderTree

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

### 2.3 SvgRenderNode

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
| `styles` | `NodeStyles` | Bundled rendering parameters — fill, stroke, effects, opacity, hints, visibility, display, and paint order (see Section 3.6) |
| `children` | `list of SvgRenderNode` | Child nodes in the rendering tree |

### 2.4 RenderState

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

### 3.6 NodeStyles

`NodeStyles` bundles all per-element rendering parameters into a single struct carried by each `SvgRenderNode`. This includes paint properties (fill, stroke), rendering hints, opacity, visibility, display mode, paint order, and effects (transform, clip-path, mask).

```
struct NodeStyles {
    fill: FillParams,
    stroke: StrokeParams,
    hints: RenderHints,
    opacity: float,
    visibility: Visibility,
    display: Display,
    paint_order: PaintOrder,
    effects: NodeEffects or null,
}
```

| Field | Type | Description |
|---|---|---|
| `fill` | `FillParams` | Fill color, opacity, and fill rule |
| `stroke` | `StrokeParams` | Stroke color, width, opacity, linecap, linejoin, dash parameters |
| `hints` | `RenderHints` | Vector effect and rendering quality flags |
| `opacity` | `float` | Object opacity for this element (0.0–1.0) |
| `visibility` | `Visibility` | Whether this element is painted; element remains in rendering tree even when hidden |
| `display` | `Display` | Whether this element is rendered; `none` skips rendering, but element remains available for `url(#id)` resolution |
| `paint_order` | `PaintOrder` | Order of fill, stroke, and markers painting operations |
| `effects` | `NodeEffects or null` | Per-element transform, clip-path, and mask (Section 3.5); `null` when none are set |

The `effects` field is kept as a separate inner struct (`NodeEffects`) rather than being flattened into `NodeStyles`. This separation reflects their different role during tree traversal: effects (transform, clip-path, mask) are pushed onto and popped from `RenderState` as the tree walker enters and exits containers, while paint properties (fill, stroke) are consumed directly by each node's render function.

## 4. Architecture Overview

The SVG engine is organized around a core tree-walk loop that traverses the rendering tree, resolves geometry values, manages inherited render state, and dispatches display commands. The architecture separates concerns into four layers.

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
    apply node.styles.effects → render_state    // push transform, clip, mask
    resolve node.geometry → concrete values
    dispatch node.tag → render function
    for each child:
        walk_node(child, render_state)
    revert node.styles.effects → render_state   // pop

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

Within the shape renderers, `render_circle` delegates to `render_ellipse` by constructing an `Ellipse` geometry with equal radii (`rx = ry = r`). This avoids duplicating the ellipse fill/stroke logic and reflects the geometric relationship that a circle is an ellipse with uniform radius. The dispatch function routes both Circle and Ellipse variants to separate entry points — the delegation is an internal detail of `render_circle`.

Note: `render_path`, `render_polyline`, and `render_polygon` are listed in the dispatch table as the intended targets but are not yet implemented (Phase 2). The current dispatch skips these variants with a no-op. The same extensibility pattern in Section 4.4 applies when they are added.

### 4.4 Extensibility

Adding a new element type follows a defined pattern across the architecture:

| Step | Component | Action |
|---|---|---|
| 1 | SvgTag hierarchy | Add the variant to the appropriate sub-enum |
| 2 | Geometry (if shape) | Add the variant with coordinate fields |
| 3 | Render dispatch | Add the match arm in the dispatch function |
| 4 | Render strategy | Implement the WebRender primitive mapping |

This pattern allows Foundation shapes (rect, circle, line) to be implemented first, with Enhancement elements (text, use, gradients) added later by repeating steps 1–4 without modifying existing code.

## 5. Crate Structure

The `svg_engine` crate (`components/svg_engine/src/`) is organized into focused modules, each with a single responsibility. The crate root (`lib.rs`) declares all modules and re-exports only the types and functions needed by external consumers.

```
svg_engine/src/
├── lib.rs          Crate root — module declarations, public re-exports
├── extract.rs      Extract params from Stylo ComputedValues + DOM attributes
├── lengths.rs      SvgLength type — pixel and percentage resolution
├── path.rs         SVG path data (d attribute) parser
├── points.rs       SVG points attribute parser
├── render.rs       Tree walk, geometry resolution, shape rendering, WebRender dispatch
├── shapes.rs       Element types (SvgTag, Geometry), tree types (SvgRenderNode, SvgRenderTree)
├── styles.rs       Style parameter types (FillParams, StrokeParams, NodeStyles, etc.)
└── transform.rs    SVG transform attribute parser
```

### 5.1 Module Responsibilities

| Module | Responsibility |
|---|---|
| `lib.rs` | Declares all modules; re-exports the public API surface (`SvgRenderInput`, `SvgTag`, `render_svg_element`, all `extract_*` functions, `SvgLength`, `parse_transform`) |
| `extract` | Reads resolved `ComputedValues` from Stylo to produce `FillParams`, `StrokeParams`, `RenderHints`; reads DOM attributes via a closure to produce `Geometry` and `NodeEffects`. Acts as the bridge between layout and the engine's internal types. |
| `lengths` | Defines `SvgLength` (pixel or percentage) with a `resolve()` method that converts to concrete pixel values against a reference dimension. |
| `path` | Parses the SVG path `d` attribute into a `BezPath` (kurbo) for `<path>` elements. |
| `points` | Parses the `points` attribute into a `Vec<KurboPoint>` for `<polyline>` and `<polygon>` elements. |
| `render` | Entry point `render_svg_element()` receives a scene of `SvgRenderInput` items and a WebRender `DisplayListBuilder`. Contains all shape renderers (`render_rect`, `render_circle`, `render_ellipse`, `render_line`), geometry resolution functions (`resolve_rect`, `resolve_circle`, etc.), and clip-chain helpers. |
| `shapes` | Core type definitions: `SvgTag` (element discriminant), `Geometry` (shape coordinate data), `ContainerTag`, `PaintServerTag`, `SvgRenderNode` (recursive tree node), `SvgRenderInput` (flat input for layout integration), `SvgRenderTree`, `ViewportInfo`. |
| `styles` | All style-related parameter types: `FillParams`, `StrokeParams`, `RenderHints`, `NodeEffects`, `NodeStyles`, plus supporting enums (`FillRule`, `SvgLineCap`, `SvgLineJoin`, `VectorEffect`, `Visibility`, `Display`, `PaintOrder`). |
| `transform` | Parses the SVG `transform` attribute string into a `Transform2D`. |

### 5.2 Module Dependencies

Module dependencies within the crate form a directed acyclic graph:

```
lib.rs
  ├── extract → {shapes, styles, lengths, path, points, transform}
  ├── lengths
  ├── path
  ├── points
  ├── render → {shapes, styles}
  ├── shapes → {styles}
  ├── styles
  └── transform
```

- `shapes` depends on `styles` because `SvgRenderNode` and `SvgRenderInput` embed style types (`FillParams`, `StrokeParams`, `NodeStyles`, etc.).
- `extract` depends on most other modules because it constructs all engine types from external input (ComputedValues + DOM attributes).
- `render` depends on `shapes` and `styles` because it receives `SvgRenderInput` and accesses `FillParams`/`StrokeParams` for rendering.
- `styles` and `lengths` are leaf modules with no internal dependencies.

### 5.3 Public API Surface

The crate exposes a minimal public API through `lib.rs`, limited to what the layout crate needs:

| Export | Source module | Used by |
|---|---|---|
| `SvgRenderInput` | shapes | layout/replaced.rs — scene construction |
| `SvgTag` | shapes | layout/replaced.rs — element type routing |
| `render_svg_element` | render | layout/display_list — WebRender dispatch |
| `extract_fill_params` | extract | layout/replaced.rs — style extraction |
| `extract_stroke_params` | extract | layout/replaced.rs — style extraction |
| `extract_geometry` | extract | layout/replaced.rs — attribute extraction |
| `extract_effects` | extract | layout/replaced.rs — attribute extraction |
| `extract_opacity` | extract | layout/replaced.rs — style extraction |
| `extract_render_hints` | extract | layout/replaced.rs — style extraction |
| `extract_visibility` | extract | layout/replaced.rs — style extraction |
| `SvgLength` | lengths | layout/replaced.rs — attribute parsing |
| `parse_transform` | transform | layout/replaced.rs — attribute parsing |

All internal types (`FillParams`, `StrokeParams`, `NodeStyles`, `Geometry`, `ContainerTag`, etc.) remain accessible within the crate but are intentionally omitted from the public re-exports to keep the API surface minimal and allow internal restructuring without affecting external consumers.

## 6. Implementation Scope

SVG features are organized into three tiers: Foundation (implemented), Enhancement (next), and Future (deferred).

### 6.1 Foundation (Current)

| Feature | Status | Notes |
|---|---|---|
| `<rect>` | Implemented | Fill via `push_rect`, stroke via `push_border`, rounded corners via clip chain |
| `<circle>` | Implemented | Delegates to ellipse renderer with `rx = ry = r` |
| `<ellipse>` | Implemented | Fill via rounded clip + `push_rect`, stroke via `push_border` |
| `<line>` | Implemented | Horizontal/vertical via thin rect, diagonal via rotated reference frame |
| `<svg>` container | Implemented | Establishes viewport, children walked in dispatch |
| `<g>` container | Implemented | No-op render, children walked recursively |
| Fill rendering | Implemented | Solid color fill with opacity |
| Stroke rendering | Implemented | Solid color stroke via border, basic linecap/linejoin |
| Geometry resolution | Implemented | `SvgLength` → pixel values against viewport |
| Style extraction | Implemented | `extract.rs` reads ComputedValues + DOM attributes |
| Layout integration | Implemented | Flat `SvgRenderInput` list built in `replaced.rs` |

### 6.2 Enhancement (Phase 2)

| Feature | Dependencies | Notes |
|---|---|---|
| `<path>` | Path parser (ready) | Needs software rasterization or tessellation |
| `<polyline>` / `<polygon>` | Points parser (ready) | Same rasterization requirement as path |
| `<a>`, `<switch>`, `<foreignObject>` | Container support | Render-state inheritance only, no special rendering |
| `SvgRenderTree` walker | Enhancement | Replace flat `SvgRenderInput` with recursive tree walk |
| `RenderState` management | Enhancement | Accumulate transform, clip, mask during tree walk |
| Clip paths (`<clipPath>`) | Enhancement | Requires url(#id) resolution into clip regions |
| Masks (`<mask>`) | Enhancement | Requires render-to-texture or equivalent |
| Transform attribute | Enhancement | Parser ready, needs composition in RenderState |
| Paint servers (gradients) | Enhancement | Requires gradient definition resolution and WebRender gradient display items |
| `defs` / `url(#id)` resolution | Enhancement | Reference target lookup table |

### 6.3 Enhancement (Phase 3)

| Feature | Notes |
|---|---|
| `<text>`, `<tspan>`, `<textPath>` | Requires text layout integration |
| `<use>` element | Requires url(#id) clone-with-override pattern |
| Markers (`<marker>`) | Requires url(#id) resolution + placement at path vertices |
| Filters (`<filter>`) | Requires render-to-texture and compositing |

### 6.4 Out of Scope

| Feature | Rationale |
|---|---|
| SVG animations (`<animate>`, `<set>`) | Handled by Servo's animation system, not the render engine |
| Scripted SVG DOM manipulation | Handled by Servo's script layer |
| Full SVG 2 conformance gaps | Addressed as-needed; no goal of 100% SVG 2 compliance |

## 7. Implementation Roadmap

The engine follows the ordered plan established by the project manager: (1) Stylo property support, (2) Presentation attribute wiring, (3) Phase 1 engine.

### 7.1 Milestones

| Phase | Scope | Dependencies | Status |
|---|---|---|---|
| **Stylo PR** | Add SVG style structs to Stylo (fill, stroke, opacity, etc.) | — | Pending |
| **Presentation Attributes PR** | Wire SVG attributes (fill="red" → CSS fill property) | Stylo PR | Pending |
| **Phase 1** | Core shape rendering with flat input list (current codebase) | Pres. Attrs PR | In progress |
| **Phase 2** | Path rendering, tree walker, clip/mask, gradients, transforms | Phase 1 | Planned |
| **Phase 3** | Text, use, markers, filters | Phase 2 | Planned |

### 7.2 Phase 1 — Core Shape Rendering

The current implementation delivers:
- Four basic shapes (rect, circle, ellipse, line) with fill and stroke
- Layout integration via `replaced.rs` constructing `Vec<SvgRenderInput>`
- WebRender display items: `push_rect`, `push_border`, `define_clip_rounded_rect`
- `extract.rs` bridge from Stylo `ComputedValues` to engine types

**Excluded from Phase 1** (deferred to Phase 2):
- `<path>`, `<polyline>`, `<polygon>` — require tessellation or software rasterization
- `SvgRenderTree` / recursive tree walk — currently using flat `SvgRenderInput`
- Clip paths, masks, filters
- Paint servers (gradients, patterns)

### 7.3 Phase 2 — Path Rendering and Effects

Planned additions:
- Path tessellation → WebRender polygon display items
- Recursive `SvgRenderTree` walker with `RenderState` push/pop
- Clip path support via `define_clip_rounded_rect` / `define_clip`
- Transform support via `push_reference_frame`
- Linear and radial gradient support
- `defs` reference resolution table

### 7.4 Phase 3 — Text, Use, and Advanced Features

Planned additions:
- Text content layout and glyph rendering
- `<use>` element with reference cloning
- Marker rendering at path vertices
- Filter effects via offscreen surfaces

## 8. Architectural Decisions

### 8.1 Pure WebRender Primitives

**Decision:** All shapes render through `push_rect`, `push_border`, and clip chains. No software rasterization or intermediate bitmap.

**Rationale:** Eliminates the serialization–resvg–rasterization overhead of the previous approach. WebRender's GPU compositor handles all pixel output. The trade-off is that shapes without native WebRender primitives (path, polyline, polygon) require tessellation before they can be rendered.

**Status:** Applied — rect, circle, ellipse, line use this approach. Path/polyline/polygon deferred to Phase 2.

### 8.2 Separated styles.rs Module

**Decision:** Style parameter types (`FillParams`, `StrokeParams`, `NodeStyles`, etc.) live in a dedicated `styles.rs` module, separate from element types (`shapes.rs`) and extraction logic (`extract.rs`).

**Rationale:** Keeps type definitions, extraction, and usage in separate files — each has a single reason to change. Prevents `shapes.rs` from becoming a dumping ground for unrelated type definitions.

**Status:** Applied.

### 8.3 NodeStyles Bundling

**Decision:** All per-node rendering parameters are bundled into a single `NodeStyles` struct, rather than stored as individual fields on `SvgRenderNode`.

**Rationale:** Reduces `SvgRenderNode` boilerplate (3 fields vs. 9), groups related data into a single concept, and simplifies the struct signature. `NodeEffects` remains a separate nested struct because effects (transform, clip, mask) behave differently during tree traversal — they are pushed/popped in `RenderState`, not consumed per-node like fill/stroke.

**Status:** Applied.

### 8.4 Circle Delegates to Ellipse

**Decision:** `render_circle` converts its `Circle` geometry to an `Ellipse` geometry with equal radii and delegates to `render_ellipse`.

**Rationale:** A circle is geometrically an ellipse with uniform radius. Delegation avoids duplicating the fill/stroke rendering logic. The dispatch function still routes Circle and Ellipse to separate entry points — the delegation is an internal detail of `render_circle`.

**Status:** Applied.

### 8.5 Flat Input List for Phase 1

**Decision:** Phase 1 uses a flat `Vec<SvgRenderInput>` built by `replaced.rs` during DOM traversal, rather than a recursive `SvgRenderTree` walk.

**Rationale:** Simpler initial integration — layout walks the DOM once, builds render inputs, and hands them off. The recursive tree walker with `RenderState` push/pop is deferred to Phase 2 when clip paths, masks, and transforms require it.

**Status:** Temporary — will be replaced by `SvgRenderTree` walk in Phase 2.

### 8.6 SvgLength Resolution at Render Time

**Decision:** Geometry coordinates are stored as `SvgLength` (pixel or percentage) and resolved to concrete pixel values at render time against the viewport dimensions.

**Rationale:** Percentage values cannot be resolved until the viewport is known, which happens during layout. Storing unresolved lengths in the geometry allows a single resolution pass during rendering. This avoids storing both the original attribute string and the resolved value.

**Status:** Applied.

### 8.7 extract.rs as Style Bridge

**Decision:** A dedicated `extract.rs` module reads both `ComputedValues` (from Stylo) and DOM attributes (via a closure) to produce engine types. It acts as the single bridge between Servo's style system and the SVG engine.

**Rationale:** Centralizes all style-to-engine conversion in one place. When new properties are added to Stylo, only `extract.rs` needs updating — the render pipeline and type definitions remain unchanged.

**Status:** Applied.
