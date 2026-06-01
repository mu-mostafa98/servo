# Stylo PR — SVG Property Changes

> **Commit:** `455f1305e enable svg scc properties for servo`
> **Repo:** `../stylo/` (patched local checkout)
> **Date:** May 2026

## Summary

This commit enables 45 SVG CSS properties for Servo that were previously restricted to Gecko-only (`engine = "gecko"`). The primary change replaces `engine = "gecko"` with `servo_restyle_damage = "repaint"`, which tells Servo's style system to trigger a repaint when the property changes. Three shorthands (`marker`, `mask`, `mask-position`) were also un-gated.

---

## Change Type Reference

| Change | Meaning |
|---|---|
| `-engine = "gecko"` | Removes Gecko-only restriction → property is now compiled and available in Servo builds |
| `+servo_restyle_damage = "repaint"` | Tells Servo's style system to trigger a repaint when this property changes (the `"layout"` damage variant triggers reflow instead) |
| `-servo_pref = "layout.unimplemented"` | Removes the experimental pref gate → property is available without `layout.unimplemented` pref |
| `#[cfg(feature = "gecko")]` removed from `pub mod marker/mask/mask_position` | Shorthand modules are now compiled for Servo too |
| Gecko-specific `From` impl `#[cfg(not(feature = "gecko"))]` | Provides Servo-compatible conversion between `mask_origin` and `mask_clip` values (excludes Gecko-only keywords like `fill-box`, `stroke-box`, `view-box`, `no-clip`) |
| Conditional `NoClip` check in mask serialization | Servo's mask shorthand serialization skips the `clip` item when it matches `NoClip` (Gecko has extra keywords) |
| `size_of_test!(ComputedValues, 224)` → `232` | `ComputedValues` grew by 8 bytes to accommodate the newly enabled SVG style fields |

---

## Property Changes — Full Table

### inherited_svg struct (20 properties)

