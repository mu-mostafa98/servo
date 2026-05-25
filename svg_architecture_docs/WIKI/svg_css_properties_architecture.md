# SVG CSS Properties Pipeline — Architecture & Data Flow

## Overview

The pipeline flows from property definition in Stylo to computed styles available per SVG element. Actual SVG rendering (Phase 3) is not yet built — this doc covers up to the style computation layer.

```
longhands.toml  →  properties.rs  →  style traversal  →  ComputedValues
   (define)          (generate)        (compute)         (result)
```

---

## Stage 1: Property Definition

**File:** `style/properties/longhands.toml` (stylo repo)

**Input:** TOML property definitions

```toml
[fill]
type = "SVGPaint"
initial = "crate::values::computed::SVGPaint::BLACK"
struct = "inherited_svg"
servo_restyle_damage = "repaint"
```

| Field | Meaning |
|-------|---------|
| `type` | Rust type for the property value (e.g., `SVGPaint`, `SVGOpacity`, `FillRule`) |
| `initial` | Default value when not specified |
| `struct` | Which style struct owns it — `"inherited_svg"` or `"svg"` |
| `servo_restyle_damage` | What update to trigger on change (`"repaint"` for visual) |
| `engine` | Gate to limit which browser engine the property is built for (removed for Servo) |

**Output:** Stylo reads this file to generate Rust parsing, computing, and matching code.

**Key constraint:** `engine = "gecko"` gates properties to Firefox-only. Removing it enables the property for Servo.

---

## Stage 2: Code Generation

**File:** `style/properties/data.py` + `properties.mako.rs` (stylo repo)

**Input:** `longhands.toml` definitions

**Process:**
- `data.py:1025-1027` — filters properties by engine. Removes properties with `engine = "gecko"` that don't match the current engine
- `properties.mako.rs` — Mako template that generates `properties.rs`
- For each property, generates: Rust field, parser, computed value conversion, debug formatting, `size_of_test!`

**Output:** `properties.rs` — the generated Rust file with all property logic

**Key data structures generated:**
```rust
// One field per property in the struct
pub struct InheritedSVG {
    pub fill: SVGPaint,           // added when fill enabled
    pub fill_opacity: SVGOpacity, // added when fill-opacity enabled
    pub stroke: SVGPaint,
    // ... one field per property
}

pub struct SVG {
    pub cx: LengthPercentage,
    pub cy: LengthPercentage,
    pub r: NonNegativeLengthPercentage,
    // ...
}
```

**Assertion:** `size_of_test!(ComputedValues, 232)` — ensures `ComputedValues` struct size doesn't regress. Must be bumped when a new style struct becomes active (gains its first property for Servo).

---

## Stage 3: Style Computation

**File:** `components/layout/traversal.rs` (servo repo) + Stylo traversal engine

**Input:** DOM tree (including inline `style="..."` attributes)

**Process:**

```
DOM element
    │
    ▼
recalc_style_at()          ← Stylo traversal (process_preorder)
    │
    ├─ Parse inline styles  ← CSS parser reads style="..."
    ├─ Match selectors      ← determine which CSS rules apply
    ├─ Compute values       ← resolve keywords/cales to computed values
    │
    ▼
ComputedValues stored in ElementData
```

**Key entry point:** `RecalcStyle::process_preorder()` at `traversal.rs:42` — called for every DOM element during style recalc.

**SVG detection:**
```rust
if dangerous_style_element.is_svg_element() {
    // Access computed styles
    let isvg = primary.get_inherited_svg();
    let svg = primary.get_svg();
}
```

`is_svg_element()` uses the DOM namespace — checks if the element's namespace is `http://www.w3.org/2000/svg`.

**Output:** `ElementData.styles` — contains `ComputedValues` with populated `InheritedSVG` and `SVG` structs.

**Current limitation:** SVG children are hidden from layout via `servo.css`:
```css
svg > * { display: none; }
```
Styles are still computed (we verified this via logging), but elements don't produce layout boxes.

---

## Stage 4: Damage Propagation

**File:** `components/layout/traversal.rs` — `compute_damage_and_rebuild_box_tree()`

**Input:** `RestyleDamage` from style changes + existing box tree

**Process:**
1. Takes damage from current element and parent
2. Propagates damage to children (layout rebuild needed?)
3. Propagates damage from children to parent
4. Manages box tree reconstruction at independent formatting context boundaries

**Output:** Updated damage state — determines what needs re-layout.

