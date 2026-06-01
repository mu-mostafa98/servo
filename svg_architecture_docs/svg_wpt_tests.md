# SVG WPT Tests — Detailed Reference

## How Tests Work

All tests in `svg/painting/parsing/` use a shared `<g id="target">` element and test helpers:

| Helper | Source | What it does |
|---|---|---|
| `test_computed_value(property, specified, expected?)` | `computed-testcommon.js` | Sets `target.style[property] = specified`, reads `getComputedStyle(target)[property]`, asserts match |
| `test_valid_value(property, value, expected?)` | `parsing-testcommon.js` | Verifies the CSS parser **accepts** the value |
| `test_invalid_value(property, value)` | `parsing-testcommon.js` | Verifies the CSS parser **rejects** the value (falls back to initial) |

These tests need only an SVG `<g>` element and `getComputedStyle` — no rendering, no layout.

---

## 1. `fill`

**Command:** `./mach test-wpt svg/painting/parsing/fill-computed.svg svg/painting/parsing/fill-valid.svg svg/painting/parsing/fill-invalid.svg`

### fill-computed.svg
- **How it tests:** Sets `target.style.fill` to various values, reads `getComputedStyle(target).fill`, verifies the computed value matches.
- **Tested values:** `none`, `rgb(12, 34, 56)`, URL values (made absolute), `url() none`, `url() rgb(...)`.

### fill-valid.svg
- **How it tests:** Sets valid `fill` values, verifies the parser accepts them (value is not empty/initial).
- **Tested values:** `none`, `context-fill`, `context-stroke`, `rgb(12, 34, 56)`, `url("https://example.com/")`, `url(...) none`, `url(...) rgb(...)`.

### fill-invalid.svg
- **How it tests:** Sets invalid `fill` values, verifies the parser rejects them (falls back to initial value).
- **Tested values:** Empty string, garbage strings, malformed colors.

---

## 2. `fill-opacity`

**Command:** `./mach test-wpt svg/painting/parsing/fill-opacity-computed.svg svg/painting/parsing/fill-opacity-valid.svg svg/painting/parsing/fill-opacity-invalid.svg`

### fill-opacity-computed.svg
- **How it tests:** Sets `target.style.fillOpacity`, reads computed value back.
- **Tested values:** Clamping (`-1` → `0`, `3` → `1`), percentage conversion (`50%` → `0.5`), decimal values.

### fill-opacity-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** Numbers in [0,1], percentages (0%–100%), `calc()` expressions.

### fill-opacity-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Negative percentages, out-of-range values, garbage strings.

---

## 3. `fill-rule`

**Command:** `./mach test-wpt svg/painting/parsing/fill-rule-computed.svg svg/painting/parsing/fill-rule-valid.svg svg/painting/parsing/fill-rule-invalid.svg`

### fill-rule-computed.svg
- **How it tests:** Sets `target.style.fillRule`, reads computed value back.
- **Tested values:** `nonzero`, `evenodd`.

### fill-rule-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `nonzero`, `evenodd`.

### fill-rule-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** `inherit` (→ `nonzero`), garbage strings.

---

## 4. `stroke`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-computed.svg svg/painting/parsing/stroke-valid.svg svg/painting/parsing/stroke-invalid.svg`

### stroke-computed.svg
- **How it tests:** Sets `target.style.stroke`, reads computed value back.
- **Tested values:** `none`, `rgb(12, 34, 56)`, URL values (made absolute), `url() color`.

### stroke-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `none`, `context-fill`, `context-stroke`, `rgb(...)`, `url(...)`, `url(...) none`, `url(...) rgb(...)`.

### stroke-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Empty string, garbage, malformed URLs.

---

## 5. `stroke-opacity`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-opacity-computed.svg svg/painting/parsing/stroke-opacity-valid.svg svg/painting/parsing/stroke-opacity-invalid.svg`

### stroke-opacity-computed.svg
- **How it tests:** Sets `target.style.strokeOpacity`, reads computed value back.
- **Tested values:** Clamping (`-2` → `0`, `5` → `1`), percentage conversion, decimal values.

### stroke-opacity-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** Numbers in [0,1], percentages, `calc()`.

### stroke-opacity-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Out-of-range percentages, garbage.

---

## 6. `stroke-width`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-width-computed.svg svg/painting/parsing/stroke-width-valid.svg svg/painting/parsing/stroke-width-invalid.svg`

