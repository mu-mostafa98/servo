# SVG Rendering Pipeline — `svg_engine`

## 1. Overview

The SVG rendering pipeline: converts `<svg>` embedded in HTML into
WebRender display-list commands.

## 2. Design

Rendering SVG is split into two stages with a single, well-defined
boundary: the document is converted into a data structure first, and that data
structure is then turned into drawing commands. The first stage lives in
`layout`; the second is this crate.

**1. Integration layer — `DOM` → `SvgRenderTree` (in `layout`)**

The `layout` crate owns the boundary between the document and the renderer. Its
`build_svg_render_tree` entry point walks the SVG subtree, resolves CSS
computed values, and collects the `<defs>` resources (gradients, patterns,
and markers) into a pure-data `SvgRenderTree`. The
result is a tree of `SvgRenderNode`s carrying only the geometry and paint
information the renderer needs — no DOM or layout types leak through.

**2. The engine — `SvgRenderTree` → display commands (in `svg_engine`)**

This crate's core is `render_svg_tree`, which walks the `SvgRenderTree`
recursively and emits a WebRender display list. At each node it resolves the
inherited transforms, clips, and paint, then produces the matching primitive.
Shapes are emitted through one of two paths: a native path that pushes
WebRender items directly (`push_rect`, `push_border`, `push_gradient`,
`push_text`, `push_image`) for shapes WebRender can express natively, and a
software path that rasterizes the shape with `vello_cpu` into a
`RasterizedImage` and pushes it as a single image. The split exists because
WebRender cannot natively express arbitrary paths and certain border and
gradient cases, so those fall back to CPU rasterization.

## 3. Input, process, output

At the top level the engine takes an `SvgRenderTree` (built from the DOM by
`layout::svg::build_svg_render_tree`), walks it recursively, and emits WebRender
display-list commands — native primitives (`push_rect`, `push_border`,
`push_gradient`, `push_text`, `push_image`) for shapes WebRender can express
directly, or a `push_image` of a `RasterizedImage` rasterized by `vello_cpu`
(`RasterSink::emit`) for everything else. The table below breaks that down per
SVG element.

| Input | Processing | Output |
|-------|------------|--------|
| `<rect>` | Fill / stroke / gradient painted into its bounds; `rx`/`ry` corner radii become a rounded-rect clip chain | `push_rect` / `push_border` / `push_gradient` |
| `<circle>` | Delegates to `<ellipse>` (→ `<rect>`): converted to a rect with `rx = ry = r` | `push_rect` / `push_border` / `push_gradient` |
| `<ellipse>` | Delegates to `<rect>`: converted to a rect with `rx`/`ry` radii | `push_rect` / `push_border` / `push_gradient` |
| `<line>` | Solid/gradient stroke emitted as a rotated segment | `stroke_line_segment` |
| `<path>` | Converted to a `BezPath`, then `vello_cpu` rasterizes it into an RGBA pixmap | `RasterizedImage` → `push_image` |
| `<polyline>` | Converted to an open `BezPath`, then `vello_cpu` rasterizes it into an RGBA pixmap | `RasterizedImage` → `push_image` |
| `<polygon>` | Converted to a closed `BezPath`, then `vello_cpu` rasterizes it into an RGBA pixmap | `RasterizedImage` → `push_image` |
| `<image>` | `href` resolved to an `ImageKey` at build time; `preserveAspectRatio` fits the natural size into the viewport, then drawn with `push_image` (gray placeholder if not loaded) | `push_image` |
| `<text>` | Applies `text-anchor`/RTL alignment and fill/stroke, then emits glyphs grouped by font | `push_text` |
| `<tspan>` | Applies fill/stroke and emits its glyphs inline within the `<text>` line | `push_text` |

## 4. Scope

`svg_engine` renders embedded `<svg>` content directly into the WebRender
display list, resolved through the same CSS cascade as the rest of the page.

**Biggest feature — CSS cascade and external styles.** Styles are resolved
through Stylo's `ComputedValues`, so external stylesheets, inheritance, and the full cascade apply to SVG exactly as they do to HTML. The
old pipeline serialized the `<svg>` subtree to a string and rasterized it into a
single bitmap, so it could only preserve inline presentation attributes.

**Supported elements**

- **Shapes** — `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>`
- **Structure** — `<g>`, `<defs>`, `<use>`, `<symbol>`
- **Paint** — fill and stroke; `<linearGradient>`, `<radialGradient>`, `<stop>`, `<pattern>`
- **Text** — `<text>`, `<tspan>` (`text-anchor`, `dominant-baseline`, `dx`/`dy`/`rotate`, RTL)
- **Image** — `<image>`
- **Markers** — `<marker>`

## 5. Implementation

### 5.1 System boundaries