**Relevant damage values:**
- `RestyleDamage::RELAYOUT` — fragment tree layout must re-run
- `LayoutDamage::box_damage()` — box tree must be rebuilt
- `LayoutDamage::descendant_has_box_damage()` — some descendant needs box rebuild

---

## Stage 5: Box Tree Construction

**File:** `components/layout/box_tree.rs`

**Input:** Layout context + DOM root node

**Process:** Creates a tree of layout boxes from the DOM tree and computed styles.

**Output:** `Arc<BoxTree>` — the layout box tree used for fragment tree layout.

**Note:** Currently SVG children are `display: none`, so they don't get box tree entries. In Phase 3, removing `svg > * { display: none; }` and adding an SVG renderer will let boxes form.

---

## Stage 6: SVG-as-Image Rasterization (existing)

**File:** `components/net/image_cache.rs` — `rasterize_vector_image()` (line 953)

**Input:** SVG bytes → parsed `usvg::Tree`, requested pixel size

**Process:**
```
SVG data → usvg::Tree  →  resvg::render()  →  tiny_skia::Pixmap
  parse       (SVG AST)      (render)           (pixel buffer)
```

**Libraries:**
- `resvg` — SVG parser and renderer
- `tiny_skia` — software 2D rasterizer (used by resvg)
- `usvg` — SVG tree representation (used by resvg)

**I/O:**
- Input: raw SVG bytes, target `DeviceIntSize`
- Output: `RasterImage` (pixel data with WebRender image key)

**Scope:** Only for `<img src="file.svg">` and CSS `background-image`, NOT for inline `<svg>` elements.

---

## Data Structures Reference

### `InheritedSVG` (style struct)
```
Fill:       fill, fill-opacity, fill-rule
Stroke:     stroke, stroke-width, stroke-opacity,
            stroke-linecap, stroke-linejoin, stroke-miterlimit,
            stroke-dasharray, stroke-dashoffset
Markers:    marker-start, marker-mid, marker-end
Text:       text-anchor, paint-order
Color:      color-interpolation, color-interpolation-filters
Shape:      shape-rendering
Clip:       clip-rule
```

### `SVG` (non-inherited style struct)
```
Geometry:   cx, cy, r, rx, ry, x, y, d, vector-effect
Filters:    flood-color, flood-opacity, lighting-color
Paints:     stop-color, stop-opacity, clip-path
Mask:       mask-image, mask-type, mask-mode, mask-clip,
            mask-origin, mask-composite, mask-position-x/y,
            mask-repeat, mask-size
```

### `ElementData`
```rust
pub struct ElementData {
    pub styles: ComputedStyles,   // primary + pseudo styles
    pub damage: RestyleDamage,    // what changed since last layout
}
```

### `ComputedValues`
- Holds `Arc` pointers to all active style structs (including `InheritedSVG` and `SVG`)
- Size assertion: `size_of_test!(ComputedValues, 232)` for Servo debug builds

---

## I/O Summary

| Stage | Input | Output | Key File |
|-------|-------|--------|----------|
| 1. Definition | `longhands.toml` entries | Property metadata | `stylo/style/properties/longhands.toml` |
| 2. Generation | TOML + Mako template | `properties.rs` | `stylo/style/properties/properties.mako.rs` |
| 3. Style Compute | DOM + CSS rules | `ComputedValues` | `servo/components/layout/traversal.rs` |
| 4. Damage | Style changes | `RestyleDamage` | `servo/components/layout/traversal.rs` |
| 5. Box Tree | DOM + styles | `BoxTree` | `servo/components/layout/box_tree.rs` |
| 6. SVG Raster | SVG bytes + size | `RasterImage` | `servo/components/net/image_cache.rs` |

---

## Key Files Map

```
stylo repo (D:\Projects\stylo):
  style/properties/longhands.toml      ← Property definitions (edited in Phase 1)
  style/properties/data.py             ← Code generation engine
  style/properties/properties.mako.rs  ← Rust generation template + size_of_test!

servo repo (D:\Projects\servo):
  components/layout/traversal.rs       ← Style traversal + SVG detection + logging
  components/layout/stylesheets/servo.css  ← svg > * { display: none } (blocking Phase 3)
  components/net/image_cache.rs        ← SVG-as-image rasterization via resvg/tiny_skia
  components/script/layout_dom/
    servo_layout_element.rs            ← TElement impl for Servo
```
