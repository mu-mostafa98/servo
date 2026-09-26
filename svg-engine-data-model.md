# SVG Engine — Data Model

Class diagrams for the `svg_engine` crate's **model** half (`src/model/`): the pure
SVG data model that the layout layer builds and the render layer consumes.

The model is deliberately free of any rendering dependency — it does **not** import
`webrender_api`, `euclid`, `kurbo`, `lyon`, `log`, or `vello_cpu`. Its only external
crate is [`svgtypes`](https://crates.io/crates/svgtypes), used solely for the `Color`
type (shown as `Color` below).

### How to read these diagrams

| Notation | Meaning |
|----------|---------|
| plain box | Rust `struct` (fields listed) |
| `<<enum>>` | Rust `enum` (variants listed) |
| `<<interface>>` | Rust `trait` |
| `A *-- B` | `A` owns `B` (composition) |
| `A o-- B` | `A` holds an optional / `Arc` reference to `B` |
| `A ..> B` | `A` uses `B` (method parameter / visitor) |
| `A <|-- B` | `B` implements `A` |

Where one concept spans several independent clusters of types, they are drawn as
separate stacked diagrams — one per connected component — so each diagram reads as a
single self-contained picture.

---

## 1. Module map

```mermaid
flowchart TD
    CE["svg_engine crate"] --> M["model<br/>(pure data — no WebRender / vello)"]
    CE --> R["render<br/>(display-list emission — future PR)"]

    M --> E["element<br/>SvgNode · SvgTag · shape · text · image"]
    M --> S["style<br/>NodeStyle · paint · gradient · transform · effects"]
    M --> D["document<br/>SvgTree · defs · viewport"]
    M --> C["coords<br/>re-export facade (viewport · geometry · transform · length)"]
    M --> U["units<br/>Id · Length · Opacity"]
    M --> G["geometry<br/>Point · PathData · PathCommand"]
    M --> RS["resource<br/>ResourceKey"]
    M --> ER["error<br/>SvgEngineError"]
```

The model splits into three conceptual buckets — **element** (anything written as
`<element>`), **style** (anything that can be an attribute), **document** (the tree,
viewport, and `<defs>` definitions) — plus four leaf modules (`units`, `geometry`,
`resource`, `error`) of small shared value types, and `coords`, a chapter-facing facade
that re-exports the coordinate-system types (`ViewportInfo`, `SvgViewport`, `ViewBox`,
`AspectRatio`, `Point`, `PathData`, `PathCommand`, `TransformOp`, `Length`) from their
canonical homes.

---

## 2. Render tree

The render tree is a **plain owned tree**: `SvgTree → SvgNode → children` with no
`Arc`/`Rc` indirection. Definitions referenced by id live in flat maps on `SvgTree`
(see §6); nodes refer to them via `DefRef` / `PaintServer` handles, not pointers.

### Tree structure

```mermaid
classDiagram
    direction TB

    class SvgTree {
        +root : SvgNode
        +viewport : ViewportInfo
        +gradients
        +clip_paths
        +patterns
        +masks
        +filters
        +markers
        +visit(visitor)
        +visit_mut(visitor)
    }

    class SvgNode {
        +id : Option~Id~
        +tag : SvgTag
        +style : NodeStyle
        +transforms : Vec~TransformOp~
        +viewport : Option~SvgViewport~
        +children : Vec~SvgNode~
        +accept(visitor)
        +accept_mut(visitor)
    }

    class SvgTag {
        <<enum>>
        Shape
        Text
        Image
        Container
    }

    class Container {
        <<enum>>
        Group
        Svg
        Defs
        Use
        Switch
        Symbol
        Text
    }

    SvgTree *-- SvgNode : root
    SvgNode *-- SvgNode : children
    SvgNode *-- SvgTag : tag
    SvgTag o-- Container : Container(..)
```

- Each `SvgNode` carries three kinds of data: **what it is** (`tag`), **how it's
  painted** (`style: NodeStyle`), and **how it's transformed** (`transforms`, structural
  not paint-level). A nested `<svg>` viewport hangs off `viewport`.
