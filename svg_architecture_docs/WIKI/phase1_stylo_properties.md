# Phase 1: Register SVG CSS Properties in Stylo

## Goal
Enable SVG CSS properties in Servo's style engine (Stylo) by removing `engine = "gecko"` gates from `longhands.toml`.

## Setup
- Stylo repo: `D:\Projects\stylo` (branch: `svg-css-properties`)
- Servo repo: `D:\Projects\servo` (using `[patch]` to local stylo)
- Test file: `C:\Users\Staff1\Desktop\servo-svg-Iinline-style.html`
- Log location: `components/layout/replaced.rs:svg_kind_size()` — logs `parent_style.get_svg()`

## Current State
All SVG properties are now registered in Stylo. Both `InheritedSVG` and `SVG` style structs have all fields populated, parsing and computing from inline styles correctly.

## Status: ✅ COMPLETE

All 4 sub-tasks are done. Phase 2 (presentation attribute mapping) is the next step.

## Main Rule: 3 Changes Per Property

For each SVG property we enable, we must make these changes in the **stylo repo** (`D:\Projects\stylo`):

### 1. Remove `engine = "gecko"` from `longhands.toml`
```toml
[fill]
type = "SVGPaint"
initial = "crate::values::computed::SVGPaint::BLACK"
struct = "inherited_svg"
# remove this line ↓
# engine = "gecko"
```
This makes the property visible to Servo's code generation.

### 2. Add `servo_restyle_damage` to `longhands.toml`
```toml
servo_restyle_damage = "repaint"   # "repaint" for visual-only changes
```
This tells Servo what to do when the property changes. Values: `"repaint"`, `"rebuild_box"`, `"recalculate_overflow"`, `"rebuild_stacking_context"`. Most SVG paint properties use `"repaint"`.

### 3. Update `size_of_test!` in `properties.mako.rs` (if needed)
When a style struct becomes active for the first time (gains its first Servo property), it adds an `Arc` pointer (8 bytes) to `ComputedValuesInner`. The `size_of_test!` assertion must be bumped by +8.

```rust
// Before:
size_of_test!(ComputedValues, 224);
// After:
size_of_test!(ComputedValues, 232);
```

### Build & Test
```bash
./mach build    # automatically picks up stylo changes via [patch]
./mach run "file:///path/to/test.html"
```

---

## Sub-tasks

### Sub-task 1: `fill` property only

**File:** `D:\Projects\stylo\style\properties\longhands.toml`
**Change:** Remove `engine = "gecko"` from `[fill]` section (around line 763)

```toml
[fill]
type = "SVGPaint"
initial = "crate::values::computed::SVGPaint::BLACK"
struct = "inherited_svg"
# remove this line ↓
# engine = "gecko"
boxed = true
spec = "https://svgwg.org/svg2-draft/painting.html#SpecifyingFillPaint"
affects = "paint"
```

**Expected result after rebuild + run:**
- `InheritedSVG` struct now exists with `fill` field
- Log shows `fill: SVGPaint::Color(Color::GREEN)` (for the test file)

**Test:**
```bash
./mach build
./mach run "file:///C:/Users/Staff1/Desktop/servo-svg-Iinline-style.html"
# Check output for: [SVG_STYLES] ... fill: ...
```

---

### Sub-task 2: Fill family (`fill-opacity`, `fill-rule`)

**File:** `D:\Projects\stylo\style\properties\longhands.toml`
**Change:** Remove `engine = "gecko"` from:

| Property | Approx Line | Type |
|----------|-------------|------|
| `fill-opacity` | 772 | `SVGOpacity` |
| `fill-rule` | 780 | `FillRule` |

**Expected:**
- `InheritedSVG` struct now has `fill_opacity`, `fill_rule` fields
- `fill-opacity="0.5"` and `fill-rule="evenodd"` are parsed and resolved

**Test:**
```html
<rect style="fill: green; fill-opacity: 0.5" />
<path style="fill: green; fill-rule: evenodd" d="..." />
```

---

### Sub-task 3: Stroke family

**File:** `D:\Projects\stylo\style\properties\longhands.toml`
**Change:** Remove `engine = "gecko"` from:

| Property | Approx Line | Type |
|----------|-------------|------|
| `stroke` | 1959 | `SVGPaint` |
| `stroke-width` | 2000 | `SVGWidth` |
| `stroke-opacity` | 1992 | `SVGOpacity` |
| `stroke-linecap` | 3800 | keyword |
| `stroke-linejoin` | 3808 | keyword |
| `stroke-miterlimit` | 1984 | `NonNegativeNumber` |
| `stroke-dasharray` | 1968 | `SVGStrokeDashArray` |
| `stroke-dashoffset` | 1976 | `SVGLength` |

**Expected:**
- `InheritedSVG` has all stroke fields
- `stroke="blue" stroke-width="4"` are resolved correctly

**Test:**
```html
<rect style="fill: green; stroke: blue; stroke-width: 4" />
```

---

### Sub-task 4: Remaining SVG properties (batch)

**File:** `D:\Projects\stylo\style\properties\longhands.toml`
**Change:** Remove `engine = "gecko"` from all remaining SVG properties:

**`struct = "inherited_svg"`:**
`clip-rule`, `marker-end`, `marker-mid`, `marker-start`, `paint-order`, `-moz-context-properties`, `text-anchor`, `color-interpolation`, `color-interpolation-filters`, `shape-rendering`

**`struct = "svg"`:**
`d`, `flood-color`, `flood-opacity`, `lighting-color`, `stop-color`, `stop-opacity`, `vector-effect`, `cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`
`mask-type`, `mask-mode`, `mask-clip`, `mask-origin`, `mask-composite`, `mask-position-x`, `mask-position-y`, `mask-repeat`, `mask-size`

**Note:** Some mask properties already have `engine = "servo"` equivalents — verify before removing.

---

## After All Sub-tests — Final Proof

Run the test file and verify the full `SVG` and `InheritedSVG` structs appear correctly with all SVG properties resolved.

## Commit & PR Checklist

- [ ] Commit each sub-task separately (or squash at end)
- [ ] Push to `origin/svg-css-properties`
- [ ] Create PR to `github.com/servo/stylo`
- [ ] Update Servo's `Cargo.toml` with new commit hash
- [ ] Remove `[patch]` section from Servo's `Cargo.toml`
