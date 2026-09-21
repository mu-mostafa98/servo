# SVG Attribute Support — `svg_engine`

Per-element reference of which attributes the `svg_engine` rendering pipeline
supports, and through **which mechanism** each one reaches the renderer.

This file has one section per SVG element. It is built up element by element;
`<svg>` is the first.

## How to read these tables

Every attribute is checked against the three ways it can be written, and a few
extra columns capture the special cases.

| Column | Meaning |
|---|---|
| **Inline style** | Works when written as `style="prop: value"` |
| **CSS** | Works when written in a `<style>` block / stylesheet / class rule |
| **Present. attr** | Works when written as `<element prop="value">` |
| **Inherited** | Whether the value cascades to child elements (`—` = not a CSS property) |

Cell values:

- ✅ — supported (Servo applies it)
- ❌ — valid form, but Servo does **not** apply it yet
- **not applicable** — that form does not exist for this attribute (e.g. it is
  not a CSS property, or has no SVG attribute form)

---

## `<svg>`

### Global / core attributes

These aren't rendering properties, so the inline-style/CSS columns don't apply —
they're the scaffolding the other tables depend on.

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |

### Viewport & geometry

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `width` | ✅ | ✅ | ✅ | ❌ | Attribute sets the **SVG viewport**; CSS `width` sets the layout box |
| `height` | ✅ | ✅ | ✅ | ❌ | Attribute sets the **SVG viewport**; CSS `height` sets the layout box |
| `x` | ❌ | ❌ | ✅ | ❌ | **Nested `<svg>` only** — ignored on the root |
| `y` | ❌ | ❌ | ✅ | ❌ | **Nested `<svg>` only** — ignored on the root |
| `viewBox` | not applicable | not applicable | ✅ | — | No CSS form exists |
| `preserveAspectRatio` | not applicable | not applicable | ✅ | — | No CSS form exists |
| `overflow` | ✅ | ❌ | ✅ | ❌ | Only the `visible` value is acted on; inline-style fallback, no stylesheet read |
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` and the SVG `transform` attribute are **merged** |

### Presentation — supported in all three

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `fill` | ✅ | ✅ | ✅ | ✅ | |
| `fill-opacity` | ✅ | ✅ | ✅ | ✅ | |
| `fill-rule` | ✅ | ✅ | ✅ | ✅ | |
| `stroke` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-width` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-linecap` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-linejoin` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-dasharray` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-dashoffset` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-miterlimit` | ✅ | ✅ | ✅ | ✅ | |
| `stroke-opacity` | ✅ | ✅ | ✅ | ✅ | |
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited — applies to the element as a whole |
| `display` | ✅ | ✅ | ✅ | ❌ | `display:none` suppresses the whole subtree |
| `visibility` | ✅ | ✅ | ✅ | ✅ | Children can override with `visibility:visible` |
| `vector-effect` | ✅ | ✅ | ✅ | ❌ | Only `non-scaling-stroke` is meaningful |
| `shape-rendering` | ✅ | ✅ | ✅ | ✅ | |
| `clip-path` | ✅ | ✅ | ✅ | ❌ | |
| `mask` | ✅ | ✅ | ✅ | ❌ | |
| `font-family` | ✅ | ✅ | ✅ | ✅ | |
| `font-size` | ✅ | ✅ | ✅ | ✅ | |
| `font-style` | ✅ | ✅ | ✅ | ✅ | |
| `font-weight` | ✅ | ✅ | ✅ | ✅ | |

### Attribute-only — read from markup, not CSS

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `color-interpolation` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `color-rendering` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `paint-order` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `filter` | ❌ | ❌ | ✅ | ❌ | `filter:url(#id)` cannot round-trip through Stylo, so the engine reads the attribute |

### CSS-only / not supported

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `color` | ✅ | ✅ | ❌ | ✅ | **CSS-only** in Servo — the `color` attribute is not synthesized; only meaningful as inherited `currentColor` for children |
| `font-stretch` | ✅ | ✅ | ❌ | ✅ | CSS-only (not synthesized as an attribute) |
| `font-size-adjust` | ❌ | ❌ | ❌ | ✅ | Not applied by the engine |
| `font-variant` | ❌ | ❌ | ❌ | ✅ | Variant features hardcoded to normal |
| `letter-spacing` | ❌ | ❌ | ❌ | ✅ | Hardcoded `None` |
| `word-spacing` | ❌ | ❌ | ❌ | ✅ | Hardcoded `None` |
| `text-rendering` | ❌ | ❌ | ❌ | ✅ | Hardcoded `None` |
| `text-decoration` | ❌ | ❌ | ❌ | ❌ | Not applied by the engine |
| `image-rendering` | ❌ | ❌ | ❌ | ✅ | Hardcoded `auto` |
| `mix-blend-mode` | ❌ | ❌ | not applicable | ❌ | CSS-only — no SVG attribute form exists |
| `isolation` | ❌ | ❌ | not applicable | ❌ | CSS-only — no SVG attribute form exists |
| `transform-origin` | ❌ | ❌ | not applicable | ❌ | CSS-only — no SVG attribute form exists |

### Not applicable to `<svg>`

Attributes that belong to other elements or to interaction/a11y, not to the
`<svg>` element itself:

- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline` (read on `<text>`/`<tspan>` only)
- **Gradients / shapes** — `stop-color`, `stop-opacity` (`<stop>`), `clip-rule` (shapes inside a clip)
- **Interaction / a11y** — `on*` event handlers, `pointer-events`, `cursor`, `aria-*`, `role`, `lang`, `autofocus`, `tabindex`