- The six def maps on `SvgTree` are `HashMap<String, Arc<T>>` keyed by element `id`
  (no `#` prefix): `gradients → Arc<GradientDef>`, `clip_paths → Arc<ClipPathDef>`,
  `patterns → Arc<PatternDef>`, `masks → Arc<MaskDef>`, `filters → Arc<FilterDef>`,
  `markers → Arc<MarkerDef>`.

### Visitor

```mermaid
classDiagram
    direction TB

    class VisitDecision {
        <<enum>>
        Continue
        SkipChildren
        Stop
    }

    class SvgTreeVisitor {
        <<interface>>
        +visit_node(node) VisitDecision
    }

    class SvgTreeVisitorMut {
        <<interface>>
        +visit_node_mut(node) VisitDecision
    }

    SvgTreeVisitor ..> VisitDecision
    SvgTreeVisitorMut ..> VisitDecision
```

- Traversal is a classic visitor: `SvgTree::visit` / `visit_mut` delegate to
  `SvgNode::accept` / `accept_mut` (tree diagram above), which drive
  `Continue` / `SkipChildren` / `Stop`.

---

## 3. Elements — shape, text, image

### Shapes

Shapes live in `element::shape` — re-exported flat as `element::Shape`,
`element::Rectangle`, … — so every `<element>` type is reachable from
`svg_engine::element` (see §1).

```mermaid
classDiagram
    direction TB

    class Shape {
        <<enum>>
        Rect
        Circle
        Ellipse
        Line
        Polyline
        Polygon
        Path
    }

    class Rectangle {
        +x : Length
        +y : Length
        +width : Length
        +height : Length
        +rx : Option~Length~
        +ry : Option~Length~
        +path_length : Option~f32~
    }
    class Circle {
        +cx : Length
        +cy : Length
        +r : Length
        +path_length : Option~f32~
    }
    class Ellipse {
        +cx : Length
        +cy : Length
        +rx : Option~Length~
        +ry : Option~Length~
        +path_length : Option~f32~
    }
    class Line {
        +x1 : Length
        +y1 : Length
        +x2 : Length
        +y2 : Length
        +path_length : Option~f32~
    }
    class Polyline {
        +points : Vec~Point~
        +path_length : Option~f32~
    }
    class Polygon {
        +points : Vec~Point~
        +path_length : Option~f32~
    }
    class Path {
        +path : PathData
        +path_length : Option~f32~
    }

    Shape o-- Rectangle : Rect
    Shape o-- Circle : Circle
    Shape o-- Ellipse : Ellipse
    Shape o-- Line : Line
    Shape o-- Polyline : Polyline
    Shape o-- Polygon : Polygon
    Shape o-- Path : Path

    Path *-- PathData : path
    Polyline *-- Point : points
    Polygon *-- Point : points
```

- Shapes hold **already-resolved geometry** (`Length`/`Point`/`PathData`) — the layout
  layer does the `d`-attribute parsing and `rx`/`ry` resolution; the model stores the
  result. `Path::path` is a flat `Vec<PathCommand>` (absolute coordinates only).
- Every shape (and `<path>`) carries `path_length: Option<f32>` — the author-asserted
  `pathLength` attribute, used to calibrate distance-along-path math (notably stroke
  dashing). `Shape::path_length()` reads it uniformly.
- `Ellipse`'s `rx`/`ry` are `Option<Length>` just like `Rectangle`'s: SVG 2 makes the
  radii support `auto`, resolved by `resolved_radii()` (both `auto` → no rendering; one
  `auto` → derive from the other, yielding a circle).

### Text