### stroke-width-computed.svg
- **How it tests:** Sets `target.style.strokeWidth`, reads computed value back.
- **Tested values:** Unitless `10` → `10px`, `calc` with `em` resolves, `40%` stays as `%`, `calc(50% + 60px)`, all length units (`em`, `ex`, `cm`, `in`, `pt`, `pc`, `px`) compute the same as for `text-indent`.

### stroke-width-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** Lengths (`1px`, `0`, `1em`), percentages, `calc()`.

### stroke-width-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Negative lengths, angles, garbage.

---

## 7. `stroke-linecap`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-linecap-computed.svg svg/painting/parsing/stroke-linecap-valid.svg svg/painting/parsing/stroke-linecap-invalid.svg`

### stroke-linecap-computed.svg
- **How it tests:** Sets `target.style.strokeLinecap`, reads computed value back.
- **Tested values:** `butt`, `round`, `square`.

### stroke-linecap-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `butt`, `round`, `square`.

### stroke-linecap-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, `inherit`, garbage.

---

## 8. `stroke-linejoin`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-linejoin-computed.svg svg/painting/parsing/stroke-linejoin-valid.svg svg/painting/parsing/stroke-linejoin-invalid.svg`

### stroke-linejoin-computed.svg
- **How it tests:** Sets `target.style.strokeLinejoin`, reads computed value back.
- **Tested values:** `miter`, `round`, `bevel`.

### stroke-linejoin-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `miter`, `miter-clip`, `round`, `bevel`, `arcs`.

### stroke-linejoin-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 9. `stroke-dasharray`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-dasharray-computed.svg svg/painting/parsing/stroke-dasharray-valid.svg svg/painting/parsing/stroke-dasharray-invalid.svg`

### stroke-dasharray-computed.svg
- **How it tests:** Sets `target.style.strokeDasharray`, reads computed value back.
- **Tested values:** `none`, `10` → `10px`, `calc` with `em` resolves, `40%` stays, comma-separated lists normalize.

### stroke-dasharray-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `none`, `10px`, `20%`, `calc(2em + 3ex)`, comma-separated lists, `0, 5`, `calc()` expressions.

### stroke-dasharray-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Empty array, negative values, garbage.

---

## 10. `stroke-dashoffset`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-dashoffset-computed.svg svg/painting/parsing/stroke-dashoffset-valid.svg svg/painting/parsing/stroke-dashoffset-invalid.svg`

### stroke-dashoffset-computed.svg
- **How it tests:** Sets `target.style.strokeDashoffset`, reads computed value back.
- **Tested values:** `10` → `10px`, `0.5em` → `20px`, `calc`, absolute length conversions (`254cm` = `9600px`), negative values, percentages.

### stroke-dashoffset-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** Lengths, percentages, `calc()`.

### stroke-dashoffset-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Angles, times, garbage.

---

## 11. `stroke-miterlimit`

**Command:** `./mach test-wpt svg/painting/parsing/stroke-miterlimit-computed.svg svg/painting/parsing/stroke-miterlimit-valid.svg svg/painting/parsing/stroke-miterlimit-invalid.svg`

### stroke-miterlimit-computed.svg
- **How it tests:** Sets `target.style.strokeMiterlimit`, reads computed value back.
- **Tested values:** `0`, `0.5`, `1`, `7.5`.

### stroke-miterlimit-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** Numbers ≥ 0.

### stroke-miterlimit-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Negative numbers, garbage.

---

## 12. `paint-order`

**Command:** `./mach test-wpt svg/painting/parsing/paint-order-computed.svg svg/painting/parsing/paint-order-valid.svg svg/painting/parsing/paint-order-invalid.svg`

### paint-order-computed.svg
- **How it tests:** Sets `target.style.paintOrder`, reads computed value back.
- **Tested values:** `normal`, single keywords (`fill`, `stroke`, `markers`), all 2-keyword and 3-keyword combinations. Trailing implied keywords are dropped (e.g., `fill stroke markers` → `fill`).

### paint-order-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `normal`, all keyword combinations (`fill stroke`, `stroke markers`, `fill stroke markers`, etc.).

### paint-order-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Unknown keywords, duplicates, garbage.

