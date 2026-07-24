# Servo SVG Rendering Engine & Integration Layer — Architecture Documentation

## Overview

Servo's SVG pipeline consists of two major subsystems, both in the **Browser Engine** architectural layer:

| Layer | Path | Role |
|---|---|---|
| **SVG Rendering Engine** | `components/svg_engine/` | Low-level rendering: shapes, styles, tessellation, display list emission |
| **Integration / Layout Layer** | `components/layout/svg/` | DOM → Render bridge: tree building, CSS parsing, definition resolution, style resolution |

Further connections to the broader layout engine exist through `components/layout/replaced.rs` (handles SVG intrinsic sizing) and `components/layout/context.rs` (rasterization and image caching for SVGs).

---

## 1. SVG Rendering Engine (`components/svg_engine/`)

The `svg_engine` crate is the core, WebRender-independent SVG renderer. It takes a custom render tree data structure as input and produces WebRender display list items as output. All layout and DOM concerns are handled upstream.

### 1.1 Entry Point & Traversal

| File | Complexity | Purpose |
|---|---|---|
| [traversal.rs](components/svg_engine/src/traversal.rs) | **complex** | **Main entry point**. `render_svg_tree` walks the render tree applying transforms, clip-paths, masks, filters, and emitting WebRender display items |

**Key Functions:**
- `render_svg_tree` (lines 33–65) — Public entry point: builds the viewport clip, pushes a viewBox reference frame if defined, and starts recursive tree rendering from the root
- `render_node` (lines 151–209) — Recursive node renderer: checks display visibility, applies transforms, resolves effects (clip-path/mask/filter), dispatches element rendering, recurses into children
- `emit_element` (lines 288–328) — Dispatches rendering to shape geometry (`emit_geometry`) or leaf items (`emit_leaf`), selecting appropriate paint-server parameters
- `emit_geometry` (lines 331–382) — Shape rendering: resolves paint order (stroke-before-fill), emits stroke and fill display list items via `emit_shape` with mask clips
- `emit_leaf` (lines 433–469) — Leaf item rendering: text spans and images, with optional filter context and mask clips
- `recurse_children` (lines 474–499) — Depth-first iteration over child nodes

**Effect Resolution:**
- `resolve_node_effects` (lines 219–254) — Resolves clip-path, mask, and filter references into WebRender clip chains and filter operations via the resource provider interfaces (`ResourceProviders` trait object)
- `push_filter_context` (lines 408–428) — Pushes a WebRender stacking context for SVG filters (blur, color-matrix, composite)

**Viewport & ViewBox:**
- `build_viewport_clip` (lines 71–85) — Creates a WebRender clip rect for the SVG viewport (skipped if `overflow:visible`)
- `push_viewbox_frame` (lines 89–123) — Computes the viewBox-to-viewport scale transform with `preserveAspectRatio` support and pushes a WebRender reference frame
- `compute_viewbox_transform` (lines 505–538) — The actual 2D transform math, supporting all `preserveAspectRatio` alignments (`none`, `xMinYMin`, `xMidYMid`, `xMaxYMax` with `meet`/`slice`)

**Transform Pipeline:**
- `apply_node_transforms` (lines 262–283) — Computes accumulated transform scale and iterates each `TransformOp` to push WebRender reference frames
- `apply_transform_op` (renderer, lines 33–104) — Core transform dispatcher: matches `Translate`/`Scale`/`Rotate`/`Skew`/`Matrix` and pushes corresponding WR reference frames
- `compute_transform_scale` (renderer, lines 142–162) — Computes accumulated 2D scale from a transform list for non-scaling stroke calculations

### 1.2 Shape System (`src/shapes/`)

| File | Complexity | Purpose |
|---|---|---|
| [mod.rs](components/svg_engine/src/shapes/mod.rs) | moderate | Unified shape system with `Shape` enum, `ClipGeometry` enum, and shared clip helpers |
| [rectangle.rs](components/svg_engine/src/shapes/rectangle.rs) | moderate | `Rectangle` struct with position, dimensions, and optional corner radii (rx, ry) |
| [circle.rs](components/svg_engine/src/shapes/circle.rs) | simple | `Circle` struct with center+radius, rounded-rect clip |
| [ellipse.rs](components/svg_engine/src/shapes/ellipse.rs) | simple | `Ellipse` struct with center+radii, non-uniform rounded-rect clip |
| [line.rs](components/svg_engine/src/shapes/line.rs) | simple | `Line` struct with start/end points (no clip geometry) |
| [polygon.rs](components/svg_engine/src/shapes/polygon.rs) | simple | `Polygon` struct with 2D point list |
| [polyline.rs](components/svg_engine/src/shapes/polyline.rs) | moderate | `Polyline` struct with bounding-box clip from points |
| [path.rs](components/svg_engine/src/shapes/path.rs) | moderate | `Path` struct wrapping an SVG path string, clip = AABB of segment endpoints |