```mermaid
classDiagram
    direction TB

    class TextSpan {
        +text : String
        +x : Vec~f32~
        +y : Vec~f32~
        +dx : Vec~f32~
        +dy : Vec~f32~
        +rotate : Vec~f32~
        +glyphs : Vec~ShapedGlyph~
        +text_anchor : TextAnchor
        +rtl : bool
        +dominant_baseline : DominantBaseline
        +font_instance_key : Option~ResourceKey~
        +advance_offset : f32
        +font_size : f32
        +origin_x() f32
        +origin_y() f32
        +total_advance() f32
    }

    class ShapedGlyph {
        +x : f32
        +y : f32
        +advance : f32
        +glyph_id : u32
        +character : char
        +font_instance_key : Option~ResourceKey~
    }

    class TextAnchor {
        <<enum>>
        Start
        Middle
        End
    }
    class DominantBaseline {
        <<enum>>
        Auto
        TextBeforeEdge
        TextAfterEdge
        Hanging
        Middle
        Central
        Ideographic
        Alphabetic
        Mathematical
    }

    TextSpan *-- ShapedGlyph : glyphs
    TextSpan *-- TextAnchor : text_anchor
    TextSpan *-- DominantBaseline : dominant_baseline
```

- `TextSpan` carries both raw text *and* pre-shaped `ShapedGlyph`s (per-glyph font key,
  so mixed-script runs work). `Text` is also a `Container` variant in §2 (inline runs).
- `x`/`y` are per-character **coordinate lists** (SVG §11.5.2): each entry repositions the
  current text position for the matching character. `origin_x()`/`origin_y()` return `x[0]`
  /`y[0]` (or `0` when unset) and the renderer offsets shaped glyphs by that origin.
  `dx`/`dy`/`rotate` are likewise per-character lists.

### Image

```mermaid
classDiagram
    direction TB

    class SvgImage {
        +x : f32
        +y : f32
        +width : f32
        +height : f32
        +href : Option~String~
        +image_key : Option~ResourceKey~
        +natural_width : Option~u32~
        +natural_height : Option~u32~
        +preserve_aspect_ratio : AspectRatio
    }
```

- `SvgImage` keeps the raster handle (`image_key: ResourceKey`) opaque, so the model
  never touches `webrender_api::ImageKey`; the render layer reconstructs the key.

---

## 4. Style — paint, effects, hints, transforms

### Node style & painting

```mermaid
classDiagram
    direction TB

    class NodeStyle {
        +visibility : Visibility
        +display : Display
        +fill : Option~FillParams~
        +stroke : Option~StrokeParams~
        +render_hints : Option~RenderHints~
        +effects : Option~NodeEffects~
        +opacity : Opacity
        +markers : Option~MarkerRefs~
        +is_visible() bool
        +is_displayed() bool
    }

    class Visibility {
        <<enum>>
        Visible
        Hidden
    }
    class Display {
        <<enum>>
        Inline
        Block
        None
    }

    class FillParams {
        +paint_server : Option~PaintServer~
        +opacity : Opacity
        +fill_rule : FillRule
    }
    class StrokeParams {
        +paint_server : Option~PaintServer~
        +opacity : Opacity
        +width : Length
        +line_cap : LineCap
        +line_join : LineJoin
        +miter_limit : f32
        +dash_array : Option~Vec~f32~~
        +dash_offset : f32
    }

    class FillRule {
        <<enum>>
        NonZero
        EvenOdd
    }
    class LineCap {
        <<enum>>
        Butt
        Round
        Square
    }
    class LineJoin {
        <<enum>>
        Miter
        MiterClip
        Round
        Bevel
        Arcs
    }

    class NodeEffects {
        +clip_path : Option~DefRef~ClipPathDef~~
        +mask : Option~DefRef~MaskDef~~
        +filter : Option~DefRef~FilterDef~~
    }

    class MarkerRefs {
        +start : Option~DefRef~MarkerDef~~
        +mid : Option~DefRef~MarkerDef~~
        +end : Option~DefRef~MarkerDef~~
    }

    NodeStyle *-- Visibility
    NodeStyle *-- Display
    NodeStyle *-- Opacity
    NodeStyle o-- FillParams : fill
    NodeStyle o-- StrokeParams : stroke
    NodeStyle o-- NodeEffects : effects
    NodeStyle o-- MarkerRefs : markers

    FillParams *-- FillRule
    FillParams o-- PaintServer : paint_server
    StrokeParams *-- LineCap
    StrokeParams *-- LineJoin
    StrokeParams o-- PaintServer : paint_server

    NodeEffects o-- DefRef : clip_path / mask / filter
    MarkerRefs o-- DefRef : start / mid / end
```

