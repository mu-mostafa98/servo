# Phase 1 Study Guide — SVG Core Shape Rendering

## Overview

Phase 1 implements SVG basic shape rendering (rect, circle, ellipse, line) in Servo. SVG elements are read from the DOM, their geometry and style are extracted, and they're converted to WebRender display items — all through a dedicated **svg_engine crate** at `components/svg_engine/`, bypassing Servo's legacy serialize→rasterize pipeline entirely. The old pipeline (serialize to XML → base64 data URL → image cache → bitmap) has been removed.

### Data Flow (5 stages)

```
DOM tree
  │ 1. Tree walk
  ▼
replaced.rs:build_svg_scene()
  │ 2. Style + geometry extraction  (paint.rs + extract_geometry())
  ▼
Vec<SvgRenderInput>                 ←── shapes.rs data types
  │ 3. Scene packaging
  ▼
ReplacedContentKind::SVGElement { scene }
  │ 4. Fragment creation
  ▼
Fragment::Svg(Arc<SvgFragment>)
  │ 5. Display list building
  ▼
render_svg_element() → WebRender push_rect / push_border / clip chains
```

---

## Module Map

### `shapes.rs` — The data types (svg_engine crate)

This is the **types hub** for the SVG engine. Every other module imports from here.

| Type | Purpose | Key detail |
|---|---|---|
| `SvgTag` | Identifies SVG element type | `Rect`, `Circle`, `Ellipse`, `Line`, `Polyline`, `Polygon`, `Path`, `Unknown` |
| `ParsedGeometry` | Holds parsed geometry per shape type | Enum with `Rect { x, y, w, h, rx, ry }`, `Circle { cx, cy, r }`, `Ellipse { cx, cy, rx, ry }`, `Line { x1, y1, x2, y2 }`, `Polyline`, `Polygon`, `Path`, `None` |
| `FillParams` | Fill color + opacity | `color: ColorF`, `opacity: f32` |
| `StrokeParams` | Stroke properties | `color: ColorF`, `width: f32`, `opacity: f32`, `line_cap`, `line_join`, `miter_limit: f32` |
| `SvgRenderInput` | One element's full render data | Combines `tag` + `geometry` + `fill` + `stroke` |
| `SvgLineCap` / `SvgLineJoin` | Enums for stroke style | Butt/Round/Square, Miter/Round/Bevel |

**Why `MallocSizeOf` everywhere**: Servo's fragment tree requires `MallocSizeOf` for memory tracking. Types that can't derive it get `#[ignore_malloc_size_of = "..."]`.

### `paint.rs` — Style extraction (svg_engine crate)

Bridges Servo's **stylo** computed values to our simpler SVG types.

| Function | What it does | Stylo types used |
|---|---|---|
| `extract_fill_params()` | Reads fill color + opacity | `SVGPaint`, `SVGOpacity` |
| `extract_stroke_params()` | Reads all stroke properties | `SVGPaint`, `SVGWidth`, `SVGOpacity`, `SVGStrokeDashArray`, `stroke_linecap::T`, `stroke_linejoin::T` |
| `extract_geometry()` | Reads DOM attributes per tag | Calls `SvgLength::parse()` for each attribute |
| `extract_opacity()` | Reads element opacity | `style.get_effects().opacity` |
| `resolve_svg_paint()` | Converts `SVGPaintKind::Color` → `ColorF` | `Color::resolve_to_absolute()`, `to_color_space(Srgb)` |

**Key insight**: stylo stores SVG presentation attributes as CSS computed values. `style.get_inherited_svg().fill` gives us the `SVGPaint` regardless of whether it came from a CSS rule or a `<rect fill="...">` attribute. The attribute value was converted to a CSS presentational hint by `element.rs:synthesize_presentational_hints_for_legacy_attributes()`.

### `lengths.rs` — Coordinate parsing (svg_engine crate)

Parses SVG length strings (`"10"`, `"50%"`, `"2cm"`, `"12pt"`) into a typed enum.

| Variant | Meaning |
|---|---|
| `Px(f32)` | Absolute pixel value (or unitless number, with unit conversion for pt/pc/cm/mm/in/em/ex) |
| `Percent(f32)` | Fraction of viewport (0.0–1.0) |