### paint-order-computed-value-01.svg (special — tests both style AND presentation attribute)
- **Command:** `./mach test-wpt svg/painting/scripted/paint-order-computed-value-01.svg`
- **How it tests:** Similar to computed tests but also sets `paint-order` as a **presentation attribute** on `<text>` elements and reads the computed style. Tests that both pathways (CSS `style` property and SVG `paint-order="..."` attribute) produce the same computed values.
- **Tested values:** Same as paint-order-computed.svg but through both pathways.

---

## 13. `shape-rendering`

**Command:** `./mach test-wpt svg/painting/parsing/shape-rendering-computed.svg svg/painting/parsing/shape-rendering-valid.svg svg/painting/parsing/shape-rendering-invalid.svg`

### shape-rendering-computed.svg
- **How it tests:** Sets `target.style.shapeRendering`, reads computed value back.
- **Tested values:** `auto`, `optimizeSpeed`, `crispEdges`, `geometricPrecision`.

### shape-rendering-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `auto`, `optimizeSpeed`, `crispEdges`, `geometricPrecision`.

### shape-rendering-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 14. `color-interpolation`

**Command:** `./mach test-wpt svg/painting/parsing/color-interpolation-computed.svg svg/painting/parsing/color-interpolation-valid.svg svg/painting/parsing/color-interpolation-invalid.svg`

### color-interpolation-computed.svg
- **How it tests:** Sets `target.style.colorInterpolation`, reads computed value back.
- **Tested values:** `auto`, `sRGB`, `linearRGB`.

### color-interpolation-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `auto`, `sRGB`, `linearRGB`.

### color-interpolation-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 15. `image-rendering`

**Command:** `./mach test-wpt svg/painting/parsing/image-rendering-computed.svg svg/painting/parsing/image-rendering-valid.svg svg/painting/parsing/image-rendering-invalid.svg`

### image-rendering-computed.svg
- **How it tests:** Sets `target.style.imageRendering`, reads computed value back.
- **Tested values:** `auto`, `smooth`, `high-quality`, `crisp-edges`, `pixelated`.

### image-rendering-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `auto`, `smooth`, `high-quality`, `crisp-edges`, `pixelated`.

### image-rendering-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 16. `text-rendering`

**Command:** `./mach test-wpt svg/painting/parsing/text-rendering-computed.svg svg/painting/parsing/text-rendering-valid.svg svg/painting/parsing/text-rendering-invalid.svg`

### text-rendering-computed.svg
- **How it tests:** Sets `target.style.textRendering`, reads computed value back.
- **Tested values:** `auto`, `optimizeSpeed`, `optimizeLegibility`, `geometricPrecision`.

### text-rendering-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `auto`, `optimizeSpeed`, `optimizeLegibility`, `geometricPrecision`.

### text-rendering-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 17. Marker Properties

**Note:** The `marker` shorthand sets all three individual marker properties (`marker-start`, `marker-mid`, `marker-end`).

**Command:** `./mach test-wpt svg/painting/parsing/marker-computed.svg svg/painting/parsing/marker-valid.svg svg/painting/parsing/marker-invalid.svg svg/painting/parsing/marker-shorthand.svg svg/painting/parsing/marker-start-computed.svg svg/painting/parsing/marker-start-valid.svg svg/painting/parsing/marker-start-invalid.svg svg/painting/parsing/marker-mid-computed.svg svg/painting/parsing/marker-mid-valid.svg svg/painting/parsing/marker-mid-invalid.svg svg/painting/parsing/marker-end-computed.svg svg/painting/parsing/marker-end-valid.svg svg/painting/parsing/marker-end-invalid.svg`

### marker-computed.svg
- **How it tests:** Sets `target.style.marker`, reads computed value back.
- **Tested values:** `none`, URL values.

### marker-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `none`, URL values.

### marker-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Garbage.

### marker-shorthand.svg
- **How it tests:** Tests the `marker` shorthand sets individual marker properties.
- **Tested values:** `none`, URL values. Checks `marker-start`, `marker-mid`, `marker-end` are set correctly.

### marker-start/marker-mid/marker-end-computed.svg
- **How it tests:** Sets `target.style.markerStart` / `markerMid` / `markerEnd`, reads computed value back.
- **Tested values:** `none`, URL values (made absolute).

### marker-start/marker-mid/marker-end-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `none`, URL values.

### marker-start/marker-mid/marker-end-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Garbage.

---

## 18. `text-anchor`