- `NodeStyle` is **paint-level only**; layout-affecting state (transforms, viewport) lives
  on `SvgNode`, not here. Its `render_hints` field is detailed in the next diagram.
- `fill` / `stroke` are `Option` — SVG's "no fill / no stroke" is distinct from a default
  black fill, and the renderer checks `is_some()`.
- There is **no `color` field** on `FillParams`/`StrokeParams`: a solid color lives inside
  the `PaintServer::Solid(Color)` variant (see §5), reached via `paint_server`.
- Effects (`clip_path`, `mask`, `filter`) and markers are `DefRef<T>` — a `#id` during
  build, rewritten to an `Arc<T>` handle by the resolve pass (see §6). The same
  transient-then-resolved pattern appears as `PaintServer::Ref` in §5.

### Render hints & paint order

```mermaid
classDiagram
    direction TB

    class RenderHints {
        +vector_effect : Option~VectorEffect~
        +color_rendering : Option~ColorRendering~
        +color_interpolation : Option~ColorInterpolation~
        +shape_rendering : Option~ShapeRendering~
        +paint_order : Option~PaintOrder~
        +text_rendering : Option~TextRendering~
        +image_rendering : Option~ImageRendering~
    }

    class PaintOrder {
        +order : PaintOperation[3]
        +stroke_before_fill() bool
    }

    class PaintOperation {
        <<enum>>
        Fill
        Stroke
        Markers
    }

    class VectorEffect {
        <<enum>>
        None
        NonScalingStroke
    }
    class ColorRendering {
        <<enum>>
        Auto
        OptimizeSpeed
        OptimizeQuality
    }
    class ColorInterpolation {
        <<enum>>
        Auto
        Srgb
        LinearRGB
    }
    class ShapeRendering {
        <<enum>>
        Auto
        OptimizeSpeed
        CrispEdges
        GeometricPrecision
    }
    class TextRendering {
        <<enum>>
        Auto
        OptimizeSpeed
        OptimizeLegibility
        GeometricPrecision
    }
    class ImageRendering {
        <<enum>>
        Auto
        OptimizeSpeed
        OptimizeQuality
    }

    RenderHints o-- PaintOrder : paint_order
    PaintOrder *-- PaintOperation : order
    RenderHints o-- VectorEffect : vector_effect
    RenderHints o-- ColorRendering : color_rendering
    RenderHints o-- ColorInterpolation : color_interpolation
    RenderHints o-- ShapeRendering : shape_rendering
    RenderHints o-- TextRendering : text_rendering
    RenderHints o-- ImageRendering : image_rendering
```

- `PaintOrder` is a **struct**, not an enum — it wraps an ordered `[PaintOperation; 3]`
  triple (default `[Fill, Stroke, Markers]`), and `stroke_before_fill()` answers the
  "does the stroke draw under the fill?" question the renderer needs.
- `RenderHints` folds the rendering-quality/order hints; several are spec stubs gated
  `#[allow(dead_code)]` (`text_rendering`, `image_rendering`).

### Transform

```mermaid
classDiagram
    direction TB

    class TransformOp {
        <<enum>>
        Translate
        Scale
        Rotate
        SkewX
        SkewY
        Matrix
    }
```

- `TransformOp` is an ordered list (`Vec<TransformOp>`) on `SvgNode` — `matrix(a b c d e f)`
  is a variant, not a wrapper.

---

## 5. Paint servers & gradients