`svg_engine` exposes a single interface — `render_svg_tree` — which takes an
`SvgRenderTree` and emits a WebRender display list.

```mermaid
flowchart LR
    subgraph LAYOUT["layout (integration layer)"]
        DOM["DOM + Stylo<br/>computed values"]
    end

    subgraph ENGINE["svg_engine (this crate)"]
        RST["render_svg_tree"]
    end

    subgraph WR["WebRender"]
        DL["display list"]
    end

    DOM -->|"SvgRenderTree"| RST
    RST -->|"display-list commands"| DL
```

### 5.2 Architecture

One traversal fans out into two rendering paths: simple shapes are pushed to
WebRender natively, and complex shapes are rasterized through `vello_cpu` and
uploaded as images.

```mermaid
flowchart TB
    classDef entry fill:#fff3e0,stroke:#d79b00,color:#6b4b00
    classDef native fill:#e3f2fd,stroke:#0288d1,color:#014361
    classDef vello fill:#fce4ec,stroke:#c2185b,color:#880e4f

    IL["layout — SVG image fragment traversal"]:::entry

    subgraph ENG["SVG Engine — components/svg_engine/"]
        direction TB
        TRAV["Traversal — tree walk & state<br/>(transforms, clips)"]

        subgraph COMPLEX["Complex Shapes Group"]
            direction LR
            POLYLINE["polyline"]:::vello
            POLYGON["polygon"]:::vello
            PATH["path"]:::vello
        end

        subgraph SIMPLE["Simple Shapes Group"]
            direction LR
            RECT["rect"]:::native
            CIRC["circle / ellipse"]:::native
            LINE["line"]:::native
            TEXT["text / tspan"]:::native
            IMG["image"]:::native
        end

        TRAV -->|"complex shapes"| COMPLEX
        TRAV -->|"simple shapes"| SIMPLE
    end

    WR["WebRender / Paint_engine"]:::native
    VELLO["Vello CPU<br/>rasterization scene"]:::vello
    UPLOAD["layout — image cache uploader"]:::vello

    IL -->|"SvgRenderTree"| TRAV
    SIMPLE -->|"push_rect / push_text / … / push_image"| WR
    COMPLEX -->|"BezPath"| VELLO
    VELLO -->|"Pixmap"| COMPLEX
    COMPLEX -->|"hash + data + w + h"| UPLOAD
    UPLOAD -->|"ImageKey"| COMPLEX
    COMPLEX -->|"push_image"| WR
```

### 5.3 Module map

| Module | Responsibility |
|--------|----------------|
| [`render_tree`](src/render_tree.rs) | Data model — tree and node types |
| [`shapes`](src/shapes/mod.rs) | Data model — shape types |
| [`style`](src/style/mod.rs) | Data model — paint and style parameters |
| [`text`](src/text.rs) | Data model — text |
| [`image`](src/image.rs) | Data model — image |
| [`traversal`](src/traversal.rs) | recursive walk |
| [`renderer`](src/renderer/mod.rs) | shape rendering |

### 5.4 Data flow

Rendering is a single recursive pass: `render_svg_tree` sets up the root
viewport, walks every node, and each shape's paint is pushed natively or
rasterized through `vello_cpu`.

#### 5.4.1 Viewport setup

`render_svg_tree` first clips the root viewport and maps `viewBox` into a
reference frame, then starts the walk at the root node.

```mermaid
flowchart LR
    A["render_svg_tree"] --> B["build_viewport_clip<br/>(unless overflow:visible)"]
    B --> C["push_viewbox_frame<br/>(viewBox → reference frame)"]
    C --> D["render_node(root)"]
```

#### 5.4.2 The node walk

`render_node` applies transforms and effects, then dispatches on the node tag:
`Shape` → `emit_geometry`, `Text` / `Image` → `emit_leaf`, and `Container` →
`recurse_children` (which walks each child back through `render_node`). A nested
`<svg>` also pushes a sub-viewport clip and `viewBox` frame before recursing.

```mermaid
flowchart TD
    A["render_node(node)"] --> B{"display: none?"}
    B -- "yes" --> END["skip subtree"]
    B -- "no" --> C["apply_node_transforms"]
    C --> E["resolve_node_effects"]
    E --> G{"node.tag?"}
    G -- "Shape" --> H["emit_geometry(shape)"]
    G -- "Text" --> I["emit_leaf(TextSpan)"]
    G -- "Image" --> J["emit_leaf(SvgImage)"]
    G -- "Container::{Group | Svg | Defs | Use | Symbol | Text}" --> L["recurse_children"]
    L -- "each child" --> A
```

#### 5.4.3 Shapes

