# Phase 1 Study Guide — SVG Core Shape Rendering

## Overview

Phase 1 implements SVG basic shape rendering (rect, circle, ellipse, line) in Servo's layout engine. SVG elements are read from the DOM, their geometry and style are extracted, and they're converted to WebRender display items — all within the **layout crate**, bypassing Servo's legacy SVG pipeline entirely.

### Data Flow (5 stages)

```
DOM tree
  │ 1. Tree walk
  ▼
replaced.rs:collect_svg_render_inputs()
  │ 2. Style + geometry extraction
  ▼
SvgRenderInput structs  ←── paint.rs + extract_geometry()
  │ 3. Scene packaging
  ▼
ReplacedContentKind::SVGElement { scene }
  │ 4. Fragment creation
  ▼
Fragment::Svg(SvgFragment)
  │ 5. Display list building
  ▼
render_svg_element() → WebRender push_rect / push_border / clip chains
```

---

## Module Map

### `svg_engine/shapes.rs` — The data types

This is the **types hub** for the SVG engine. Every other module imports from here.

| Type | Purpose | Key detail |
|---|---|---|
| `SvgTag` | Identifies SVG element type | Has `is_basic_shape()` and `is_container()` helpers |
| `ParsedGeometry` | Holds parsed geometry per shape type | Enum with variants for each shape, holds `Option<SvgLength>` fields |
| `FillParams` | Fill color + opacity | `color: Option<ColorF>`, `opacity: f32` |
| `StrokeParams` | Stroke properties | color, width, opacity, dasharray, linecap, linejoin, miterlimit |
| `SvgRenderInput` | One element's full render data | Combines tag + geometry + fill + stroke + transform + clip_path + opacity |
| `SvgLineCap` / `SvgLineJoin` | Enums for stroke style | Butt/Round/Square, Miter/Round/Bevel |

**Why `MallocSizeOf` everywhere**: Servo's fragment tree requires `MallocSizeOf` for memory tracking. Types that can't derive it (BezPath, Vec<KurboPoint>) get `#[ignore_malloc_size_of = "..."]`.

### `svg_engine/paint.rs` — Style extraction

Bridges Servo's **stylo** computed values to our simpler SVG types.

| Function | What it does | Stylo types used |
|---|---|---|
| `extract_fill_params()` | Reads fill color + opacity | `SVGPaint`, `SVGOpacity` |
| `extract_stroke_params()` | Reads all stroke properties | `SVGPaint`, `SVGWidth`, `SVGOpacity`, `SVGStrokeDashArray`, `stroke_linecap::T`, `stroke_linejoin::T` |
| `extract_geometry()` | Reads DOM attributes per tag | Calls `SvgLength::parse()` for each attribute |
| `extract_opacity()` | Reads element opacity | `style.get_effects().opacity` |
| `resolve_svg_paint()` | Converts `SVGPaintKind::Color` → `ColorF` | `Color::resolve_to_absolute()`, `to_color_space(Srgb)` |

**Key insight**: stylo stores SVG presentation attributes as CSS computed values. `style.get_inherited_svg().fill` gives us the `SVGPaint` regardless of whether it came from a CSS rule or a `<rect fill="...">` attribute. The attribute value was converted to a CSS presentational hint by the DOM engine.

### `svg_engine/lengths.rs` — Coordinate parsing

Parses SVG length strings (`"10"`, `"50%"`, `"2cm"`, `"12pt"`) into a typed enum.

| Variant | Meaning |
|---|---|
| `Px(f32)` | Absolute pixel value (or unitless number) |
| `Percent(f32)` | Fraction of viewport (0.0–1.0) |

Key method: `.resolve(reference_length)` converts to f32 — percentages multiply against the reference, pixels return directly.

### `svg_engine/render.rs` — WebRender output

The **most complex module**. Converts `SvgRenderInput` into actual pixels through WebRender API calls.

#### Architecture constraint

WebRender has NO native circle, ellipse, or path primitives. We approximate:
- **Filled rect** → `push_rect` directly
- **Rounded rect** → `define_clip_rounded_rect()` + `push_rect`
- **Circle/Ellipse fill** → `define_clip_rounded_rect(ClipMode::Clip)` + `push_rect`
- **Circle/Ellipse stroke** → Ring clip (outer `Clip` + inner `ClipOut`) + `push_rect`
- **Rect stroke** → `push_border` with `BorderStyle::Solid`
- **Line** → Thin rect (horizontal/vertical), bounding rect fallback (diagonal)

#### Key functions

| Function | Role |
|---|---|
| `render_svg_element()` | Entry point — iterates scene, dispatches to shape renderers |
| `resolve_rect/circle/ellipse/line()` | Convert `ParsedGeometry` + viewport → resolved f32 values |
| `render_rect()` | Fill via push_rect, stroke via push_border |
| `render_circle()` | Delegates to `render_ellipse_common()` |
| `render_ellipse()` | Delegates to `render_ellipse_common()` |
| `render_ellipse_common()` | Fill with elliptical clip, stroke with ring clip |
| `render_line()` | Thin rects for axis-aligned lines, bounding rect for diagonal |
| `make_clip_chain()` | Helper to define a rounded-rect clip + wrap in clip chain |

#### Ring clip technique (for circle/ellipse strokes)

```
1. Define outer clip: ComplexClipRegion { rect: outer_bounds, radii: outer_R, mode: Clip }
2. Define inner clip: ComplexClipRegion { rect: outer_bounds, radii: inner_R, mode: ClipOut }
3. Create clip chain from both
4. Push rect with that clip chain → only the ring between outer and inner ellipse is painted
```

This avoids needing a dedicated stroke primitive for curved shapes.