```mermaid
classDiagram
    direction TB

    class PaintServer {
        <<enum>>
        Solid(Color)
        Gradient(Arc~GradientDef~)
        Pattern(Arc~PatternDef~)
        Ref(Id, Option~Color~)
        ContextFill
        ContextStroke
    }

    class GradientDef {
        <<enum>>
        Linear
        Radial
    }

    class LinearGradient {
        +id : String
        +href : Option~String~
        +x1 : GradientLength
        +y1 : GradientLength
        +x2 : GradientLength
        +y2 : GradientLength
        +units : GradientUnits
        +stops : Vec~GradientStop~
        +transform : Vec~TransformOp~
        +spread_method : SpreadMethod
        +explicit : GradientExplicit
    }

    class RadialGradient {
        +id : String
        +href : Option~String~
        +cx : GradientLength
        +cy : GradientLength
        +r : GradientLength
        +fx : GradientLength
        +fy : GradientLength
        +fr : GradientLength
        +units : GradientUnits
        +stops : Vec~GradientStop~
        +transform : Vec~TransformOp~
        +spread_method : SpreadMethod
        +explicit : GradientExplicit
    }

    class GradientStop {
        +offset : f32
        +color : Color
    }

    class GradientUnits {
        <<enum>>
        ObjectBoundingBox
        UserSpaceOnUse
    }
    class SpreadMethod {
        <<enum>>
        Pad
        Reflect
        Repeat
    }
    class GradientLength {
        <<enum>>
        Number(f32)
        Percentage(f32)
    }
    class GradientExplicit {
        +x1 : bool
        +y1 : bool
        +x2 : bool
        +y2 : bool
        +cx : bool
        +cy : bool
        +r : bool
        +fx : bool
        +fy : bool
        +fr : bool
        +units : bool
        +spread_method : bool
        +transform : bool
        +stops : bool
    }

    PaintServer o-- GradientDef : Gradient(Arc)
    PaintServer o-- PatternDef : Pattern(Arc)
    PaintServer *-- Color : Solid

    GradientDef o-- LinearGradient : Linear
    GradientDef o-- RadialGradient : Radial
    LinearGradient *-- GradientStop : stops
    RadialGradient *-- GradientStop : stops
    LinearGradient *-- GradientUnits
    LinearGradient *-- SpreadMethod
    LinearGradient *-- GradientLength
    LinearGradient *-- GradientExplicit : explicit
    RadialGradient *-- GradientExplicit : explicit
    GradientStop *-- Color
```

- `PaintServer` mirrors `DefRef`: `Solid`/`Gradient`/`Pattern` are resolved forms, `Ref`
  (a struct variant with named `id`/`fallback` fields, drawn tuple-style above) is the
  transient build-time state that the resolve pass rewrites to an `Arc` handle.
  No `Ref` value survives past build. `ContextFill`/`ContextStroke` carry the
  `context-fill`/`context-stroke` keywords through `<marker>`/`<use>` (they render as no
  paint when no context element supplies the value).
- `GradientDef` stores parsed `<linearGradient>` / `<radialGradient>` from `<defs>`;
  `href` inheritance is supported — `GradientExplicit` records which attributes were
  *authored* (vs. defaulted) so an inherited value can be told apart from a local default
  (matters for `fx`/`fy`).
- `Color` = `svgtypes::Color`.

---

## 6. Definitions & references (`<defs>`)

### `DefRef`

```mermaid
classDiagram
    direction TB

    class DefRef~T~ {
        <<enum>>
        Ref(Id)
        Resolved(Arc~T~)
        +resolved() Option~T~
    }
```

- `DefRef<T>` is the core indirection: `Ref(Id)` during tree building → `Resolved(Arc<T>)`
  after the resolve pass. `Clone` is manual so `DefRef<T>` is `Clone` for any `T`, even
  `ClipPathDef`/`MaskDef`/`FilterDef`/`MarkerDef` that embed a non-`Clone` `SvgNode`.