Key method: `.resolve(reference_length)` converts to `f32` — percentages multiply against the reference, pixels return directly.

### `render.rs` — WebRender output (svg_engine crate)

The **most complex module**. Converts `SvgRenderInput` into actual pixels through WebRender API calls.

#### Architecture constraint (svg_engine crate)

The `svg_engine` crate (at `components/svg_engine/`) is a separate workspace crate, not a module inside layout. It follows the same pattern as `canvas/`, `paint/`, `webgl/`, and depends only on external crates (`stylo`, `webrender_api`, `kurbo`, `euclid`, `app_units`). The layout crate imports it as a dependency (`svg_engine = { workspace = true }`).

WebRender has NO native circle, ellipse, or path primitives. We approximate:
- **Filled rect** → `push_rect` directly
- **Rounded rect** → `define_clip_rounded_rect()` + `push_rect`
- **Circle/Ellipse fill** → `define_clip_rounded_rect(ClipMode::Clip)` + `push_rect`
- **Circle/Ellipse stroke** → Ring clip (outer `Clip` + inner `ClipOut`) + `push_border`
- **Rect stroke** → `push_border` with `BorderStyle::Solid`
- **Line** → `push_border` (thin rect approximation)

#### Key functions

| Function | Role |
|---|---|
| `render_svg_element()` | Entry point — iterates scene, dispatches to shape renderers |
| `resolve_geometry()` | Convert `ParsedGeometry` + viewport → resolved `f32` values in `ResolvedGeometry` |
| `render_rect()` | Fill via `push_rect`, stroke via `push_border`, optional rounded corners |
| `render_circle()` | Delegates to `render_ellipse_common()` |
| `render_ellipse()` | Delegates to `render_ellipse_common()` |
| `render_ellipse_common()` | Fill with elliptical clip, stroke with ring clip |
| `render_line()` | `push_border` with appropriate thickness |
| `make_shape_clip()` | Helper to define a rounded-rect clip + wrap in clip chain |

#### Ring clip technique (for circle/ellipse strokes)

```
1. Define outer clip: ComplexClipRegion { rect: outer_bounds, radii: outer_R, mode: Clip }
2. Define inner clip: ComplexClipRegion { rect: outer_bounds, radii: inner_R, mode: ClipOut }
3. Create clip chain from both (define_clip_chain)
4. Push border with that clip chain → only the ring between outer and inner ellipse is painted
```

This avoids needing a dedicated stroke primitive for curved shapes.

### `replaced.rs` — Integration with layout tree

The bridge between Servo's layout system and the svg_engine crate. Key sections:

#### `ReplacedContentKind::SVGElement` variant

```rust
SVGElement {
    scene: Option<Arc<Vec<SvgRenderInput>>>,  // SVG scene data
}
```

No more `vector_image` or `has_viewbox` fields — those were part of the old serialization pipeline and have been removed.

#### `svg_kind_size()` — Scene building entry point

Called during layout of any `<svg>` element (detected via `node.as_svg().is_some()`):
1. Calls `build_svg_scene()` which walks the flat tree children
2. Packages result into `ReplacedContentKind::SVGElement { scene: Some(Arc::new(scene)) }`
3. Returns hardcoded 300×150 natural size (unchanged from before, matches CSS spec default)

#### `build_svg_scene()` — DOM walker

Walks flat tree children of the `<svg>` root:
1. Checks if child is a basic shape via local_name matching
2. Reads style via `node.style(&context.style_context)`
3. Calls `extract_geometry()` with DOM attributes: `element.attribute_as_str(&ns!(), &local_name!("..."))`
4. Calls `extract_fill_params()` + `extract_stroke_params()` from stylo computed values
5. Builds `SvgRenderInput { tag, geometry, fill, stroke }`
6. Pushes onto the scene vector

#### `make_fragments()` — Fragment creation

