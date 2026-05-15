# SVG CSS Properties — Specification Coverage

Official W3C specification references and property implementation status for Phase 1 of the SVG engine project.

---

## 1. [SVG 2 — Chapter 7: Geometry](https://svgwg.org/svg2-draft/geometry.html)

Defines geometry properties for position and dimension of SVG graphics elements (`circle`, `ellipse`, `rect`, `image`, `foreignObject`, `svg`).

| Property | Type | Status |
|---|---|---|
| `cx` | CSS + Attribute | ✅ Done (SVG struct) |
| `cy` | CSS + Attribute | ✅ Done (SVG struct) |
| `r` | CSS + Attribute | ✅ Done (SVG struct) |
| `rx` | CSS + Attribute | ✅ Done (SVG struct) |
| `ry` | CSS + Attribute | ✅ Done (SVG struct) |
| `x` | CSS + Attribute | ✅ Done (SVG struct) |
| `y` | CSS + Attribute | ✅ Done (SVG struct) |
| `width` | CSS + Attribute | ✅ Already existed (standard CSS) |
| `height` | CSS + Attribute | ✅ Already existed (standard CSS) |

**Done: 9/9** — 7 registered by us, 2 pre-existing

---

## 2. [SVG 2 — Chapter 12: Painting](https://www.w3.org/TR/SVG2/painting.html)

Filling, Stroking and Marker Symbols. The primary spec for visual rendering properties.

### Fill properties

| Property | Type | Status |
|---|---|---|
| `fill` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `fill-rule` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `fill-opacity` | CSS + Attribute | ✅ Done (InheritedSVG) |

### Stroke properties

| Property | Type | Status |
|---|---|---|
| `stroke` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-width` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-opacity` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-linecap` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-linejoin` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-miterlimit` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-dasharray` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `stroke-dashoffset` | CSS + Attribute | ✅ Done (InheritedSVG) |

### Marker properties

| Property | Type | Status |
|---|---|---|
| `marker-start` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `marker-mid` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `marker-end` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `marker` (shorthand) | CSS shorthand | ✅ Done (shorthands.toml) |

### Painting support properties

| Property | Type | Status |
|---|---|---|
| `paint-order` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `text-anchor` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `color-interpolation` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `color-interpolation-filters` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `shape-rendering` | CSS + Attribute | ✅ Done (InheritedSVG) |
| `vector-effect` | CSS + Attribute | ✅ Done (SVG struct) |
| `opacity` | CSS + Attribute | ✅ Already existed (standard CSS) |

### Text rendering

| Property | Type | Status |
|---|---|---|
| `text-rendering` | CSS + Attribute | ✅ Already existed (no gate) |

**Done: 21/21** — 19 registered by us, 2 pre-existing

---

## 3. [SVG 2 — Chapter 6: Styling](https://www.w3.org/TR/SVG2/styling.html)

Defines how SVG properties interact with CSS, including the presentation attributes model. The key section is **§6.6** which lists all properties that can be used as presentation attributes.

Properties listed here but *not* covered by Chapters 7 or 12 above:

### Standard CSS properties (already existed)

| Property | Type | Status |
|---|---|---|
| `color` | CSS + Attribute | ✅ Already existed |
| `display` | CSS + Attribute | ✅ Already existed |
| `visibility` | CSS + Attribute | ✅ Already existed |
| `overflow` | CSS + Attribute | ✅ Already existed |
| `cursor` | CSS + Attribute | ✅ Already existed |
| `direction` | CSS + Attribute | ✅ Already existed |
| `unicode-bidi` | CSS + Attribute | ✅ Already existed |
| `writing-mode` | CSS + Attribute | ✅ Already existed |
| `font-family` | CSS + Attribute | ✅ Already existed |
| `font-size` | CSS + Attribute | ✅ Already existed |
| `font-size-adjust` | CSS + Attribute | ✅ Already existed |
| `font-stretch` | CSS + Attribute | ✅ Already existed |
| `font-style` | CSS + Attribute | ✅ Already existed |
| `font-variant` | CSS + Attribute | ✅ Already existed |
| `font-weight` | CSS + Attribute | ✅ Already existed |
| `text-decoration` | CSS + Attribute | ✅ Already existed |
| `text-overflow` | CSS + Attribute | ✅ Already existed |
| `white-space` | CSS + Attribute | ✅ Already existed |
| `word-spacing` | CSS + Attribute | ✅ Already existed |
| `letter-spacing` | CSS + Attribute | ✅ Already existed |
| `pointer-events` | CSS + Attribute | ✅ Already existed |
| `filter` | CSS + Attribute | ✅ Already existed |