| # | Property | Type | Change | Test Location | Test Status |
|---|---|---|---|---|---|
| 1 | clip-rule | FillRule | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/clip-rule-*.html` | ✅ |
| 2 | fill | SVGPaint | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/fill-*.svg` | ✅ |
| 3 | fill-opacity | SVGOpacity | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/fill-opacity-*.svg` | ✅ |
| 4 | fill-rule | FillRule | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/fill-rule-*.svg` | ✅ |
| 5 | stroke | SVGPaint | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-*.svg` | ✅ |
| 6 | stroke-dasharray | SVGStrokeDashArray | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-dasharray-*.svg` | ✅ |
| 7 | stroke-dashoffset | SVGLength | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-dashoffset-*.svg` | ✅ |
| 8 | stroke-linecap | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-linecap-*.svg` | ✅ |
| 9 | stroke-linejoin | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-linejoin-*.svg` | ✅ |
| 10 | stroke-miterlimit | NonNegativeNumber | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-miterlimit-*.svg` | ✅ |
| 11 | stroke-opacity | SVGOpacity | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-opacity-*.svg` | ✅ |
| 12 | stroke-width | SVGWidth | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/stroke-width-*.svg` | ✅ |
| 13 | marker-end | UrlOrNone | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/marker-end-*.svg` | ✅ |
| 14 | marker-mid | UrlOrNone | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/marker-mid-*.svg` | ✅ |
| 15 | marker-start | UrlOrNone | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/marker-start-*.svg` | ✅ |
| 16 | paint-order | SVGPaintOrder | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/paint-order-*.svg` | ✅ |
| 17 | color-interpolation | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/color-interpolation-*.svg` | ✅ |
| 18 | color-interpolation-filters | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `css/filter-effects/parsing/color-interpolation-filters-*.html` | ✅ |
| 19 | shape-rendering | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/painting/parsing/shape-rendering-*.svg` | ✅ |
| 20 | text-anchor | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/text/parsing/text-anchor-*.svg` | ✅ |

### svg struct (22 properties)

| # | Property | Type | Change | Test Location | Test Status |
|---|---|---|---|---|---|
| 21 | d | DProperty | `engine=gecko` → `servo_restyle_damage=repaint` | — | ❌ |
| 22 | flood-color | Color | `engine=gecko` → `servo_restyle_damage=repaint` | `css/filter-effects/parsing/flood-color-*.html` | ✅ |
| 23 | flood-opacity | Opacity | `engine=gecko` → `servo_restyle_damage=repaint` | `css/filter-effects/parsing/flood-opacity-*.svg` | ✅ |
| 24 | lighting-color | Color | `engine=gecko` → `servo_restyle_damage=repaint` | `css/filter-effects/parsing/lighting-color-*.html` | ✅ |
| 25 | stop-color | Color | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/pservers/parsing/stop-color-*.svg` | ✅ |
| 26 | stop-opacity | Opacity | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/pservers/parsing/stop-opacity-*.svg` | ✅ |
| 27 | vector-effect | VectorEffect | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/styling/vector-effect-invalid.html` only | ⚠️ |
| 28 | cx | LengthPercentage | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/cx-*.svg` | ✅ |
| 29 | cy | LengthPercentage | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/cy-*.svg` | ✅ |
| 30 | r | NonNegativeLengthPercentage | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/r-*.svg` | ✅ |
| 31 | rx | NonNegativeLengthPercentageOrAuto | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/rx-*.svg` | ✅ |
| 32 | ry | NonNegativeLengthPercentageOrAuto | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/ry-*.svg` | ✅ |
| 33 | x | LengthPercentage | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/x-*.svg` | ✅ |
| 34 | y | LengthPercentage | `engine=gecko` → `servo_restyle_damage=repaint` | `svg/geometry/parsing/y-*.svg` | ✅ |
| 35 | mask-position-x | HorizontalPosition | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-position-*.html` | ✅ |
| 36 | mask-position-y | VerticalPosition | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-position-*.html` | ✅ |
| 37 | mask-repeat | BackgroundRepeat | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-repeat-*.html` | ✅ |
| 38 | mask-size | BackgroundSize | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-size-*.html` | ✅ |
| 39 | mask-type | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-type-*.html` | ✅ |
| 40 | mask-mode | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | rendering tests only | ❌ |
| 41 | mask-clip | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | — | ❌ |
| 42 | mask-origin | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | — | ❌ |
| 43 | mask-composite | keyword | `engine=gecko` → `servo_restyle_damage=repaint` | `css/css-masking/parsing/mask-composite-*.html` | ✅ |

### Special Cases

| # | Property | Change | Test Location | Test Status |
|---|---|---|---|---|
| 44 | mask-image | `-servo_pref = "layout.unimplemented"` (no `engine=gecko` — was already enabled but gated behind pref) | `css/css-masking/parsing/mask-image-computed.html` | ✅ |
| 45 | dominant-baseline | `engine=gecko` removed (in `inherited_box` struct, not SVG-specific) | `css/css-inline/parsing/dominant-baseline-*.html` | ✅ |

### Shorthand Changes

| Shorthand | File | Change |
|---|---|---|
| `marker` | `shorthands.toml` | `-engine = "gecko"` → available in Servo |
| `mask` | `shorthands.toml` | `-engine = "gecko"` → available in Servo |
| `mask-position` | `shorthands.toml` | `-engine = "gecko"` → available in Servo |

### Code Changes in `shorthands.rs`

| Change | Description |
|---|---|
| `#[cfg(feature = "gecko")]` removed from `pub mod marker` | `marker` shorthand module compiled for Servo |
| `#[cfg(feature = "gecko")]` removed from `pub mod mask` | `mask` shorthand module compiled for Servo |
| `#[cfg(feature = "gecko")]` removed from `pub mod mask_position` | `mask-position` shorthand module compiled for Servo |
| `#[cfg(not(feature = "gecko"))]` impl `From<mask_origin> for mask_clip` | Servo-only conversion that excludes Gecko keywords (`fill-box`, `stroke-box`, `view-box`) |
| Conditional `NoClip` in mask serialization | Servo skips clip when `NoClip` (Gecko has extra keywords like `fill-box` etc. that map to `NoClip`) |