**Key design pattern:** Each shape implements a `clip_info` method that converts shape bounds to `ClipGeometry` (`RoundedRect` or `Polygon`) for WebRender clipping. The `Shape` enum provides unified dispatch via `clip_info` across all shape variants.

**Shapes provided:**
- `Shape::Rect(Rectangle)` — rectangles with optional rounded corners
- `Shape::Circle(Circle)` — circles
- `Shape::Ellipse(Ellipse)` — ellipses
- `Shape::Line(Line)` — lines (no clip geometry)
- `Shape::Polyline(Polyline)` — polylines
- `Shape::Polygon(Polygon)` — polygons
- `Shape::Path(Path)` — arbitrary SVG paths

### 1.3 Style System (`src/style/`)

| File | Complexity | Purpose |
|---|---|---|
| [mod.rs](components/svg_engine/src/style/mod.rs) | moderate | `NodeStyle` — main per-element struct aggregating visibility, display, fill, stroke, render hints, effects, and opacity. Provides `Default`, `is_visible`, and `is_displayed` helpers |
| [fill.rs](components/svg_engine/src/style/fill.rs) | simple | `FillParams` (color, paint-server ref, opacity, `FillRule::NonZero`/`EvenOdd`) |
| [stroke.rs](components/svg_engine/src/style/stroke.rs) | simple | `StrokeParams` (color, width, cap, join, dash-array/dash-offset, miter-limit, opacity, paint-server) |
| [gradient.rs](components/svg_engine/src/style/gradient.rs) | **complex** | Full gradient subsystem: `LinearGradient`, `RadialGradient`, `PaintServer` enum, gradient stop parsing with offset/color/opacity, `objectBoundingBox`/`userSpaceOnUse` units, spread methods (pad/reflect/repeat) |
| [hints.rs](components/svg_engine/src/style/hints.rs) | moderate | Render hint enums: `VectorEffect`, `ColorRendering`, `ColorInterpolation`, `ShapeRendering`, `PaintOrder`, `TextRendering`, `ImageRendering` |
| [visibility.rs](components/svg_engine/src/style/visibility.rs) | simple | `Visibility::Visible`/`Hidden`, `Display::Inline`/`Block`/`None` |
| [node_effects.rs](components/svg_engine/src/style/node_effects.rs) | simple | `NodeEffects` — optional clip-path, mask, and filter URL references |
| [transform_ops.rs](components/svg_engine/src/style/transform_ops.rs) | **complex** | `TransformOp` enum (Translate/Scale/Rotate/SkewX/SkewY/Matrix), SVG `transform` attribute string parsing via cssparser's `TransformListParser`. Extensive round-trip tests |
| [color.rs](components/svg_engine/src/style/color.rs) | moderate | CSS color string parsing (hex, named, rgb/rgba, hsl) via the cssparser crate |

### 1.4 Renderers (`src/renderer/`)

Each shape type has a corresponding renderer that produces WebRender display items:

| File | Purpose |
|---|---|
| [mod.rs](components/svg_engine/src/renderer/mod.rs) | Module root, re-exports per-shape renderers, `Render` trait, `RenderContext`, and paint providers |
| [text.rs](components/svg_engine/src/renderer/text.rs) | SVG text rendering: `render` → `emit` → `emit_glyphs` (glyph instances) / `emit_rects` (character rect fallback), with text-anchor alignment, stroke/fill color application |
| [transform.rs](components/svg_engine/src/renderer/transform.rs) | Transform application: `apply_transform_op` dispatches each `TransformOp` to a WebRender reference frame; `compute_transform_scale` for non-scaling stroke; `push_reference_frame` creates spatial nodes |
| [fill.rs](components/svg_engine/src/renderer/fill.rs) | Shape fill rendering (solid/gradient/pattern via tessellator) |
| [stroke.rs](components/svg_engine/src/renderer/stroke.rs) | Stroke rendering with `StrokeParams` |
| [gradient.rs](components/svg_engine/src/renderer/gradient.rs) | Gradient paint-server resolution → WebRender |
| [pattern.rs](components/svg_engine/src/renderer/pattern.rs) | Pattern paint-server — tiled rendering |
| [circle.rs](components/svg_engine/src/renderer/circle.rs) / [ellipse.rs](components/svg_engine/src/renderer/ellipse.rs) / [rect.rs](components/svg_engine/src/renderer/rect.rs) / [line.rs](components/svg_engine/src/renderer/line.rs) / [path.rs](components/svg_engine/src/renderer/path.rs) / [polygon.rs](components/svg_engine/src/renderer/polygon.rs) / [polyline.rs](components/svg_engine/src/renderer/polyline.rs) | Per-shape WebRender display list generation |
| [image.rs](components/svg_engine/src/renderer/image.rs) | SVG `<image>` element rendering |
| [providers.rs](components/svg_engine/src/renderer/providers.rs) | Paint/clip/filter resource providers for dependency injection |
| [helpers.rs](components/svg_engine/src/renderer/helpers.rs) | Shared rendering utilities |
| [render_trait.rs](components/svg_engine/src/renderer/render_trait.rs) | The `Render` trait that all renderable nodes must implement |

