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

### ✓ 5. `fill="none"` vs "no fill specified" inheritance confusion

**Fixed in:** [layout/svg_builder.rs](components/layout/svg_builder.rs)

**Fix (2026-07-06):** `build_style_from_attrs` now returns `Some(FillParams { color: None, paint_server: None })` when the `fill` attribute is `"none"` (explicitly no paint), vs `None` when there is no fill attribute at all (inherit from parent). The renderer already handled both cases identically (skip fill), so this is a data model fix that enables correct inheritance in the future. Previously both cases produced `fill: None` at the `NodeStyle` level, conflating "no value" with "explicitly none."

**Note:** The `FromComputedValues` path (CSS-styled elements, the main rendering path) was already correct — Stylo resolves inheritance before it reaches the renderer, so a child inheriting `fill="red"` from a parent gets the resolved color. The fix primarily affects the `build_style_from_attrs` path used for pattern-definition shapes.

---

### ✓ 6. `currentColor` not supported as default fill

**Fixed in:** [layout/svg_builder.rs](components/layout/svg_builder.rs)

**Fix (2026-07-06):** Modified `FromComputedValues for FillParams` to return `currentColor` as the default fill when no fill paint is specified (SVG 2 spec behavior). When `fill="none"` is explicitly set (`SVGPaintKind::None`), returns `None` (no fill). For all other cases (unset/unknown paint kind) falls back to the computed CSS `color` property value (`currentColor`). Elements without a fill attribute now render as visible shapes using the inherited `color` value instead of being invisible.

---

### ✓ 8. SVG viewport clip always applied regardless of `overflow`

**Fixed in:** [traversal.rs](components/svg_engine/src/traversal.rs) + [layout/svg_builder.rs](components/layout/svg_builder.rs) + [render_tree.rs](components/svg_engine/src/render_tree.rs)

**Fix (2026-07-06):** Added `overflow_visible: bool` to `ViewportInfo`. Modified `extract_viewport_info` to check the computed CSS `overflow` property on the `<svg>` element via `node.style()`. When both `overflow-x` and `overflow-y` are `Visible`, the viewport clip is skipped entirely, allowing content outside the SVG bounds to render.

---

### ✓ 12. `visibility` attribute ignored in `build_style_from_attrs`

**Fixed in:** [layout/svg_builder.rs](components/layout/svg_builder.rs)

**Fix (2026-07-06):** Added `read_attr("visibility")` parsing in `build_style_from_attrs` so that pattern shapes with `visibility="hidden"` respect the property. Previously all pattern shapes were unconditionally `Visibility::Visible`.

---

### ✓ 15. `stroke-linecap` ignored for polylines

**Fixed in:** [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs)

**Fix (2026-07-06):** Added a `draw_capped_rect` helper that applies the three SVG line cap styles: **butt** (exact rect), **square** (extended by half-width past each endpoint), and **round** (extended plus a pill-shaped rounded-rect clip for semicircular ends). Integrated into `emit_rotated_rects_for_segment` (solid-stroke dashes), the gradient dash path, the no-dash gradient path, and `draw_rotated_stroke_segment` (used for gradient polyline strokes).

---

### ✓ 7. Dashed strokes not rendered for `<line>` elements

**Fixed in:** [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs)

**Fix (2026-07-06):** Added a `dash_intervals()` function that decomposes a line segment into dash/gap intervals based on `stroke-dasharray` and `stroke-dashoffset`. Modified `stroke_line_segment()` to branch on `stroke.dash_array`: when present, it decomposes each segment and draws multiple rotated sub-rects (one per dash interval) inside a single reference frame. Also handles gradient strokes with dashes — each dash is rendered as a gradient-filled sub-rect. Added 14 unit tests for `dash_intervals()` covering basic patterns, offsets, wrap-around, negative offsets, zero-length segments, and edge cases.

---

### ✓ 9. CSS `transform` property on SVG elements is ignored

**Fixed in:** [layout/svg_builder.rs](components/layout/svg_builder.rs)

**Fix (2026-07-06):** Added `css_transform_from_computed()` helper that reads the CSS `transform` from Stylo computed values via `to_transform_3d_matrix()` and converts it to a `TransformOp::Matrix`. The `build_style` function now merges CSS transforms (applied first) with the SVG `transform` attribute (applied second), matching the SVG spec ordering.

---

## P2 — Limitations & Edge Cases

### 10. Clip paths limited to rect/circle/ellipse shapes