### Size Change

| File | Change | Meaning |
|---|---|---|
| `properties.mako.rs` | `size_of_test!(ComputedValues, 224)` → `232` | `ComputedValues` increased by 8 bytes to hold the newly included SVG style fields |

---

## Test Coverage Status

### Legend
- ✅ Full coverage: computed + valid + invalid parsing tests exist
- ⚠️ Partial: only invalid-value test exists
- ❌ No dedicated parsing/computed tests

### Properties With No Dedicated Parsing/Computed Tests

These properties are untested for parsing/computed-style behavior:

1. **`d`** — the SVG path data property. Tests exist only for animation/interpolation (`svg/path/property/d-interpolation-*.svg`), not for parsing or computed values
2. **`vector-effect`** — only `svg/styling/vector-effect-invalid.html` (tests that invalid values are rejected). No valid or computed test
3. **`mask-mode`** — has rendering tests (`css/css-masking/mask-image/mask-mode-*.html`) but no dedicated parsing test
4. **`mask-clip`** — has rendering tests (`css/css-masking/mask-image/mask-clip-*.html`) but no dedicated parsing test
5. **`mask-origin`** — has rendering tests (`css/css-masking/mask-image/mask-origin-*.html`) but no dedicated parsing test

---

## Testing Methodology

### Tool
```bash
./mach test-wpt <test-path>
```
Uses Servo's built-in WPT runner which evaluates test files in a browser-like environment.

### How Tests Work

Tests in `svg/painting/parsing/` and similar directories use shared helpers from `computed-testcommon.js` and `parsing-testcommon.js`:

| Helper | What it does |
|---|---|
| `test_computed_value(property, specified, expected?)` | Sets `target.style[property]` to `specified`, reads `getComputedStyle(target)[property]`, asserts match |
| `test_valid_value(property, value, expected?)` | Verifies the CSS parser *accepts* the value |
| `test_invalid_value(property, value)` | Verifies the CSS parser *rejects* the value (falls back to initial) |

These tests require only a DOM element and `getComputedStyle` — no rendering or layout.

### Test Batches Run

| Batch | Files | Tests Run | Result |
|---|---|---|---|
| `svg/painting/parsing/` | 50+ SVG files | 61 | ✅ All passed |
| `svg/geometry/parsing/` | 15+ SVG files | ~22 | ✅ All passed |
| `svg/pservers/parsing/` | 6 SVG files | ~6 | ✅ All passed |
| `svg/text/parsing/` | text-anchor only | ~4 | ✅ All passed |
| `css/css-masking/parsing/` | 25+ HTML files | ~25 | ✅ All passed (where Servo supports the test format) |
| `css/filter-effects/parsing/` | 12+ files | ~12 | ✅ All passed |
| `css/css-inline/parsing/` | dominant-baseline | 3 | ✅ All passed |
| **Total** | **~65+ test files** | **~130+ tests** | **✅ All passed** |

### Extracting Test Commands

Test files are organized by spec section:
- `svg/painting/parsing/` — fill, stroke, paint-order, markers, color-interpolation, shape-rendering
- `svg/geometry/parsing/` — cx, cy, r, rx, ry, x, y, width, height
- `svg/pservers/parsing/` — stop-color, stop-opacity
- `svg/text/parsing/` — text-anchor
- `css/css-masking/parsing/` — clip-rule, mask-* properties
- `css/filter-effects/parsing/` — flood-color, flood-opacity, lighting-color, color-interpolation-filters
- `css/css-inline/parsing/` — dominant-baseline

To run all SVG-related parsing tests:
```bash
./mach test-wpt svg/painting/parsing/ svg/geometry/parsing/ svg/pservers/parsing/ svg/text/parsing/ css/css-masking/parsing/ css/filter-effects/parsing/ css/css-inline/parsing/
```
