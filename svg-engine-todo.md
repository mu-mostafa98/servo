# SVG Engine — Bug Fix & Enhancement TODO

> Generated from code review on 2026-07-06.
> ✓ = Fix applied.  Priority order: P1 → P2 → P3 → Future.

---

## P1 — Correctness Bugs (Existing features that misbehave)

### ✓ 1. Gradient strokes silently dropped for polylines/paths/circles/ellipses

**Fixed in:** [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs) + [renderer/line.rs](components/svg_engine/src/renderer/line.rs)

**Fix (visibility):** Extracted a shared `stroke_line_segment` helper that handles both solid-color and gradient paint servers. `Line::render` and `stroke_polyline` both delegate to it. The old `stroke_polyline` guarded on `stroke.color.is_some()` — now it checks both `color` and `paint_server`.

**Fix (regression):** `stroke_polyline` was passing local coordinates to `stroke_line_segment` which expects absolute coordinates (with `svg_origin` already added). Fixed by adding `svg_origin` in the call site.

**Fix (per-segment solid color, 2026-07-06):** `stroke_polyline_gradient` was evaluating the gradient at each full segment's midpoint and drawing the entire segment as a single solid color. Changed to subdivide each segment into ~4px pieces and evaluate the gradient at each piece's midpoint in absolute (parent-frame) coordinates, so the gradient varies smoothly along the entire polyline. Added `draw_rotated_stroke_segment` helper to avoid per-piece `NodeStyle`/`RenderContext` allocation.

---

### ✓ 2. Radial gradient + polygon/polyline fill paints bounding rectangle, not shape

**Fixed in:** [renderer/fill.rs](components/svg_engine/src/renderer/fill.rs) + [tessellator.rs](components/svg_engine/src/tessellator.rs)

**Fix:** Added `FillStyle::RadialGradient` variant to the tessellator. When `fill_polygon` encounters a radial gradient, it converts coordinates to absolute space and tessellates the polygon, computing per-pixel radial `t` values (distance from focal point / radius) in the scanline loop. The old code fell back to `fill_rect_with_gradient_by_id` which painted the bounding rect.

---

### ✓ 3. Pattern fill on polygons paints bounding rectangle, not shape

**Fixed in:** [renderer/fill.rs](components/svg_engine/src/renderer/fill.rs) + [tessellator.rs](components/svg_engine/src/tessellator.rs)

**Fix (v1):** Added `FillStyle::Pattern` variant to the tessellator. When `fill_polygon` encounters a pattern paint server, it computes tile dimensions/origin in absolute space and evaluates the pattern per-pixel using point-in-shape geometry (rect/circle/ellipse hit testing). Only solid-filled shapes in patterns are evaluated — gradient/pattern-filled pattern shapes are skipped.

**Fix (v2 — shape rendering, 2026-07-06):** Replaced the per-pixel `pattern_color_at` evaluation with proper `shape.render()` calls inside the scanline loop, grouped by tile column. For each scanline the tessellator clips each tile's shape rendering to the polygon boundary via a per-tile clip rect. This means pattern shapes (circles, rounded rects) are now rendered as proper WebRender primitives with anti-aliasing, matching the quality of the rect-based pattern path. Removed the now-unused `point_in_shape` and `pattern_color_at` helpers.

---

### ✓ 4. `userSpaceOnUse` gradients use wrong coordinate origin in polygon fill

**Fixed in:** [renderer/fill.rs](components/svg_engine/src/renderer/fill.rs) + [renderer/gradient.rs](components/svg_engine/src/renderer/gradient.rs)

**Fix:** Both the tessellator path (`fill_polygon`) and the rect path (`render_linear`/`render_radial`) now convert all gradient coordinates to absolute space and add a bounding-box offset so pixel positions are also interpreted as absolute. Previously `userSpaceOnUse` computed pixel positions relative to the bounding box but compared them against absolute gradient coordinates.

**Note:** This also fixes the same bug for rect fills — `userSpaceOnUse` gradients on `<rect>` elements now work correctly regardless of the rect's position.

---

### 5. `fill="none"` vs "no fill specified" inheritance confusion