**File:** [components/svg_engine/src/shapes/mod.rs:58-65](components/svg_engine/src/shapes/mod.rs#L58-L65)

### 11. Gradients inside nested `<g>` inside `<defs>` not collected

**File:** [components/layout/svg_builder.rs:407-447](components/layout/svg_builder.rs#L407-L447)

### 13. `preserveAspectRatio` not implemented

**File:** [components/svg_engine/src/traversal.rs:50-71](components/svg_engine/src/traversal.rs#L50-L71)

### ✓ 14. `em`/`ex` units in length attributes not properly resolved

**Fixed in:** [shapes/attr_parsers.rs](components/svg_engine/src/shapes/attr_parsers.rs) + [layout/svg_builder.rs](components/layout/svg_builder.rs)

**Fix (2026-07-06):** Added `font_size` parameter to `parse_length()` with standard CSS/SVG unit conversion (em, ex, in, cm, mm, pt, pc → px). Updated all callers in `svg_builder.rs` to pass the SVG default font-size of 16px. Added 5 unit tests.

---

## P3 — Performance & Robustness

### 16. Gradient scanline rasterization emits too many `push_rect` calls

**File:** [components/svg_engine/src/renderer/gradient.rs:85-98](components/svg_engine/src/renderer/gradient.rs#L85-L98)

### ✓ 17. No NaN guard in scanline rasterizer

**Fixed in:** [tessellator.rs](components/svg_engine/src/tessellator.rs)

**Fix:** Changed `width <= 0.0` to `!(width > 0.0)` which catches NaN, preventing zero-width or NaN-dimensioned rects.

---

### ✓ 18. `sort_vertices_by_y` uses `partial_cmp` with NaN fallback

**Fixed in:** [tessellator.rs](components/svg_engine/src/tessellator.rs)

**Fix:** Replaced `unwrap_or(Equal)` with explicit `is_nan()` checks creating a total order. NaN sorts as less than all finite values. Added 5 unit tests.

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
| #5 fill="none" vs inheritance | **✓ FIXED** | [layout/svg_builder.rs](components/layout/svg_builder.rs) | build_style_from_attrs explicit none |
| #6 currentColor default | **✓ FIXED** | [layout/svg_builder.rs](components/layout/svg_builder.rs) | FromComputedValues for FillParams |
| #7 Dashed line strokes | **✓ FIXED** | [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs) | dash_intervals, emit_rotated_rects_for_segment |
| #8 Viewport overflow clip | **✓ FIXED** | [traversal.rs](components/svg_engine/src/traversal.rs) + [render_tree.rs](components/svg_engine/src/render_tree.rs) | overflow_visible check on ViewportInfo |
| #9 CSS transform ignored | **✓ FIXED** | [layout/svg_builder.rs](components/layout/svg_builder.rs) | css_transform_from_computed |
| #10 Limited clip paths | Not fixed | [shapes/mod.rs](components/svg_engine/src/shapes/mod.rs) | 58-65 |
| #11 Nested defs collection | Not fixed | [layout/svg_builder.rs](components/layout/svg_builder.rs) | 407-447 |
| #12 visibility in attrs | **✓ FIXED** | [layout/svg_builder.rs](components/layout/svg_builder.rs) | build_style_from_attrs visibility attr |
| #13 preserveAspectRatio | Not fixed | [traversal.rs](components/svg_engine/src/traversal.rs) | 50-71 |
| #14 em/ex units | **✓ FIXED** | [shapes/attr_parsers.rs](components/svg_engine/src/shapes/attr_parsers.rs) | parse_length font_size conversion |
| #15 linecap for polylines | **✓ FIXED** | [renderer/stroke.rs](components/svg_engine/src/renderer/stroke.rs) | draw_capped_rect with LineCap |
| #16 Gradient perf | Not fixed | [renderer/gradient.rs](components/svg_engine/src/renderer/gradient.rs) | 85-98 |
| #17 NaN in tessellator | **✓ FIXED** | [tessellator.rs](components/svg_engine/src/tessellator.rs) | scanline NaN guard |
| #18 partial_cmp NaN | **✓ FIXED** | [tessellator.rs](components/svg_engine/src/tessellator.rs) | sort_vertices_by_y is_nan check |

## Visual Test Files

- [svg_line_test.html](svg_line_test.html) — `<line>` rendering including dashes, angles, widths, opacity, gradient strokes, line caps (butt/square/round) with solid &amp; dashed &amp; gradient strokes
- [svg_ellipse_test.html](svg_ellipse_test.html) — `<ellipse>`, `<circle>`, `<rect>` with fills, strokes, fill="none" inheritance, patterns, currentColor default fill
- [svg_style_test.html](svg-style-test.html) — opacity, visibility, clip-path, patterns, masks, filters, viewport overflow, visibility in pattern shapes
- [svg_gradient_test.html](svg_gradient_test.html) — linear/radial gradients, stops, units
- [svg_polyline_polygon_test.html](svg_polyline_polygon_test.html) — polys, paths, fill-rule
- [svg_transform_debug.html](svg-transform-debug.html) — CSS &amp; SVG transforms, em/ex/in/pt length unit conversion
- [svg_bug_demo.html](svg-bug-demo.html) — Regression test for all fixed bugs