### 1.5 Tessellator ([tessellator.rs](components/svg_engine/src/tessellator.rs))

A **complex** scanline-based triangle rasterizer for filling polygons/paths:

- `tessellate_polygon` — Public entry point: resolve fill-rule → tessellate → scanline fill
- `tessellate_to_triangles` — Vector tessellation via the Lyon library with configurable fill-rule and tolerance
- `scanline_fill_triangle` — **Core rasterizer** (Y-sorts vertices, walks scanlines, interpolates edges, per-pixel color lookup for solid/linear-gradient/radial-gradient/pattern with spread-mode support)
- `FillStyle` enum — Carries fill paint data: `Solid(color)`, `LinearGradient(...)`, `RadialGradient(...)`, `Pattern`
- `sort_vertices_by_y` — NaN-safe vertex ordering for scanline traversal
- `emit_gradient_rect` — Emits a single horizontal WebRender rect segment with a gradient color

### 1.6 Supporting Modules

| File | Purpose |
|---|---|
| [text.rs](components/svg_engine/src/text.rs) | Text data structures: `ShapedGlyph`, `TextSpan` (with dx/dy arrays, text-anchor, font key), `TextAnchor::Start`/`Middle`/`End` enum |
| [visitor.rs](components/svg_engine/src/visitor.rs) | `PaintServerFixupVisitor` — Post-traversal fixup: replaces gradient paint-server IDs with pattern references when the ID matches a known pattern |
| [render_tree.rs](components/svg_engine/src/render_tree.rs) | Render tree data structure definitions |
| [image.rs](components/svg_engine/src/image.rs) | Image data model |
| [attr_parsers.rs](components/svg_engine/src/attr_parsers.rs) | Attribute parsing utilities |
| [effects/](components/svg_engine/src/effects/) | Effects subsystem (clip.rs, filter.rs, mod.rs) |
| [error.rs](components/svg_engine/src/error.rs) | Error types |
| [lib.rs](components/svg_engine/src/lib.rs) | Crate root |

---

## 2. Integration Layer (`components/layout/svg/`)

The integration layer is the **bridge** between the DOM (script crate) and the rendering engine. It converts DOM nodes into a render tree consumable by the `svg_engine` crate. All files live in the **Browser Engine** layer.

### 2.1 Module Entry Point

| File | Complexity | Purpose |
|---|---|---|
| [mod.rs](components/layout/svg/mod.rs) | simple | Barrel file re-exporting all SVG submodules and providing the `build_svg_render_tree` entry point |

**`build_svg_render_tree`** (lines 41–46) is the top-level function that orchestrates the entire pipeline, calling into all submodules.

**Depends on:** `builder`, `css`, `defines`, `geometry`, `style`, `transforms`, `viewport` — all 7 submodules.

### 2.2 Render Tree Builder ([builder.rs](components/layout/svg/builder.rs))

**Complex** — The heart of the pipeline. Traverses the DOM and converts SVG elements into render tree nodes.

**`SvgRenderTreeBuilder`** struct (lines 34–38) — 3 properties, 0 methods. Orchestrates the build process.

**Key Functions:**
- `new` (lines 42–49) — Constructor
- `build` (lines 52–76) — Main build method (25 lines)
- `build_render_node` (lines 79–109) — Constructs a single render node from a DOM element (31 lines)
- `build_text_node` (lines 117–138) — Text node construction
- `shape_text_span` (lines 142–207) — **66 lines** — Shapes a text span into a `TextSpan` with dx/dy arrays, text-anchor, and font key
- `resolve_children` (lines 214–228) — Resolves child node references
- `resolve_use_children` (lines 234–303) — **70 lines** — Expands `<use>` elements by cloning the referenced subtree
- `collect_definitions` (lines 318–329) — Pre-traversal to gather all `<defs>` elements before rendering
- `build_tag` (lines 334–348) — Generic, tag-based element construction
- `build_image_tag` (lines 351–373) — SVG `<image>` element construction
- `find_element_by_id` (lines 385–402) — ID lookup for `url(#...)` references