### `replaced.rs` — Integration with layout tree

The bridge between Servo's layout system and the SVG engine. Key sections:

#### `ReplacedContentKind::SVGElement` variant (line 141)

```rust
SVGElement {
    vector_image: Option<VectorImage>,  // Existing field (unchanged)
    has_viewbox: bool,                  // Existing field (unchanged)
    scene: Option<Arc<Vec<SvgRenderInput>>>,  // NEW: our SVG scene
}
```

#### `svg_kind_size()` — Scene building entry point

Called during layout of any `<svg>` element. Previously just logged. Now:
1. Calls `build_svg_scene()` which walks the flat tree
2. Packages result into `ReplacedContentKind::SVGElement { scene: Some(Arc::new(scene)) }`
3. Returns hardcoded 300×150 natural size (unchanged from before)

#### `collect_svg_render_inputs()` — DOM walker

Recursively traverses flat tree children:
1. Checks if element is a basic shape via `SvgTag::from_str()` + `is_basic_shape()`
2. Reads style via `node.style(&context.style_context)`
3. Extracts geometry from DOM attributes: `element.attribute_as_str(&ns!(), &LocalName::from(name))`
4. Extracts fill/stroke/opacity from stylo computed values
5. Reads transform attribute
6. Pushes `SvgRenderInput` onto the scene list
7. Recurses into container elements (svg, g, defs)

#### `make_fragments()` — Fragment creation

Checks if `scene` is present and non-empty. If so, returns `Fragment::Svg(ArcRefCell::new(SvgFragment { ... }))` instead of the legacy image-based path.

### `fragment_tree/fragment.rs` — SvgFragment

```rust
pub(crate) struct SvgFragment {
    pub base: BaseFragment,
    pub scene: Arc<Vec<SvgRenderInput>>,
}
```

New `Fragment::Svg(ArcRefCell<SvgFragment>)` variant added with match arms in `base()`, `base_mut()`, `print()`, `scrollable_overflow_for_parent()`, and other required Fragment methods.

### `display_list/mod.rs` — Display list dispatch

```rust
Fragment::Svg(svg_fragment) => {
    // Visibility check, compute rect from base.rect + containing block offset
    // Call render_svg_element(&scene, rect, spatial_id, clip_chain_id, wr)
    // Then builder.check_if_paintable()
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
Outer `Clip` + inner `ClipOut` = a ring shape, then push a rect over the whole outer bounds.

---

## Stylo Integration Details

### How SVG attributes become CSS values

1. `<rect fill="red">` → DOM engine calls `element.set_attribute("fill", "red")`
2. Script creates a presentational hint — a pseudo-CSS declaration `fill: red`
3. Stylo resolves it to `ComputedValues` → `style.get_inherited_svg().fill`
4. `paint.rs` reads `SVGPaintKind::Color(resolved_color)`
5. Calls `color.resolve_to_absolute(&current_color)` to handle currentColor
6. Converts to sRGB then to `ColorF` for WebRender

### SVG type aliases in stylo

| Stylo type | Layout meaning |
|---|---|
| `SVGPaint = GenericSVGPaint<Color, ComputedUrl>` | fill or stroke paint |
| `SVGPaintKind::Color(c)` | Solid color |
| `SVGPaintKind::None` | No paint (transparent) |
| `SVGPaintKind::PaintServer` | Gradient/pattern (Phase 2+) |
| `SVGOpacity::Opacity(f32)` | Opacity value (the inner value IS f32, not a newtype) |
| `SVGWidth::LengthPercentage(lp)` | Stroke width → call `to_used_value(Au).to_f32_px()` |
| `SVGStrokeDashArray::Values(v)` | Array of dash lengths |

---

## Build Fixes Compendium

Issues encountered during the first build and how they were fixed:

| Problem | Cause | Fix |
|---|---|---|
| `#[ignore_malloc_size_of]` panics | Macro requires explanation string | Added `= "reason"` to all ignore attributes |
| `LayoutPoint` / `LayoutRect` private | Not re-exported from `webrender_api` root | Import from `webrender_api::units::*` |
| `LayoutRect::new(point, size)` | `Box2D::new()` takes TWO points, not point+size | Use `LayoutRect::from_origin_and_size(origin, size)` |
| `SVGOpacity::Opacity(o) => o.0` | Inner value is already `f32`, not a tuple struct | Just use `o` directly |
| Closure lifetime: `element` doesn't live long enough | Closure captures a reference and returns `&str` tied to it | Return `Option<String>` (owned) instead |
| `SvgLength` missing `MallocSizeOf` | Simple enum (f32 fields) just needs the derive | Added `#[derive(MallocSizeOf)]` |
| `SvgTag` / `SvgLineCap` / `SvgLineJoin` missing `MallocSizeOf` | Same — simple unit enums | Added derive |

---

## Test Page Structure

The `phase1_test.html` covers:
- Filled rectangles (red, green, blue, no-fill-with-stroke)
- Rounded rectangles (varying rx/ry)
- Filled circles (solid, semi-transparent, stroke-only, fill+stroke)
- Ellipses (filled, stroked, transparent fill)
- Horizontal, vertical, and diagonal lines
- Stroke variants with thick borders on rect, circle, ellipse
- Combined opacity on a line

---

## What's Next (Phase 2)

- **Path rendering**: Software rasterization or tessellation since WR has no path primitive
- **Polygon/Polyline**: Line loop / polyline stroke
- **Gradients**: Linear and radial gradient fills
- **Transform support**: `transform="translate(x,y)"` etc. on individual shapes
- **Text**: SVG `<text>` element rendering
- **Clipping**: `<clipPath>` support
