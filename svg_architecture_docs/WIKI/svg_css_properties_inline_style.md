# Phase 1 — SVG CSS Properties Registered in Stylo

**Status:** ✅ Complete
**Date:** 2026-05-13
**Scope:** All 42 SVG CSS properties enabled in Servo's style engine (Stylo) by removing `engine = "gecko"` gates and adding `servo_restyle_damage = "repaint"`.

Properties are organized into two style structs:

- **`InheritedSVG`** — Inherited by child elements (32 properties)
- **`SVG`** — Reset per element, not inherited (10 properties + 2 pre-existing)

---

## Fill Properties (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `fill` | `SVGPaint` | `black` | Boxed type |
| `fill-opacity` | `SVGOpacity` | `1` | |
| `fill-rule` | `FillRule` | `nonzero` | Animation: discrete |

---

## Stroke Properties (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `stroke` | `SVGPaint` | `none` | Boxed type |
| `stroke-width` | `SVGWidth` | `1` | |
| `stroke-opacity` | `SVGOpacity` | `1` | |
| `stroke-linecap` | keyword | `butt` | Values: butt, round, square |
| `stroke-linejoin` | keyword | `miter` | Values: miter, round, bevel |
| `stroke-miterlimit` | `NonNegativeNumber` | `4` | |
| `stroke-dasharray` | `SVGStrokeDashArray` | `none` | |
| `stroke-dashoffset` | `SVGLength` | `0` | |

---

## Marker Properties (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `marker-start` | `url::UrlOrNone` | `none` | |
| `marker-mid` | `url::UrlOrNone` | `none` | |
| `marker-end` | `url::UrlOrNone` | `none` | |

---

## Paint Order (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `paint-order` | `SVGPaintOrder` | `normal` | Controls fill/stroke/markers order |

---

## Text Anchor (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `text-anchor` | keyword | `start` | Values: start, middle, end |

---

## Color Interpolation (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `color-interpolation` | keyword | `srgb` | Values: srgb, auto, linearrgb |
| `color-interpolation-filters` | keyword | `linearrgb` | Values: linearrgb, auto, srgb |

---

## Shape Rendering (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `shape-rendering` | keyword | `auto` | Values: auto, optimizeSpeed, crispEdges, geometricPrecision |

---

## Clip Rule (InheritedSVG)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `clip-rule` | `FillRule` | `nonzero` | Animation: discrete |

---

## Geometry Properties (SVG — not inherited)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `cx` | `LengthPercentage` | `0` | Circle/ellipse center X |
| `cy` | `LengthPercentage` | `0` | Circle/ellipse center Y |
| `r` | `NonNegativeLengthPercentage` | `0` | Circle radius |
| `rx` | `NonNegativeLengthPercentageOrAuto` | `auto` | Rect corner radius X |
| `ry` | `NonNegativeLengthPercentageOrAuto` | `auto` | Rect corner radius Y |
| `x` | `LengthPercentage` | `0` | Rect/other X position |
| `y` | `LengthPercentage` | `0` | Rect/other Y position |
| `d` | `DProperty` | `none` | Path data (`<path d="...">`) |

---

## Vector Effect (SVG — not inherited)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `vector-effect` | `VectorEffect` | `none` | Animation: discrete |

---

## Filter / Paint Server Properties (SVG — not inherited)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `flood-color` | `Color` | `black` | Used with `<feFlood>` filter |
| `flood-opacity` | `Opacity` | `1` | Used with `<feFlood>` filter |
| `lighting-color` | `Color` | `white` | Used with `<feDiffuseLighting>` / `<feSpecularLighting>` |
| `stop-color` | `Color` | `black` | Used with `<stop>` in gradients |
| `stop-opacity` | `Opacity` | `1` | Used with `<stop>` in gradients |

---

## Mask Properties (SVG — not inherited)

| Property | CSS Type | Initial Value | Notes |
|----------|----------|---------------|-------|
| `mask-image` | `Image` | `none` | Pre-existing, `servo_pref = "layout.unimplemented"` |
| `mask-position-x` | `HorizontalPosition` | `0%` | Vector (repeatable) |
| `mask-position-y` | `VerticalPosition` | `0%` | Vector (repeatable) |
| `mask-repeat` | `BackgroundRepeat` | `repeat` | Vector |
| `mask-size` | `BackgroundSize` | `auto` | Vector (repeatable) |
| `mask-type` | keyword | `luminance` | Values: luminance, alpha |
| `mask-mode` | keyword | `match-source` | Vector; Values: match-source, alpha, luminance |
| `mask-clip` | keyword | `border-box` | Vector; WebKit prefix support |
| `mask-origin` | keyword | `border-box` | Vector; WebKit prefix support |
| `mask-composite` | keyword | `add` | Vector; WebKit prefix support |

---

## Pre-existing Properties (already active before Phase 1)

| Property | Struct | Notes |
|----------|--------|-------|
| `clip-path` | `svg` | Was already enabled for Servo |
| `mask-image` | `svg` | Was already enabled (gated behind `layout.unimplemented` pref) |

---

## Scope Notes

### Phase 1 delivers:
- **Inline styles** (`style="fill: red; stroke-width: 4"`) — fully working
- **Inheritance** — SVG properties inherit correctly through the SVG DOM tree (e.g., `<rect>` inherits `fill` from parent `<svg>`)
- **Default values** — all properties have correct initial values

### Phase 2 (next):
- **Presentation attributes** — mapping HTML attributes like `fill="red"`, `cx="50"`, `stroke-width="2"` to their CSS property equivalents
- This is needed because SVG traditionally uses element attributes in addition to CSS styling

### Phase 3 (future):
- **Rendering** — the new SVG engine will consume these computed styles to actually paint SVG elements

---

## Technical Details

**Repository:** `D:\Projects\stylo` (branch: `svg-css-properties`)
**Stylo crate:** `style` v0.17.0
**37 TOML edits** in `style/properties/longhands.toml`:
- Removed `engine = "gecko"` from each property
- Added `servo_restyle_damage = "repaint"` to trigger proper repaint on property changes
- `size_of_test!(ComputedValues, 232)` updated when the first property activated `InheritedSVG`