**`DefinitionMaps`** struct (lines 308–314) — 5 properties, holds all parsed definitions (gradients, clip-paths, patterns, masks, filters).

**Imports (downstream dependencies):**
- `components/svg_engine/src/shapes/mod.rs` — shape data structures
- `components/svg_engine/src/style/mod.rs` — `NodeStyle`
- `components/svg_engine/src/traversal.rs` — `render_svg_tree` entry point
- `components/svg_engine/src/visitor.rs` — paint-server fixup
- `components/layout/context.rs` — layout context
- `components/layout/display_list/mod.rs` — display list
- `components/layout/dom.rs` — DOM traversal
- `components/layout/fragment_tree/` — fragment tree

### 2.3 Definition Parsing ([defines.rs](components/layout/svg/defines.rs))

**Complex** — A **trait-based parser hierarchy** that handles all SVG `<defs>` content. Uses a trait pattern for each paint-server type.

**Parser traits/structs:**
- `DefinitionParser` trait (lines 30–37) — Base trait with 2 methods
- `DefinitionCollector` (line 41) — 1 method, orchestrates all parsers
- `GradientParser` (line 91) — 2 methods, parses `<linearGradient>` / `<radialGradient>`
- `ClipPathParser` (line 150) — 2 methods
- `PatternParser` (line 201) — 2 methods
- `MaskParser` (line 278) — 2 methods
- `FilterParser` (line 317) — 2 methods, **161-line parse method**, the most complex parser

**Color parsing utilities:** `parse_color`, `parse_hex_color`, `parse_rgb_color`, `parse_named_color`

Each parser's `parse` method is responsible for: reading attributes → constructing an intermediate representation → storing the parsed definition into `DefinitionMaps`.

### 2.4 CSS Processing ([css.rs](components/layout/svg/css.rs))

**Moderate** — Collects CSS `<style>` elements and inline `style=""` attributes and applies them to SVG rendering attributes.

**Function pipeline:**
1. `collect_svg_css_rules` (20 lines) — Entry point, orchestrates collection
2. `extract_style_text_content` (14 lines) — Extracts text from `<style>` elements
3. `parse_svg_class_rules` (25 lines) — Parses CSS text into selectors → declarations
4. `parse_svg_declarations` (17 lines) — Parses individual CSS declaration blocks
5. `apply_css_class_rules` (17 lines) — Matches selectors to elements
6. `apply_css_property` (**179 lines, complex**) — Large switch/match mapping CSS properties to SVG rendering attributes

### 2.5 Style Resolution ([style.rs](components/layout/svg/style.rs))

**Complex** — Resolves SVG presentation attributes and computed style into the render engine's `NodeStyle`.

**Key structs:**
- `FromComputedValues` trait (lines 37–39) — 1 method, Stylo computed values → SVG styles
- `ResolvedPaint` enum (lines 43–47) — 3 variants, resolved paint (solid / gradient / pattern)

**Key functions:**
- `resolve_svg_paint` (36 lines) — Resolves paint references to concrete paints
- `from_computed_values` — **3 overloads** (lines 89–133, 137–207, 211–303), handle computed styles for SVG elements, totaling **209 lines** of style resolution logic
- `apply_stroke_presentation_attrs` (**104 lines**) — Maps SVG stroke attributes (`stroke-width`, `stroke-linecap`, `stroke-dasharray`, etc.) to `StrokeParams`
- `apply_fill_presentation_attrs` (53 lines) — Maps fill attributes
- `apply_render_hints_from_attrs` (32 lines) — Render hints
- `build_style` (20 lines) — Assembles final `NodeStyle` from all sources
- `build_style_from_attrs` (33 lines) — Alternative path building style from attributes alone
- `apply_presentation_attrs` (34 lines) — Orchestrates application of all presentation attributes
- `apply_filter_attribute` (17 lines) — Parses `filter="url(#...)"`
- `extract_url_fragment` (9 lines) — Extracts ID fragment from `url(#id)` references

### 2.6 Geometry Construction ([geometry.rs](components/layout/svg/geometry.rs))

**Moderate** — Parses DOM element attributes into render engine shape data structures.