**Command:** `./mach test-wpt svg/text/parsing/text-anchor-computed.svg svg/text/parsing/text-anchor-valid.svg svg/text/parsing/text-anchor-invalid.svg`

### text-anchor-computed.svg
- **How it tests:** Sets `target.style.textAnchor`, reads computed value back.
- **Tested values:** `start`, `middle`, `end`.

### text-anchor-valid.svg
- **How it tests:** Parsing acceptance.
- **Tested values:** `start`, `middle`, `end`.

### text-anchor-invalid.svg
- **How it tests:** Parsing rejection.
- **Tested values:** Misspellings, garbage.

---

## 19. Geometry Properties (cx, cy, r, rx, ry, x, y)

**Command:** `./mach test-wpt svg/geometry/parsing/cx-computed.svg svg/geometry/parsing/cx-valid.svg svg/geometry/parsing/cx-invalid.svg svg/geometry/parsing/cy-computed.svg svg/geometry/parsing/cy-valid.svg svg/geometry/parsing/cy-invalid.svg svg/geometry/parsing/r-computed.svg svg/geometry/parsing/r-valid.svg svg/geometry/parsing/r-invalid.svg svg/geometry/parsing/rx-computed.svg svg/geometry/parsing/rx-valid.svg svg/geometry/parsing/rx-invalid.svg svg/geometry/parsing/ry-computed.svg svg/geometry/parsing/ry-valid.svg svg/geometry/parsing/ry-invalid.svg svg/geometry/parsing/x-computed.svg svg/geometry/parsing/x-valid.svg svg/geometry/parsing/x-invalid.svg svg/geometry/parsing/y-computed.svg svg/geometry/parsing/y-valid.svg svg/geometry/parsing/y-invalid.svg`

### cx, cy
- **How it tests:** Sets `target.style.cx` / `cy`, reads computed value back. Tests parsing of valid values, and rejection of invalid values.
- **Tested values:** Lengths (`10px`, `10` → `10px`), percentages (`50%`), `calc()`.

### r, rx, ry
- **How it tests:** Sets `target.style.r` / `rx` / `ry`, reads computed value back.
- **Tested values:** Non-negative lengths, percentages, `auto` (for rx/ry).

### x, y
- **How it tests:** Sets `target.style.x` / `y`, reads computed value back.
- **Tested values:** Lengths, percentages, `calc()`.

---

## 20. Paint Server Properties (stop-color, stop-opacity)

**Command:** `./mach test-wpt svg/pservers/parsing/stop-color-computed.svg svg/pservers/parsing/stop-color-valid.svg svg/pservers/parsing/stop-color-invalid.svg svg/pservers/parsing/stop-opacity-computed.svg svg/pservers/parsing/stop-opacity-valid.svg svg/pservers/parsing/stop-opacity-invalid.svg`

### stop-color
- **How it tests:** Sets `target.style.stopColor`, reads computed value back.
- **Tested values:** Named colors, `rgb()`, hex colors, `currentColor`.

### stop-opacity
- **How it tests:** Sets `target.style.stopOpacity`, reads computed value back.
- **Tested values:** Numbers in [0,1], percentages.

---

## 21. Filter Effect Properties (flood-color, flood-opacity, lighting-color, color-interpolation-filters)

**Command:** `./mach test-wpt css/filter-effects/parsing/flood-color-computed.html css/filter-effects/parsing/flood-color-valid.html css/filter-effects/parsing/flood-color-invalid.html css/filter-effects/parsing/flood-opacity-computed.svg css/filter-effects/parsing/flood-opacity-valid.svg css/filter-effects/parsing/flood-opacity-invalid.svg css/filter-effects/parsing/lighting-color-computed.html css/filter-effects/parsing/lighting-color-parsing-valid.html css/filter-effects/parsing/lighting-color-parsing-invalid.html css/filter-effects/parsing/color-interpolation-filters-computed.html css/filter-effects/parsing/color-interpolation-filters-parsing-valid.html css/filter-effects/parsing/color-interpolation-filters-parsing-invalid.html`

### flood-color
- **How it tests:** Sets `target.style.floodColor`, reads computed value back.
- **Tested values:** Named colors, `rgb()`, hex colors.

### flood-opacity
- **How it tests:** Sets `target.style.floodOpacity`, reads computed value back.
- **Tested values:** Numbers in [0,1], percentages.

### lighting-color
- **How it tests:** Sets `target.style.lightingColor`, reads computed value back.
- **Tested values:** Named colors, `rgb()`, hex colors.