Checks if `scene` is present and non-empty. If so, returns `Fragment::Svg(Arc::new(SvgFragment { ... }))`. Otherwise returns an empty vec (no fallback to the old pipeline — it's gone).

### `fragment_tree/fragment.rs` — SvgFragment

```rust
pub(crate) struct SvgFragment {
    pub base: BaseFragment,
    pub scene: Arc<Vec<SvgRenderInput>>,
}
```

New `Fragment::Svg(#[conditional_malloc_size_of] Arc<SvgFragment>)` variant added with match arms in `base()`, `base_mut()`, `print()`, `scrollable_overflow_for_parent()`, and other required Fragment methods.

### `display_list/mod.rs` — Display list dispatch

```rust
Fragment::Svg(svg_fragment) => {
    // Visibility check (Visibility::Visible)
    // Compute rect from base.rect + containing block offset → to_webrender()
    // Call render_svg_element(&scene, rect, spatial_id, clip_chain_id, wr)
}
```

---

## How WebRender Drawing Works

WebRender is a GPU-oriented renderer that accepts a **display list** — a sequence of drawing commands grouped by spatial position and clipping.

### Key concepts used in Phase 1

| Concept | Meaning |
|---|---|
| `SpatialId` | Identifies a scrollable/transformable coordinate space |
| `ClipChainId` | Identifies a chain of clip regions |
| `CommonItemProperties` | Bundles clip_rect + spatial_id + clip_chain_id + flags |
| `PrimitiveFlags::IS_BACKFACE_VISIBLE` | Render even when geometry faces away |
| `DisplayListBuilder` | Accumulates display items (push_rect, push_border, define_clip*) |

### Drawing a filled rect
```rust
let common = make_common(bounds, spatial_id, clip_chain_id);
wr.push_rect(&common, bounds, color);
```

### Drawing a filled circle (using clip)
```rust
// 1. Define elliptical clip
let clip_id = wr.define_clip_rounded_rect(spatial_id, ComplexClipRegion { rect, radii, mode: Clip });
// 2. Create clip chain
let chain_id = wr.define_clip_chain(None, [clip_id]);
// 3. Push rect with clip
let common = make_common(bounds, spatial_id, chain_id);
wr.push_rect(&common, bounds, fill_color);
```

### Drawing a stroked circle (ring clip)
Outer `Clip` + inner `ClipOut` = a ring shape, then push a border over the whole outer bounds.

---

## Stylo Integration Details

### How SVG attributes become CSS values

1. `<rect fill="red">` → DOM engine calls `element.set_attribute("fill", "red")`
2. `element.rs:synthesize_presentational_hints_for_legacy_attributes()` creates a CSS `PropertyDeclaration::Fill(val)` via the `svg_attr!` macro and `parse_declared()`
3. Stylo resolves it to `ComputedValues` → `style.get_inherited_svg().fill`
4. `paint.rs` reads `SVGPaintKind::Color(resolved_color)`
5. Calls `color.resolve_to_absolute(&current_color)` to handle `currentColor`
6. Converts to sRGB then to `ColorF` for WebRender

### SVG type aliases in stylo

| Stylo type | Layout meaning |
|---|---|
| `SVGPaint = GenericSVGPaint<Color, ComputedUrl>` | fill or stroke paint |
| `SVGPaintKind::Color(c)` | Solid color |
| `SVGPaintKind::None` | No paint (transparent) |
| `SVGPaintKind::PaintServer` | Gradient/pattern (Phase 6+) |
| `SVGOpacity::Opacity(f32)` | Opacity value (the inner value IS f32, not a newtype) |
| `SVGWidth::LengthPercentage(lp)` | Stroke width → call `to_used_value(Au).to_f32_px()` |
| `SVGStrokeDashArray::Values(v)` | Array of dash lengths |

### Presentation attribute coverage

50 SVG presentation attributes are mapped to CSS declarations in `element.rs`:

```
fill, stroke, stroke-width, stroke-opacity, fill-opacity, stroke-linecap,
stroke-linejoin, stroke-miterlimit, stroke-dasharray, stroke-dashoffset,
fill-rule, clip-rule, opacity, visibility, color, cx, cy, r, rx, ry,
x, y, width, height, d, dx, dy, rotate, text-anchor, transform,
transform-origin, vector-effect, flood-color, flood-opacity,
lighting-color, stop-color, stop-opacity, clip-path, clip-rule, mask,
marker-start, marker-mid, marker-end, paint-order, shape-rendering,
color-interpolation, color-interpolation-filters, text-rendering,
image-rendering
```

---

## What Was Removed (Old Pipeline Cleanup)

The old serialize→rasterize pipeline was removed from these files:

| File | What Changed |
|---|---|
| `shared/layout/lib.rs` | Removed `SVGElementData` struct, `ratio_from_view_box()` |
| `shared/layout/layout_node.rs` | `svg_data()` returns `Option<()>` (boolean marker) |
| `layout/dom.rs` | `as_svg()` returns `Option<()>` |
| `layout/context.rs` | Removed `queue_svg_element_for_serialization()`, `pending_svg_elements` |
| `layout/layout_impl.rs` | Removed serialization data from reflow result |
| `layout/replaced.rs` | Removed `vector_image`, `has_viewbox` from `SVGElement` variant |
| `script/dom/node/node.rs` | `svg_data()` simplified to `self.downcast::<SVGSVGElement>().map(\|_\| ())` |
| `script/dom/svg/svgsvgelement.rs` | Removed `data()` method, `cached_serialized_data_url`, `uuid`, `serialize_and_cache_subtree()`, `process_use_elements()`, `invalidate_cached_serialized_subtree()` |
| `script/dom/window.rs` | Removed `serialize_and_cache_subtree()` loop |
| `script/layout_dom/servo_layout_node.rs` | `svg_data()` simplified |

---

## Build Fixes Compendium

Issues encountered during the first build and how they were fixed:

| Problem | Cause | Fix |
|---|---|---|
| `#[ignore_malloc_size_of]` panics | Macro requires explanation string | Added `= "reason"` to all ignore attributes |
| `LayoutPoint` / `LayoutRect` private | Not re-exported from `webrender_api` root | Import from `webrender_api::units::*` |
| `LayoutRect::new(point, size)` | `Box2D::new()` takes TWO points, not point+size | Use `LayoutRect::from_origin_and_size(origin, size)` |
| `SVGOpacity::Opacity(o) => o.0` | Inner value is already `f32`, not a tuple struct | Just use `o` directly |
| Closure lifetime | Closure captures a reference and returns `&str` tied to it | Return `Option<String>` (owned) instead |
| `SvgLength` missing `MallocSizeOf` | Simple enum (f32 fields) just needs the derive | Added `#[derive(MallocSizeOf)]` |
| `SvgTag` / `SvgLineCap` / `SvgLineJoin` missing `MallocSizeOf` | Same — simple unit enums | Added derive |
| `crate::svg_engine` paths broken after extraction to separate crate | Internal paths must use `crate::` not `crate::svg_engine::` | Updated to `crate::shapes::`, `crate::lengths::`, etc. |
| Workpace dependency key mismatch | Used `servo-svg-engine` as key but referenced as `svg_engine` | Changed key to `svg_engine` with `package = "servo-svg-engine"` |

---

## Test Page Structure

The `svg_test.html` covers:
- Filled rectangles (red, blue, green, no-fill-with-stroke)
- Rounded rectangles (varying rx/ry)
- Filled circles (solid, semi-transparent, stroke-only, fill+stroke)
- Ellipses (filled, stroked, transparent fill)
- Horizontal, vertical, and diagonal lines
- Stroke variants with thick borders on rect, circle, ellipse
- Combined shapes scene

---

## What's Next

### Phase 2 — Complex Shapes
- **Path rendering**: Software rasterization (`tiny_skia`/`resvg`) since WR has no bezier path primitive
- **Polygon/Polyline**: Line loop / polyline stroke

### Phase 3 — Structure & Transforms
- **`<g>` support**: Group elements with push/pop transform stack
- **`transform` attribute**: `push_reference_frame()` for per-element transforms
- **`viewBox` / `preserveAspectRatio`**: Coordinate system mapping
- **Opacity**: `push_stacking_context()` for standalone opacity
- **`<use>`**: Reference resolution without DOM cloning

### Phase 4 — SVG Text
- **`<text>` / `<tspan>`**: Text positioning, `text-anchor`, Servo font system integration

### Phase 5 — Clipping & Masks
- **`<clipPath>`**: WebRender clip chain integration
- **`<mask>`**: Mask support

### Phase 6 — Gradients & Filters
- **Linear/radial gradients**: `push_gradient()` / `push_radial_gradient()`
- **Filters**: `push_stacking_context_with_filters()`