### Clip, mask, pattern, marker

```mermaid
classDiagram
    direction TB

    class ClipPathDef {
        +root : SvgNode
        +clip_path_units : ClipPathUnits
    }
    class MaskDef {
        +root : SvgNode
        +mask_type : MaskType
        +content_units : MaskContentUnits
    }
    class PatternDef {
        +width : PatternLength
        +height : PatternLength
        +x : PatternLength
        +y : PatternLength
        +pattern_units : PatternUnits
        +pattern_content_units : PatternContentUnits
        +transform : Vec~TransformOp~
        +view_box : Option~ViewBox~
        +aspect_ratio : Option~AspectRatio~
        +root : SvgNode
    }
    class MarkerDef {
        +root : SvgNode
        +view_box : Option~ViewBox~
        +ref_x : f32
        +ref_y : f32
        +marker_width : f32
        +marker_height : f32
        +marker_units : MarkerUnits
        +orient : MarkerOrient
    }

    class ClipPathUnits {
        <<enum>>
        ObjectBoundingBox
        UserSpaceOnUse
    }
    class MaskType {
        <<enum>>
        Luminance
        Alpha
    }
    class MaskContentUnits {
        <<enum>>
        ObjectBoundingBox
        UserSpaceOnUse
    }
    class PatternUnits {
        <<enum>>
        ObjectBoundingBox
        UserSpaceOnUse
    }
    class PatternContentUnits {
        <<enum>>
        ObjectBoundingBox
        UserSpaceOnUse
    }
    class PatternLength {
        <<enum>>
        Number(f32)
        Percentage(f32)
    }
    class MarkerUnits {
        <<enum>>
        StrokeWidth
        UserSpaceOnUse
    }
    class MarkerOrient {
        <<enum>>
        Auto
        AutoStartReverse
        Angle
    }

    ClipPathDef *-- SvgNode : root
    MaskDef *-- SvgNode : root
    PatternDef *-- SvgNode : root
    MarkerDef *-- SvgNode : root
    ClipPathDef *-- ClipPathUnits
    MaskDef *-- MaskType
    MaskDef *-- MaskContentUnits
    PatternDef *-- PatternUnits
    PatternDef *-- PatternContentUnits
    PatternDef *-- PatternLength : width / height / x / y
    MarkerDef *-- MarkerUnits
    MarkerDef *-- MarkerOrient
```

- Every definition type stores its **content as a nested `SvgNode` subtree** (`root`), so
  `<g>`, `<use>`, `<text>`, and nested `<defs>` inside a clip/mask/pattern/marker are not
  flattened away — they stay a full sub-tree.
- `PatternDef`'s `x`/`y`/`width`/`height` are `PatternLength` — a `Number`/`Percentage`
  newtype (like `GradientLength`) that keeps the unit until the reference box is known at
  render time.

### Filter

```mermaid
classDiagram
    direction TB

    class FilterDef {
        +primitives : Vec~FilterPrimitive~
        +x : f32
        +y : f32
        +width : f32
        +height : f32
    }

    class FilterPrimitive {
        <<enum>>
        GaussianBlur(f32, f32)
        DropShadow(f32, f32, f32, f32, f32, f32, f32)
        ColorMatrix([f32; 20])
        Saturate(f32)
        LuminanceToAlpha
        Offset(f32, f32)
        Flood(f32, f32, f32, f32)
        Composite(FeCompositeKind)
        Tile
        Image(FeImageKind)
    }

    class FeCompositeKind {
        <<enum>>
        Arithmetic(f32, f32, f32, f32)
        Over
        In
        Out
        Atop
        Xor
        Lighter
    }
    class FeImageKind {
        <<enum>>
        FragmentRef(String)
        ExternalUrl(String)
    }

    FilterDef *-- FilterPrimitive : primitives
    FilterPrimitive o-- FeCompositeKind : Composite
    FilterPrimitive o-- FeImageKind : Image
```