### color-interpolation-filters
- **How it tests:** Sets `target.style.colorInterpolationFilters`, reads computed value back.
- **Tested values:** `auto`, `sRGB`, `linearRGB`.

---

## 22. CSS Masking Properties (clip-rule, mask-type, mask-composite, mask-image, mask-repeat, mask-size, mask-position)

**Command:** `./mach test-wpt css/css-masking/parsing/clip-rule-computed.html css/css-masking/parsing/clip-rule-valid.html css/css-masking/parsing/clip-rule-invalid.html css/css-masking/parsing/mask-type-computed.html css/css-masking/parsing/mask-type-valid.html css/css-masking/parsing/mask-type-invalid.html css/css-masking/parsing/mask-composite-computed.html css/css-masking/parsing/mask-composite-valid.html css/css-masking/parsing/mask-composite-invalid.html css/css-masking/parsing/mask-image-computed.html css/css-masking/parsing/mask-repeat-computed.html css/css-masking/parsing/mask-repeat-valid.html css/css-masking/parsing/mask-repeat-invalid.html css/css-masking/parsing/mask-size-computed.html css/css-masking/parsing/mask-size-valid.html css/css-masking/parsing/mask-size-invalid.html css/css-masking/parsing/mask-position-valid.html css/css-masking/parsing/mask-position-invalid.html css/css-masking/parsing/mask-computed.html css/css-masking/parsing/mask-valid.sub.html css/css-masking/parsing/mask-invalid.html`

### clip-rule
- **How it tests:** Sets `target.style.clipRule`, reads computed value back.
- **Tested values:** `nonzero`, `evenodd`.

### mask-type
- **How it tests:** Sets `target.style.maskType`, reads computed value back.
- **Tested values:** `luminance`, `alpha`.

### mask-composite
- **How it tests:** Sets `target.style.maskComposite`, reads computed value back.
- **Tested values:** `add`, `subtract`, `intersect`, `exclude`.

### mask-image
- **How it tests:** Sets `target.style.maskImage`, reads computed value back.
- **Tested values:** `none`, URL values.

### mask-repeat
- **How it tests:** Sets `target.style.maskRepeat`, reads computed value back.
- **Tested values:** `repeat`, `repeat-x`, `repeat-y`, `space`, `round`, `no-repeat`.

### mask-size
- **How it tests:** Sets `target.style.maskSize`, reads computed value back.
- **Tested values:** `auto`, `contain`, `cover`, length/percentage values.

### mask-position
- **How it tests:** Sets `target.style.maskPosition`, reads computed value back.
- **Tested values:** Keywords (`center`, `top`, `left`), length/percentage combinations.

### mask (shorthand)
- **How it tests:** Sets the `mask` shorthand, reads back individual longhands.
- **Tested values:** Combinations of mask-image, mask-mode, etc.

---

## 23. `dominant-baseline`

**Command:** `./mach test-wpt css/css-inline/parsing/dominant-baseline-computed.html css/css-inline/parsing/dominant-baseline-valid.html css/css-inline/parsing/dominant-baseline-invalid.html`

### dominant-baseline-computed.html
- **How it tests:** Sets `target.style.dominantBaseline`, reads computed value back.
- **Tested values:** `auto`, `alphabetic`, `central`, `hanging`, `ideographic`, `mathematical`, `middle`, `no-change`, `reset-size`, `text-after-edge`, `text-before-edge`, `use-script`.

### dominant-baseline-valid.html
- **How it tests:** Parsing acceptance.
- **Tested values:** All keyword values.

### dominant-baseline-invalid.html
- **How it tests:** Parsing rejection.
- **Tested values:** Garbage strings, misspellings.

---

## 24. `vector-effect`

**Command:** `./mach test-wpt svg/styling/vector-effect-invalid.html`

### vector-effect-invalid.html
- **How it tests:** Sets invalid `vector-effect` values, verifies the parser rejects them.
- **Tested values:** Invalid strings like `"non-scaling-stroke"`, `"none-scaling"`, `"non-scaling"`, empty values, numeric inputs.
- **Note:** No `*-computed.svg` or `*-valid.svg` tests exist for `vector-effect` in WPT. This is the only available WPT test for this property.

---

## 25. `required-properties.svg` — Property Existence Check

**Command:** `./mach test-wpt svg/styling/required-properties.svg`