**File:** [components/layout/svg_builder.rs:293-310](components/layout/svg_builder.rs#L293-L310)

**Problem:** When a presentation attribute says `fill="none"`, `PaintServer::from_attr` returns `None`, which causes `build_style_from_attrs` to produce `fill: None`. In the renderer, `fill: None` means "do not fill" — same behavior as `fill="none"`. But `fill="none"` is a *final* value (the element and its children have no fill), while no fill attribute means "inherit from parent."

**NOT YET FIXED** — requires tri-state enum change.

---

### 6. `currentColor` not supported as default fill

**File:** [components/layout/svg_builder.rs:88-103](components/layout/svg_builder.rs#L88-L103)

**Problem:** SVG 2 spec says fill defaults to `currentColor`. Currently, elements with no fill attribute get `fill: None` → rendered invisible.

**NOT YET FIXED** — requires passing currentColor through the pipeline.

---

### 7. Dashed strokes not rendered for `<line>` elements

**File:** [components/svg_engine/src/renderer/line.rs](components/svg_engine/src/renderer/line.rs)

**Problem:** The `<line>` renderer reads `stroke.width` and `stroke.color` but ignores `stroke.dash_array` and `stroke.dash_offset`.

**NOT YET FIXED** — requires dash decomposition or WebRender border dash support.

---

### 8. SVG viewport clip always applied regardless of `overflow`

**File:** [components/svg_engine/src/traversal.rs:41-46](components/svg_engine/src/traversal.rs#L41-L46)

**Problem:** The viewport clip rect is always defined. Per the SVG spec, `overflow="visible"` should allow content outside the viewport.

**NOT YET FIXED** — requires overflow property plumbing.

---

### 9. CSS `transform` property on SVG elements is ignored

**File:** [components/layout/svg_builder.rs:249](components/layout/svg_builder.rs#L249)

**Problem:** Stylo-computed CSS `transform` is overwritten by the raw attribute.

**NOT YET FIXED** — requires merging computed + attribute transforms.

---

## P2 — Limitations & Edge Cases

### 10. Clip paths limited to rect/circle/ellipse shapes

**File:** [components/svg_engine/src/shapes/mod.rs:58-65](components/svg_engine/src/shapes/mod.rs#L58-L65)

### 11. Gradients inside nested `<g>` inside `<defs>` not collected

**File:** [components/layout/svg_builder.rs:407-447](components/layout/svg_builder.rs#L407-L447)

### 12. `build_style_from_attrs` ignores CSS `visibility`

**File:** [components/layout/svg_builder.rs:341-351](components/layout/svg_builder.rs#L341-L351)

### 13. `preserveAspectRatio` not implemented

**File:** [components/svg_engine/src/traversal.rs:50-71](components/svg_engine/src/traversal.rs#L50-L71)

### 14. `em`/`ex` units in length attributes not properly resolved

**File:** [components/svg_engine/src/shapes/attr_parsers.rs:33-35](components/svg_engine/src/shapes/attr_parsers.rs#L33-L35)

### 15. `stroke-linecap` ignored for polylines

**File:** [components/svg_engine/src/renderer/stroke.rs:93-138](components/svg_engine/src/renderer/stroke.rs#L93-L138)

---

## P3 — Performance & Robustness

### 16. Gradient scanline rasterization emits too many `push_rect` calls

**File:** [components/svg_engine/src/renderer/gradient.rs:85-98](components/svg_engine/src/renderer/gradient.rs#L85-L98)

### 17. No NaN guard in scanline rasterizer

**File:** [components/svg_engine/src/tessellator.rs:159-222](components/svg_engine/src/tessellator.rs#L159-L222)

### 18. `sort_vertices_by_y` uses `partial_cmp` with NaN fallback

**File:** [components/svg_engine/src/tessellator.rs:221](components/svg_engine/src/tessellator.rs#L221)

---

## Future Features (Not Yet Implemented — Low Priority)

- `<text>` / `<tspan>` — SVG text rendering
- `<image>` inside SVG — embedded images
- SMIL animations — animated SVG
- `<marker>` elements — arrowheads, pins
- `<switch>` — conditional rendering
- `<foreignObject>` — embedded HTML in SVG
- SVG fonts / `@font-face` in SVG
- `<use>` with `viewBox` override on `<symbol>` target
- `mix-blend-mode` on SVG elements
- `<filter>` effects beyond basic gaussian blur / drop shadow / color matrix
- Nested `<svg>` elements
- `writing-mode` and bidirectional text
- `<a>` (anchor) elements in SVG
- SVG-as-image (`<img src="file.svg">`)

---

## Quick Reference — File Map

| Issue | Status | File | Lines |
|-------|--------|------|-------|
| #1 Gradient strokes dropped | **✓ FIXED** | [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs) | stroke_line_segment, stroke_polyline, draw_rotated_stroke_segment |
| #2 Radial gradient fills rect | **✓ FIXED** | [tessellator.rs](components/svg_engine/src/tessellator.rs) | RadialGradient variant |
| #3 Pattern fills rect | **✓ FIXED** | [tessellator.rs](components/svg_engine/src/tessellator.rs) | shape.render() per tile, clip to polygon |
| #4 userSpaceOnUse origin | **✓ FIXED** | [renderer/gradient.rs](components/svg_engine/src/renderer/gradient.rs) | offset_x/offset_y in strategies |
| #5 fill="none" vs inheritance | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 293-310 |
| #6 currentColor default | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 88-103 |
| #7 Dashed line strokes | Not fixed | [renderer/line.rs](components/svg_engine/src/renderer/line.rs) | line.rs |
| #8 Viewport overflow clip | Not fixed | [traversal.rs](components/svg_engine/src/traversal.rs) | 41-46 |
| #9 CSS transform ignored | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 249 |
| #10 Limited clip paths | Not fixed | [shapes/mod.rs](components/svg_engine/src/shapes/mod.rs) | 58-65 |
| #11 Nested defs collection | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 407-447 |
| #12 visibility in attrs | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 341-351 |
| #13 preserveAspectRatio | Not fixed | [traversal.rs](components/svg_engine/src/traversal.rs) | 50-71 |
| #14 em/ex units | Not fixed | [shapes/attr_parsers.rs](components/svg_engine/src/shapes/attr_parsers.rs) | 33-35 |
| #15 linecap for polylines | Not fixed | [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs) | stroke_polyline |
| #16 Gradient perf | Not fixed | [renderer/gradient.rs](components/svg_engine/src/renderer/gradient.rs) | 85-98 |
| #17 NaN in tessellator | Not fixed | [tessellator.rs](components/svg_engine/src/tessellator.rs) | scanline loop |
| #18 partial_cmp NaN | Not fixed | [tessellator.rs](components/svg_engine/src/tessellator.rs) | 221 |