- `FilterPrimitive` carries typed payloads: `Composite(FeCompositeKind)` (arithmetic or a
  Porter-Duff operator) and `Image(FeImageKind)` (a `#fragment` or external URL). The
  `Arithmetic` composite is a struct variant with named `k1`–`k4` coefficients
  (drawn tuple-style above: `result = k1·i1·i2 + k2·i1 + k3·i2 + k4`).

---

## 7. Viewport

```mermaid
classDiagram
    direction TB

    class ViewportInfo {
        +width : Length
        +height : Length
        +view_box : Option~ViewBox~
        +overflow_visible : bool
        +aspect_ratio : Option~AspectRatio~
    }

    class SvgViewport {
        +x : Length
        +y : Length
        +width : Length
        +height : Length
        +view_box : Option~ViewBox~
        +aspect_ratio : Option~AspectRatio~
        +overflow_visible : bool
    }

    class ViewBox {
        +min_x : Length
        +min_y : Length
        +width : Length
        +height : Length
    }

    class AspectRatio {
        +align : AspectAlign
        +meet_or_slice : MeetOrSlice
    }
    class AspectAlign {
        <<enum>>
        None
        XMinYMin
        XMidYMin
        XMaxYMin
        XMinYMid
        XMidYMid
        XMaxYMid
        XMinYMax
        XMidYMax
        XMaxYMax
    }
    class MeetOrSlice {
        <<enum>>
        Meet
        Slice
    }

    ViewportInfo o-- ViewBox : view_box
    ViewportInfo o-- AspectRatio : aspect_ratio
    SvgViewport o-- ViewBox : view_box
    SvgViewport o-- AspectRatio : aspect_ratio
    AspectRatio *-- AspectAlign
    AspectRatio *-- MeetOrSlice
```

- `ViewportInfo` is the **root** `<svg>` viewport (its size is imposed by layout), stored on
  `SvgTree::viewport`. `SvgViewport` is a **nested** `<svg>` viewport, carried by
  `SvgNode::viewport` — it owns its `x`/`y`/`width`/`height` in the parent coordinate system.
- `AspectRatio` defaults to `xMidYMid meet` (SVG spec: `viewBox` alone implies it).

---

## 8. Leaf value types

### Geometry

```mermaid
classDiagram
    direction LR

    class Point {
        +x : f32
        +y : f32
        +new(x, y) Point
    }

    class PathCommand {
        <<enum>>
        MoveTo(Point)
        LineTo(Point)
        QuadTo(Point, Point)
        CurveTo(Point, Point, Point)
        Close
    }
    class PathData {
        +commands : Vec~PathCommand~
    }

    PathData *-- PathCommand : commands
    PathCommand o-- Point
```

- **`PathData`** is a flat absolute-coordinate command list (no relative commands, no
  arcs) — the layout layer normalizes the SVG `d` attribute into this form.

### Typed newtypes

```mermaid
classDiagram
    direction LR

    class Id {
        +new(id) Id
        +as_str() String
    }
    class Length {
        +new(value) Length
        +get() f32
    }
    class Opacity {
        +new(value) Opacity
        +get() f32
    }
```

- **Typed newtypes** (`Id`, `Length`, `Opacity`) turn unit mix-ups into compile-time
  errors: an opacity can't be passed where a length is expected. `Length` is
  **unit-erased** — by the time it's built, `px`/`em`/`%`/… are already resolved to user
  space, so the renderer cannot recover the original unit. `Opacity` clamps to `[0, 1]`;
  `Length` may be negative.

### Resource & error

```mermaid
classDiagram
    direction LR

    class ResourceKey {
        +namespace : u32
        +id : u32
    }

    class SvgEngineError {
        <<enum>>
        MissingAttribute(String)
        ParseError(String)
        UnsupportedFeature(String)
    }
```

- **`ResourceKey`** is an opaque `(namespace, id)` pair that round-trips to
  `webrender_api::{ImageKey, FontInstanceKey}` — the model carries no WebRender type.