### What it tests
This is a property existence test (not a computed-value test). For each property in the SVG specification property index, it checks that `propertyName in element.style` returns `true`. This verifies Stylo recognizes the property at all.

### Properties it verifies exist
All SVG CSS properties including: `alignment-baseline`, `baseline-shift`, `clip`, `clip-path`, `clip-rule`, `color`, `color-interpolation`, `color-interpolation-filters`, `cursor`, `direction`, `display`, `dominant-baseline`, `fill`, `fill-opacity`, `fill-rule`, `filter`, `flood-color`, `flood-opacity`, `font` (shorthand), `glyph-orientation-vertical`, `image-rendering`, `letter-spacing`, `lighting-color`, `line-height`, `marker` (shorthand), `marker-end`, `marker-mid`, `marker-start`, `mask`, `opacity`, `overflow`, `paint-order`, `pointer-events`, `shape-rendering`, `stop-color`, `stop-opacity`, `stroke`, `stroke-dasharray`, `stroke-dashoffset`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit`, `stroke-opacity`, `stroke-width`, `text-anchor`, `text-decoration`, `text-rendering`, `unicode-bidi`, `vector-effect`, `visibility`, `word-spacing`, `writing-mode`, `transform`, `transform-box`, `transform-origin`, `isolation`, `vertical-align`.

---

## Coverage Summary

### All 45 SVG CSS Properties — Test Coverage Status

#### Full coverage (computed + valid + invalid) — 32 properties

| # | Property | Test Location |
|---|---|---|
| 1 | `fill` | `svg/painting/parsing/` |
| 2 | `fill-opacity` | `svg/painting/parsing/` |
| 3 | `fill-rule` | `svg/painting/parsing/` |
| 4 | `stroke` | `svg/painting/parsing/` |
| 5 | `stroke-opacity` | `svg/painting/parsing/` |
| 6 | `stroke-width` | `svg/painting/parsing/` |
| 7 | `stroke-linecap` | `svg/painting/parsing/` |
| 8 | `stroke-linejoin` | `svg/painting/parsing/` |
| 9 | `stroke-dasharray` | `svg/painting/parsing/` |
| 10 | `stroke-dashoffset` | `svg/painting/parsing/` |
| 11 | `stroke-miterlimit` | `svg/painting/parsing/` |
| 12 | `paint-order` | `svg/painting/parsing/` + `svg/painting/scripted/` |
| 13 | `shape-rendering` | `svg/painting/parsing/` |
| 14 | `color-interpolation` | `svg/painting/parsing/` |
| 15 | `image-rendering` | `svg/painting/parsing/` |
| 16 | `text-rendering` | `svg/painting/parsing/` |
| 17 | `marker` (shorthand) | `svg/painting/parsing/` |
| 18 | `marker-start` | `svg/painting/parsing/` |
| 19 | `marker-mid` | `svg/painting/parsing/` |
| 20 | `marker-end` | `svg/painting/parsing/` |
| 21 | `text-anchor` | `svg/text/parsing/` |
| 22 | `cx` | `svg/geometry/parsing/` |
| 23 | `cy` | `svg/geometry/parsing/` |
| 24 | `r` | `svg/geometry/parsing/` |
| 25 | `rx` | `svg/geometry/parsing/` |
| 26 | `ry` | `svg/geometry/parsing/` |
| 27 | `x` | `svg/geometry/parsing/` |
| 28 | `y` | `svg/geometry/parsing/` |
| 29 | `stop-color` | `svg/pservers/parsing/` |
| 30 | `stop-opacity` | `svg/pservers/parsing/` |
| 31 | `flood-color` | `css/filter-effects/parsing/` |
| 32 | `flood-opacity` | `css/filter-effects/parsing/` |

#### Full coverage (CSS tests) — 6 properties

| # | Property | Test Location |
|---|---|---|
| 33 | `clip-rule` | `css/css-masking/parsing/` |
| 34 | `color-interpolation-filters` | `css/filter-effects/parsing/` |
| 35 | `lighting-color` | `css/filter-effects/parsing/` |
| 36 | `dominant-baseline` | `css/css-inline/parsing/` |
| 37 | `mask-type` | `css/css-masking/parsing/` |
| 38 | `mask-composite` | `css/css-masking/parsing/` |

#### Computed-only coverage — 2 properties

| # | Property | Test Location | Notes |
|---|---|---|---|
| 39 | `mask-image` | `css/css-masking/parsing/mask-image-computed.html` | Only computed test (no valid/invalid) |
| 40 | `mask-size` | `css/css-masking/parsing/mask-size-*.html` | Computed + valid + invalid |

#### Limited or no dedicated parsing tests — 5 properties

| # | Property | Status | What exists |
|---|---|---|---|
| 41 | `vector-effect` | ⚠️ Invalid only | `svg/styling/vector-effect-invalid.html` (rejection only) + `required-properties.svg` (existence) |
| 42 | `mask-mode` | ❌ No parsing tests | Rendering tests only in `css/css-masking/mask-image/` |
| 43 | `mask-clip` | ❌ No parsing tests | Rendering tests only in `css/css-masking/mask-image/` |
| 44 | `mask-origin` | ❌ No parsing tests | Rendering tests only in `css/css-masking/mask-image/` |
| 45 | `d` (path data) | ❌ No parsing tests | Animation/interpolation tests only in `svg/path/property/` |

### Additional properties with computed tests (mask-position, mask-repeat)

| Property | Status | Location |
|---|---|---|
| `mask-position` | ✅ Valid + invalid | `css/css-masking/parsing/mask-position-*.html` |
| `mask-repeat` | ✅ Computed + valid + invalid | `css/css-masking/parsing/mask-repeat-*.html` |

**Note:** `mask-position-x` and `mask-position-y` are individual longhands tested via the `mask-position` shorthand tests.

---

## Quick Run Commands

```sh
# Run ALL SVG painting parsing tests
./mach test-wpt svg/painting/parsing/