### SVG-specific properties (already existed, no gate)

| Property | Type | Status |
|---|---|---|
| `alignment-baseline` | CSS + Attribute | ✅ Already existed (no gate) |
| `baseline-shift` | CSS + Attribute | ✅ Already existed (no gate) |

### SVG-specific properties (needing work)

| Property | Type | Status |
|---|---|---|
| `dominant-baseline` | CSS + Attribute | ✅ Done (gate removed) |
| `color-rendering` | CSS + Attribute | ⏭️ Removed from SVG 2 spec — no browser implements |
| `glyph-orientation-horizontal` | CSS + Attribute | ⏭️ Removed from SVG 2 spec — no browser implements |
| `glyph-orientation-vertical` | CSS + Attribute | ⏭️ Deprecated in SVG 2 — no browser implements |

**Done: 25/25** — all implementable properties enabled. 3 removed/deprecated properties skipped.

---

## 4. [CSS Masking Module Level 1](https://www.w3.org/TR/css-masking-1/)

Defines clipping (`clip-path`) and masking (`mask-*`) properties.

### Clipping

| Property | Type | Status |
|---|---|---|
| `clip-path` | CSS + Attribute | ✅ Done (SVG struct) |
| `clip-rule` | CSS + Attribute | ✅ Done (InheritedSVG) |

### Masking

| Property | Type | Status |
|---|---|---|
| `mask-image` | CSS + Attribute | ✅ Done (SVG struct, pref removed) |
| `mask-type` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-mode` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-clip` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-origin` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-composite` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-position-x` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-position-y` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-repeat` | CSS + Attribute | ✅ Done (SVG struct) |
| `mask-size` | CSS + Attribute | ✅ Done (SVG struct) |

### Shorthand properties

| Property | Type | Status |
|---|---|---|
| `mask` | CSS shorthand | ✅ Done (gate removed) |
| `mask-position` | CSS shorthand | ✅ Done (gate removed) |

### Border mask (not in scope)

| Property | Type | Status |
|---|---|---|
| `mask-border-source` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border-mode` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border-slice` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border-width` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border-outset` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border-repeat` | CSS + Attribute | ⏭️ Not in scope |
| `mask-border` (shorthand) | CSS shorthand | ⏭️ Not in scope |

**Done: 14/14** (main properties). Border mask: 7 properties not in scope.

---

## Summary

| Spec | Total | ✅ Done | ❌ Remaining |
|---|---|---|---|
| Ch 7: Geometry | 9 | 9 | **0** |
| Ch 12: Painting | 21 | 21 | **0** |
| Ch 6: Styling (extra) | 25 | 25 | **0** |
| CSS Masking L1 | 14 (main) | 14 | **0** |
| **TOTAL** | **69** | **69** | **0** 🎉 |

### Phase 1 complete

All SVG CSS properties that are defined in current W3C specifications have been registered in Stylo and verified to parse and compute. Three SVG 1.1 properties were intentionally skipped because they were removed from SVG 2 and are not implemented by any browser:

| Skipped property | Reason |
|---|---|
| `color-rendering` | Removed from SVG 2 |
| `glyph-orientation-horizontal` | Removed from SVG 2 |
| `glyph-orientation-vertical` | Deprecated in SVG 2 |

---

## Reference Links

- [SVG 2 — Full Specification](https://www.w3.org/TR/SVG2/)
- [SVG 2 — Chapter 6: Styling](https://www.w3.org/TR/SVG2/styling.html)
- [SVG 2 — Chapter 7: Geometry](https://svgwg.org/svg2-draft/geometry.html)
- [SVG 2 — Chapter 12: Painting](https://www.w3.org/TR/SVG2/painting.html)
- [CSS Masking Module Level 1](https://www.w3.org/TR/css-masking-1/)
- [CSS Fill and Stroke Module Level 3](https://www.w3.org/TR/fill-stroke-3/)