- `build_shape` (19 lines) — Dispatches to the correct shape parser
- `build_text` (32 lines) — Builds text span render nodes
- `parse_rect` (47 lines) — `x, y, width, height, rx, ry` → `Rectangle` struct
- `parse_circle` (29 lines) — `cx, cy, r` → `Circle` struct
- `parse_ellipse` (38 lines) — `cx, cy, rx, ry` → `Ellipse` struct
- `parse_length_list` (13 lines) — For polyline/polygon point attributes
- `extract_text_content` (13 lines) — Extracts text from DOM text nodes

### 2.7 Transforms ([transforms.rs](components/layout/svg/transforms.rs))

**Simple** — Converts Stylo-computed transform operations into CSS transform representations for SVG rendering.

- `css_transform_from_computed` (9 lines) — Entry point
- `convert_transform_operations` (48 lines) — Converts matrix, translate, scale, rotate, skew, perspective operations into `TransformOp` variants

### 2.8 Viewport ([viewport.rs](components/layout/svg/viewport.rs))

**Simple** — Extracts SVG viewport metadata from DOM elements.

- `extract_viewport_info` (35 lines) — Reads `viewBox`, `width`, `height` attributes to establish the coordinate space before rendering

---

## 3. Complete Data Flow: DOM → Render Pipeline

```
DOM (components/script/)
  │
  ▼
[layout/svg/mod.rs] build_svg_render_tree()  ← Top-level entry point
  │
  ├─ [layout/svg/defines.rs]   Collect <defs>: gradients, clip-paths, patterns, masks, filters
  ├─ [layout/svg/css.rs]       Collect CSS rules & inline styles
  ├─ [layout/svg/builder.rs]   SvgRenderTreeBuilder traverses DOM:
  │     ├─ build_render_node()     Construct render nodes
  │     ├─ build_text_node() /     Shape text spans
  │     │  shape_text_span()
  │     ├─ resolve_use_children()  Expand <use> references
  │     └─ Builds shapes from   [layout/svg/geometry.rs]
  │        Resolves styles from  [layout/svg/style.rs]
  │        Parses transforms from [layout/svg/transforms.rs]
  │        Extracts viewport from [layout/svg/viewport.rs]
  │
  ▼
SVG Render Tree (svg_engine data structures)
  │
  ▼
[svg_engine/src/visitor.rs]    PaintServerFixupVisitor (post-processing fixup)
  │
  ▼
[svg_engine/src/traversal.rs]  render_svg_tree()  ← Rendering entry point
  │
  ├─ build_viewport_clip()      Viewport clip rect
  ├─ push_viewbox_frame()       viewBox → viewport transform
  ├─ render_node()              Recursive for each node:
  │     ├─ apply_node_transforms()     Push spatial reference frames
  │     ├─ resolve_node_effects()      Resolve clip-path / mask / filter
  │     ├─ emit_element()              Dispatch to emit_geometry / emit_leaf
  │     │     ├─ [renderer/]           Shape / text / image → WR display items
  │     │     └─ [tessellator.rs]      Polygon / path → scanline rasterization
  │     └─ recurse_children()          Depth-first child iteration
  │
  ▼
WebRender Display List
```

---

## 4. Key Integration Points

### 4.1 Cross-Crate Dependencies

`layout/svg/builder.rs` is the critical bridge connecting the two crates, **importing 4** `svg_engine` modules:
- `components/svg_engine/src/shapes/mod.rs` — shape data structures
- `components/svg_engine/src/style/mod.rs` — `NodeStyle`
- `components/svg_engine/src/traversal.rs` — `render_svg_tree` entry point
- `components/svg_engine/src/visitor.rs` — paint-server fixup

### 4.2 Replaced Element Layout ([replaced.rs](components/layout/replaced.rs))

Contains `svg_kind_size` (lines 233–348, 115 lines) — determines SVG element intrinsic size by parsing `viewBox`, `width`, and `height` attributes with percentage handling. This feeds into the layout engine's replaced-element flow.

### 4.3 Layout Context ([context.rs](components/layout/context.rs))

Provides SVG image resolution and rasterization via `rasterize_vector_image` (SVG → pixels for `<img>` usage) and `queue_svg_element_for_serialization` (inline `<svg>` serialization).

### 4.4 Shared Types

- `components/shared/layout/lib.rs` — `LayoutElementType` enum includes SVG element types; `ReflowResult` carries pending SVG elements for serialization; `ratio_from_view_box` parses viewBox for aspect ratio computation
- `components/shared/layout/layout_node.rs` — `LayoutNode` trait with 35 methods for layout tree traversal, style resolution, and SVG data retrieval

---