# Run a single property's full suite (computed + valid + invalid)
./mach test-wpt svg/painting/parsing/fill-computed.svg svg/painting/parsing/fill-valid.svg svg/painting/parsing/fill-invalid.svg

# Run only computed value tests
./mach test-wpt svg/painting/parsing/*-computed.svg

# Run only valid value tests
./mach test-wpt svg/painting/parsing/*-valid.svg

# Run only invalid value tests
./mach test-wpt svg/painting/parsing/*-invalid.svg

# Run geometry parsing tests
./mach test-wpt svg/geometry/parsing/

# Run paint server parsing tests
./mach test-wpt svg/pservers/parsing/

# Run filter effects parsing tests
./mach test-wpt css/filter-effects/parsing/

# Run CSS masking parsing tests
./mach test-wpt css/css-masking/parsing/

# Run dominant-baseline tests
./mach test-wpt css/css-inline/parsing/dominant-baseline-computed.html css/css-inline/parsing/dominant-baseline-valid.html css/css-inline/parsing/dominant-baseline-invalid.html

# Run ALL SVG-related parsing tests
./mach test-wpt svg/painting/parsing/ svg/geometry/parsing/ svg/pservers/parsing/ svg/text/parsing/ css/css-masking/parsing/ css/filter-effects/parsing/ css/css-inline/parsing/
```

## Test Counts

| Category | Files | Properties covered |
|---|---|---|
| **SVG painting parsing** (sections 1–17) | 60 | fill, fill-opacity, fill-rule, stroke, stroke-opacity, stroke-width, stroke-linecap, stroke-linejoin, stroke-dasharray, stroke-dashoffset, stroke-miterlimit, paint-order, shape-rendering, color-interpolation, image-rendering, text-rendering, marker |
| **Text anchor** (section 18) | 3 | text-anchor |
| **Geometry** (section 19) | ~22 | cx, cy, r, rx, ry, x, y |
| **Paint servers** (section 20) | 6 | stop-color, stop-opacity |
| **Filter effects** (section 21) | 12 | flood-color, flood-opacity, lighting-color, color-interpolation-filters |
| **CSS masking** (section 22) | ~21 | clip-rule, mask-type, mask-composite, mask-image, mask-repeat, mask-size, mask-position, mask (shorthand) |
| **dominant-baseline** (section 23) | 3 | dominant-baseline |
| **Vector effect** (section 24) | 1 | vector-effect (invalid only) |
| **Property existence** (section 25) | 1 | All SVG CSS properties |
| **Total** | **~129** | **40 properties with tests, 5 without** |

**Results:** All testable properties pass. No dedicated parsing/computed tests exist for: `mask-mode`, `mask-clip`, `mask-origin`, `d`, and `vector-effect` (invalid only).