The node walk hands `Shape` to `emit_geometry`, which wraps the paint in effects and delegates to `emit_shape`. `emit_shape`
resolves the paint to a native primitive or a `vello_cpu` raster; markers
(`emit_markers`) are emitted afterward on line/polyline/polygon shapes.

```mermaid
flowchart TD
    A["emit_geometry(shape)"] --> B["emit_shape(shape)"]
    B --> C{"fill or stroke?"}
    C -- "pattern / gradient / solid<br/>on basic shapes" --> D["native render<br/>push_gradient / push_rect / push_border / stroke_line_segment"]
    C -- "paths, dashed,<br/>unsupported gradients" --> E["rasterize_bez (vello_cpu)"]
    D --> F["WebRender display list"]
    E --> G["RasterizedImage → push_image"]
    G --> F
```

#### 5.4.4 Text

The node walk hands `Text` to `emit_leaf`, which builds a `RenderContext` and
calls `TextSpan::render`. Real glyphs are drawn with `push_text` when a
`FontInstanceKey` is available; otherwise estimated rectangles are drawn as a
fallback.

```mermaid
flowchart TD
    A["emit_leaf(TextSpan)"] --> B["build RenderContext"]
    B --> C["TextSpan::render"]
    C --> D["stroke? then fill<br/>(paint-order)"]
    D --> E{"font_instance_key?"}
    E -- "yes" --> F["emit_glyphs → push_text<br/>(grouped by font)"]
    E -- "no" --> G["emit_rects → push_rect<br/>(estimated boxes)"]
    F --> H["WebRender display list"]
    G --> H
```

#### 5.4.5 Image

The node walk hands `Image` to `emit_leaf`, which builds a `RenderContext` and
calls `SvgImage::render`. A loaded image is drawn with `push_image` (fitted via
`preserveAspectRatio`); otherwise a placeholder (gray rect with an X) is drawn.

```mermaid
flowchart TD
    A["emit_leaf(SvgImage)"] --> B["build RenderContext"]
    B --> C["SvgImage::render"]
    C --> D["compute_viewbox_transform<br/>(preserveAspectRatio fit)"]
    D --> E{"image_key?"}
    E -- "loaded" --> F["push_image"]
    E -- "pending / failed / vector" --> G["placeholder<br/>push_rect + X"]
    F --> H["WebRender display list"]
    G --> H
```

## 6. Dependencies and build impact

Dependencies declared in [Cargo.toml](Cargo.toml):

| Library | Usage |
|---------|-------|
| `euclid` | Affine transforms (`Transform2D`) for node/viewBox/gradient/pattern/marker matrices, plus the `Point2D`/`Rect`/`Size`/`Vector2D` primitives behind layout coordinates |
| `kurbo` | Path representation: `BezPath` (every shape via `to_bez_path`), `Stroke` + dash handling, `Affine`, and `PathEl` — the format handed to `vello_cpu` and the basis of `ComplexClip` geometry |
| `lyon` | Polygon tessellation: `FillTessellator` triangulates polygons into triangles emitted as per-scanline `push_rect` bands, used to fill shapes inside `<pattern>` content |
| `svgtypes` | Spec-compliant SVG parsing: `Length`/`LengthUnit`, `PointsParser`, `ViewBox`, `Color`, and `TransformListParser` — backing `attr_parsers`, `render_tree`, `transform_ops` |
| `vello_cpu` | Software rasterization — takes a `BezPath` and produces an RGBA `Pixmap` |

`euclid`, `kurbo`, and `vello_cpu` are already dependencies of existing
components (the canvas and layout crates); the engine introduces only two new
third-party crates — `lyon` (polygon tessellation) and `svgtypes` (SVG value
parsing).

**Build-system impact**

- `svg_engine` is a **workspace member** (listed in the root `Cargo.toml`),
  published at `components/svg_engine`.
- No build bootstrap, feature-unification, or build-script changes are
  introduced — the added crates are pure Rust libraries.
- `vello_cpu` is enabled with the `multithreading` feature in the workspace pin.

## 7. Public API

| API | Description | Input parameters | Return type |
|-----|-------------|------------------|-------------|
| `build_svg_render_tree` (components/layout/svg) | Builds the `SvgRenderTree` from the DOM subtree and resolved CSS values. | `node: ServoLayoutNode<'dom>`, `context: &LayoutContext` | `Option<Arc<SvgRenderTree>>` |
| `render_svg_tree` (components/svg_engine) | Renders an entire `SvgRenderTree` into a WebRender display list. | `tree: &SvgRenderTree`, `svg_origin: &LayoutPoint`, `svg_size: LayoutSize`, `device_scale: f32`, `spatial_id: SpatialId`, `clip_chain_id: ClipChainId`, `sink: &RasterSink`, `wr: &mut DisplayListBuilder` | No return — pushes display commands directly into `wr` (`&mut DisplayListBuilder`) |
