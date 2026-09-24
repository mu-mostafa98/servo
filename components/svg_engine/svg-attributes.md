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
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

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

### Valid on `<svg>` but not consumed by the renderer

Per spec these attributes **are** valid on `<svg>` and have a defined effect —
that effect just happens in a component other than `svg_engine`:

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized (`synthesize_presentational_hints` has
> no entry for either), so they only work via CSS — hit-testing reads
> `pointer-events` from the computed value, not the attribute. The rest of this
> group is attribute-native (the attribute is their only form, and it is read).

### Not applicable to `<svg>` (per spec)

Per spec these properties apply to **other elements**, so writing them on
`<svg>` has no effect:

- Text: `text-anchor`, `dominant-baseline`, `alignment-baseline` — apply to `<text>`/`<tspan>`
- Gradients / shapes: `stop-color`, `stop-opacity` — apply to `<stop>`; `clip-rule` — applies to shapes inside a `clipPath`

---

## `<g>`

The group element. It carries **no viewport or geometry of its own** — it groups
children and applies a shared style (`transform`, `opacity`, `fill`, `stroke`,
`clip-path`, …) to them. Presentation-attribute support is identical to `<svg>`;
only the viewport attributes (`width`/`height`/`viewBox`/`preserveAspectRatio`/
`x`/`y`/`overflow`) don't apply.

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Transform & positioning

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` + SVG `transform` attribute merged; the primary positioning mechanism for `<g>` |

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
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited — applies to the group as a whole |
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

### Valid on `<g>` but not consumed by the renderer

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized, so they only work via CSS. The rest of
> this group is attribute-native (the attribute is their only form, and it is read).

### Not applicable to `<g>` (per spec)

- **Viewport / geometry** — `width`, `height`, `viewBox`, `preserveAspectRatio`, `x`, `y`, `overflow` (apply to `<svg>`/shapes/`<image>`, not `<g>`)
- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline` (apply to `<text>`/`<tspan>`)
- **Gradients / shapes** — `stop-color`, `stop-opacity` (apply to `<stop>`); `clip-rule` (applies to shapes inside a `clipPath`)

---

## `<defs>`

The definitions container. It holds reusable content — gradients, patterns,
clip-paths, masks, filters, markers, symbols, shapes — that is **never rendered
directly**. Its children only appear when referenced:

- via `url(#id)` — `fill`/`stroke`/`filter`/`clip-path`/`mask`/`marker-*` point at the definition, or
- via `<use href="#id">` — a shape/group/symbol is cloned into place.

**Consequence for the attributes below:** they are *parsed* and *synthesized*
exactly as on `<g>` (Servo runs the same `build_style` and
`synthesize_presentational_hints` for all containers), but they have **no visible
effect on `<defs>` itself** — `<defs>` paints nothing, and referenced content is
drawn in the *referencing* element's context (it does not inherit from `<defs>`).

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Transform & positioning

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` + SVG `transform` attribute merged; no visible effect on `<defs>` (it renders nothing) |

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
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited — applies to the group as a whole |
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

### Valid on `<defs>` but not consumed by the renderer

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized, so they only work via CSS. The rest of
> this group is attribute-native (the attribute is their only form, and it is read).

### Not applicable to `<defs>` (per spec)

- **Viewport / geometry** — `width`, `height`, `viewBox`, `preserveAspectRatio`, `x`, `y`, `overflow`
- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline`
- **Gradients / shapes** — `stop-color`, `stop-opacity`, `clip-rule`

## `<symbol>`

A reusable container like `<defs>`, but one that can **establish a viewport**.
A `<symbol>` is never rendered directly — it only appears when referenced via
`<use href="#id">`. At that point its `viewBox`/`preserveAspectRatio` (if
present) map its internal coordinates onto the viewport declared by the `<use>`,
exactly like a nested `<svg>`.

Presentation-attribute support is otherwise identical to `<g>`/`<defs>`: Servo
runs the same `build_style` and `synthesize_presentational_hints` for all
containers, so every presentation attribute is *parsed* — but it has **no
visible effect on `<symbol>` itself**, which renders nothing. Placement and
viewport are controlled by the referencing `<use>`.

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Viewport (via `<use>`)

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `viewBox` | not applicable | not applicable | ✅ | — | No CSS form; establishes the symbol's viewport when referenced via `<use>` |
| `preserveAspectRatio` | not applicable | not applicable | ✅ | — | No CSS form; used only when referenced via `<use>` |
| `width` | ❌ | ❌ | ✅ | ❌ | Legacy SVG 1.1 fallback — read from the attribute only when `<use>` omits `width`; removed from `<symbol>` in SVG 2 |
| `height` | ❌ | ❌ | ✅ | ❌ | Legacy SVG 1.1 fallback — read from the attribute only when `<use>` omits `height`; removed from `<symbol>` in SVG 2 |

### Transform & positioning

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` + SVG `transform` attribute merged; no visible effect on `<symbol>` (renders nothing; `<use>` controls placement) |

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
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited — applies to the group as a whole |
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

### Valid on `<symbol>` but not consumed by the renderer

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized, so they only work via CSS. The rest of
> this group is attribute-native (the attribute is their only form, and it is read).

### Not applicable to `<symbol>` (per spec)

- **Positioning** — `x`, `y` (carried by the referencing `<use>`, not `<symbol>`; removed from `<symbol>` in SVG 2)
- **Overflow** — `overflow` (not read from `<symbol>` by Servo)
- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline`
- **Gradients / shapes** — `stop-color`, `stop-opacity`, `clip-rule`

## `<use>`

Instantiates a copy of another element. `<use href="#id">` clones the referenced
element's render node into place and applies an optional `x`/`y` translation. It
is itself a container, and its presentation attributes + CSS are **inherited by
the cloned content** (where the clone doesn't set its own) — so `fill="red"` on
`<use>` recolors the shapes it references. The engine passes the `<use>` node's
computed style down as the inherited style when building the target
([builder.rs:504-505](components/layout/svg/builder.rs#L504-L505)).

Two viewport facts to keep in mind:

- `x`/`y` are read from the **attribute only** (not CSS) and become a `translate`
  on the clone.
- `width`/`height` are read from the **attribute only** and matter *only* when
  the target is a `<symbol>` with a `viewBox` — they then define the `<use>`
  viewport the symbol maps into; otherwise they are ignored.

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Reference

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `href` | not applicable | not applicable | ✅ | ❌ | References the element to clone by `#id` |
| `xlink:href` | not applicable | not applicable | ✅ | ❌ | Legacy SVG 1.1 spelling — fallback when `href` is absent; deprecated in SVG 2 |

### Positioning & viewport

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `x` | ❌ | ❌ | ✅ | ❌ | Read from the attribute only — applied as a `translate` on the cloned content |
| `y` | ❌ | ❌ | ✅ | ❌ | Read from the attribute only — applied as a `translate` on the cloned content |
| `width` | ❌ | ❌ | ✅ | ❌ | Read from the attribute only, and only when the target is a `<symbol>` with a `viewBox` (defines the `<use>` viewport); ignored otherwise |
| `height` | ❌ | ❌ | ✅ | ❌ | Read from the attribute only, and only when the target is a `<symbol>` with a `viewBox`; ignored otherwise |
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` + SVG `transform` attribute merged; applies to the cloned content as a whole (composes with the `x`/`y` translate) |

### Presentation — supported in all three

Inherited by the cloned content (fills/strokes here act as defaults that the
clone can override).

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
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited — applies to the whole clone as a group |
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

### Valid on `<use>` but not consumed by the renderer

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized, so they only work via CSS. The rest of
> this group is attribute-native (the attribute is their only form, and it is read).

### Not applicable to `<use>` (per spec)

- **Viewport** — `viewBox`, `preserveAspectRatio` (carried by a referenced `<symbol>`, not `<use>`), `overflow`
- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline`
- **Gradients / shapes** — `stop-color`, `stop-opacity`, `clip-rule`

## The 7 geometry shapes

`<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>`.

Everything below the per-shape geometry tables is identical across all seven
(they share one `build_style` / `synthesize_presentational_hints` path and one
`build_shape` entry point), so it is listed once here instead of seven times.
Each shape then gets a single table for the geometry attributes that are
*only* meaningful on that shape.

### How geometry is read

Servo reads shape geometry in two ways ([geometry.rs](components/layout/svg/geometry.rs#L36-L53)):

- **From the CSS cascade** — `x`, `y`, `cx`, `cy`, `r`, `rx`, `ry`. These are
  SVG geometry *CSS properties*: the presentation attribute is synthesized into
  CSS, so the attribute, an inline style, and a stylesheet all work.
- **From the DOM attribute only** — `width`, `height`, `x1`, `y1`, `x2`, `y2`,
  `points`, `d`. These have no CSS form in Servo (for `width`/`height` the CSS
  form exists but is *not* used for shape geometry), so only the attribute works.

That is why the per-shape tables below mix cascade rows (✅ in all three) with
attribute-only rows (✅ in the attribute column only).

### Shared across all shapes

#### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

#### Transform & positioning

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `transform` | ✅ | ✅ | ✅ | ❌ | CSS `transform` + SVG `transform` attribute merged |

#### Presentation — supported in all three

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
| `opacity` | ✅ | ✅ | ✅ | ❌ | Not inherited |
| `display` | ✅ | ✅ | ✅ | ❌ | `display:none` suppresses the shape |
| `visibility` | ✅ | ✅ | ✅ | ✅ | |
| `vector-effect` | ✅ | ✅ | ✅ | ❌ | Only `non-scaling-stroke` is meaningful |
| `shape-rendering` | ✅ | ✅ | ✅ | ✅ | |
| `clip-path` | ✅ | ✅ | ✅ | ❌ | |
| `mask` | ✅ | ✅ | ✅ | ❌ | |

#### Attribute-only — read from markup, not CSS

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `color-interpolation` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `color-rendering` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `paint-order` | ❌ | ❌ | ✅ | ✅ | Read from the attribute only |
| `filter` | ❌ | ❌ | ✅ | ❌ | `filter:url(#id)` cannot round-trip through Stylo, so the engine reads the attribute |

#### CSS-only / not supported

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `color` | ✅ | ✅ | ❌ | ✅ | **CSS-only** in Servo — the `color` attribute is not synthesized; meaningful only as `currentColor` for `fill`/`stroke` |
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

#### Valid on shapes but not consumed by the renderer

| Attribute | Effect (per spec) | Handled by |
|---|---|---|
| `on*` event handlers | fires the handler | script engine |
| `pointer-events` | hit-testing (inherited → whole subtree) | hit-testing / layout |
| `cursor` | mouse cursor (inherited → whole subtree) | UI |
| `aria-*`, `role` | accessibility tree | a11y |
| `lang` | language determination (`:lang()`, font) | semantics |
| `autofocus`, `tabindex` | focus management | focus |

> **Attribute-form caveat:** `pointer-events` and `cursor` are the only two here
> that are *also* CSS properties. Per spec their attribute form is valid, but in
> Servo the attribute is not synthesized, so they only work via CSS. The rest of
> this group is attribute-native (the attribute is their only form, and it is read).

#### Not applicable to shapes (per spec)

- **Viewport** — `viewBox`, `preserveAspectRatio`, `overflow`
- **Text** — `text-anchor`, `dominant-baseline`, `alignment-baseline`
- **Gradients / clipping** — `stop-color`, `stop-opacity`, `clip-rule`

---

### `<rect>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `x` | ✅ | ✅ | ✅ | ❌ | Cascade geometry (attribute synthesized into CSS) |
| `y` | ✅ | ✅ | ✅ | ❌ | Cascade geometry |
| `width` | ❌ | ❌ | ✅ | ❌ | DOM attribute only — CSS `width` does not drive shape geometry |
| `height` | ❌ | ❌ | ✅ | ❌ | DOM attribute only — CSS `height` does not drive shape geometry |
| `rx` | ✅ | ✅ | ✅ | ❌ | Cascade geometry; `auto`/absent → no rounding |
| `ry` | ✅ | ✅ | ✅ | ❌ | Cascade geometry; `auto`/absent → no rounding |

### `<circle>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `cx` | ✅ | ✅ | ✅ | ❌ | Cascade geometry |
| `cy` | ✅ | ✅ | ✅ | ❌ | Cascade geometry |
| `r` | ✅ | ✅ | ✅ | ❌ | Cascade geometry; `r ≤ 0` → circle not rendered |

### `<ellipse>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `cx` | ✅ | ✅ | ✅ | ❌ | Cascade geometry |
| `cy` | ✅ | ✅ | ✅ | ❌ | Cascade geometry |
| `rx` | ✅ | ✅ | ✅ | ❌ | Cascade geometry; `rx ≤ 0` → not rendered |
| `ry` | ✅ | ✅ | ✅ | ❌ | Cascade geometry; `ry ≤ 0` → not rendered |

### `<line>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `x1` | not applicable | not applicable | ✅ | — | DOM attribute only |
| `y1` | not applicable | not applicable | ✅ | — | DOM attribute only |
| `x2` | not applicable | not applicable | ✅ | — | DOM attribute only |
| `y2` | not applicable | not applicable | ✅ | — | DOM attribute only |

### `<polyline>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `points` | not applicable | not applicable | ✅ | — | DOM attribute only; space/comma-separated `x,y` pairs |

### `<polygon>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `points` | not applicable | not applicable | ✅ | — | DOM attribute only; automatically closed |

### `<path>`

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `d` | not applicable | not applicable | ✅ | — | DOM attribute only; parsed by kurbo's SVG path parser |

## `<linearGradient>` & `<radialGradient>`

Paint servers defined in `<defs>` and referenced from `fill`/`stroke` via
`fill="url(#id)"`. They are **never rendered directly** — they exist only as
definitions. Both are parsed by `parse_gradient_element`
([gradient.rs](components/svg_engine/src/style/gradient.rs#L148-L258)), which
reads **raw DOM attributes only**: none of the geometry/units/spread attributes
below are CSS properties, and Servo does not run the gradient element through
`build_style` / `synthesize_presentational_hints`. So `style="…"` and stylesheet
rules have no effect on a gradient — every attribute must be written on the
element itself.

The two types share the same global + common attributes and the same `<stop>`
children; they differ only in the geometry attributes that define the gradient
spine.

### Shared across both gradients

#### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | **Required** — the gradient is referenced by `fill="url(#id)"` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors (no effect on gradient rendering) |
| `style` | not applicable | not applicable | ✅ | ❌ | Valid, but no effect — gradient attributes are not CSS |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

#### Common gradient attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `gradientUnits` | not applicable | not applicable | ✅ | — | `objectBoundingBox` (default) / `userSpaceOnUse` |
| `spreadMethod` | not applicable | not applicable | ✅ | — | `pad` (default) / `reflect` / `repeat` |
| `gradientTransform` | not applicable | not applicable | ✅ | — | Transform applied to gradient coordinates (distinct from CSS `transform`) |

#### `<stop>` children

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `offset` | not applicable | not applicable | ✅ | — | Stop position `0.0`–`1.0` (or `%`); a stop without `offset` is dropped |
| `stop-color` | ❌ | ❌ | ✅ | ❌ | CSS property per spec, but Servo reads the attribute only; default black |
| `stop-opacity` | ❌ | ❌ | ✅ | ❌ | CSS property per spec, but Servo reads the attribute only; multiplies stop alpha |

> **Gradient `href` inheritance — stop reuse only:** per spec a gradient can
> chain to another via `href`/`xlink:href` and inherit its missing attributes.
> Servo reads `href`/`xlink:href` and, when a gradient has no `<stop>` children
> of its own, inherits the referenced gradient's stops (transitively, with cycle
> detection). Geometry/units/transform/spread attribute inheritance is **not**
> implemented — each gradient must still declare those attributes directly.

### `<linearGradient>` — geometry

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `x1` | not applicable | not applicable | ✅ | — | Start X; default `0` |
| `y1` | not applicable | not applicable | ✅ | — | Start Y; default `0` |
| `x2` | not applicable | not applicable | ✅ | — | End X; default `100%` (objectBoundingBox) / `100` (userSpaceOnUse) |
| `y2` | not applicable | not applicable | ✅ | — | End Y; default `0` |

### `<radialGradient>` — geometry

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `cx` | not applicable | not applicable | ✅ | — | Center X; default `50%` |
| `cy` | not applicable | not applicable | ✅ | — | Center Y; default `50%` |
| `r` | not applicable | not applicable | ✅ | — | Radius; default `50%` |
| `fx` | not applicable | not applicable | ✅ | — | Focal X; default = `cx` |
| `fy` | not applicable | not applicable | ✅ | — | Focal Y; default = `cy` |
| `fr` | not applicable | not applicable | ✅ | — | Focal radius; default `0` |

## `<stop>`

One color stop inside a `<linearGradient>` or `<radialGradient>`. It is **never
rendered directly** — it exists only to be read by the gradient collector
(`GradientParser` in [defines.rs](components/layout/svg/defines.rs#L91-L147)),
which pulls **raw DOM attributes only**. `<stop>` is not styled via CSS and does
not run through `synthesize_presentational_hints`, so `style="…"` and stylesheet
rules have no effect on it.

### Stop attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `offset` | not applicable | not applicable | ✅ | — | Stop position `0.0`–`1.0` (or `%`); a stop without `offset` is dropped |
| `stop-color` | ❌ | ❌ | ✅ | ❌ | CSS property per spec, but Servo reads the attribute only; default black |
| `stop-opacity` | ❌ | ❌ | ✅ | ❌ | CSS property per spec, but Servo reads the attribute only; multiplies stop alpha |

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Valid, but not used — stops are positional children, not `url(#id)` targets |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors (no effect — stops aren't styled) |
| `style` | not applicable | not applicable | ✅ | ❌ | Valid, but no effect — stop attributes are not CSS |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; ignored by Servo |

### Behavior notes

- Stops are **sorted by `offset`** after parsing
  ([gradient.rs:194-198](components/svg_engine/src/style/gradient.rs#L194-L198)).
- A `<stop>` with no usable `offset` is **dropped**; if a gradient ends up with
  no stops, Servo synthesizes two black stops at `0.0` and `1.0`.
- Everything else is **not applicable to `<stop>`** — `fill`, `stroke`,
  `transform`, viewport attributes, and text attributes belong to rendered
  shapes/text, not to a paint-server stop.

## `<pattern>`

A tiling paint server defined in `<defs>` and referenced from `fill` via
`fill="url(#id)"`. It repeats its child shapes across a region. **Never rendered
directly** — parsed by `PatternParser`
([defines.rs](components/layout/svg/defines.rs#L201-L289)), which reads **raw DOM
attributes only**: the pattern's own geometry/units are not read from the CSS
cascade (unlike a shape's `x`/`y`/`cx`/…), so `style="…"` and stylesheet rules
have no effect on the pattern itself.

### Geometry & units

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `width` | ❌ | ❌ | ✅ | ❌ | Tile width; **required** — `width ≤ 0` discards the pattern; read attribute-only |
| `height` | ❌ | ❌ | ✅ | ❌ | Tile height; **required** — `height ≤ 0` discards the pattern; read attribute-only |
| `x` | ❌ | ❌ | ✅ | ❌ | Tile offset X; default `0`; read attribute-only |
| `y` | ❌ | ❌ | ✅ | ❌ | Tile offset Y; default `0`; read attribute-only |
| `patternUnits` | not applicable | not applicable | ✅ | — | `objectBoundingBox` / `userSpaceOnUse` (default) |
| `patternContentUnits` | not applicable | not applicable | ✅ | — | `objectBoundingBox` / `userSpaceOnUse` (default) |
| `patternTransform` | not applicable | not applicable | ✅ | — | Transform applied to the tile (distinct from CSS `transform`) |
| `viewBox` | not applicable | not applicable | ✅ | — | Optional tile viewport |
| `preserveAspectRatio` | not applicable | not applicable | ✅ | — | Optional; meaningful only with `viewBox` |

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | **Required** — referenced by `fill="url(#id)"` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors (no effect on pattern rendering) |
| `style` | not applicable | not applicable | ✅ | ❌ | Valid, but no effect — pattern attributes are not CSS |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; ignored by Servo |

### Behavior notes

- **Children:** only shapes are collected into the tile (`build_shape`); non-shape
  children are ignored. If the pattern ends up with no shapes, it is discarded
  ([defines.rs:253-287](components/layout/svg/defines.rs#L253-L287)).
- **Pattern inheritance not supported:** per spec a pattern can chain to another
  via `href`/`xlink:href`; Servo's `PatternParser` does not read `href`, so each
  pattern must declare everything directly.
- Everything else (`fill`, `stroke`, text attributes, …) is not applicable to
  `<pattern>` — it is a paint-server definition, not a rendered shape.

## `<marker>`

A paint server placed at line vertices — referenced from a shape's
`marker-start`/`marker-mid`/`marker-end` attributes (each `url(#id)`). **Never
rendered directly** — parsed by `MarkerParser`
([defines.rs](components/layout/svg/defines.rs#L659-L729)), which reads **raw DOM
attributes only**. `style="…"` and stylesheet rules have no effect on the marker
itself.

### Marker attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `refX` | not applicable | not applicable | ✅ | — | Reference point X (aligns to the vertex); default `0` |
| `refY` | not applicable | not applicable | ✅ | — | Reference point Y; default `0` |
| `markerWidth` | not applicable | not applicable | ✅ | — | Marker viewport width; default `3` |
| `markerHeight` | not applicable | not applicable | ✅ | — | Marker viewport height; default `3` |
| `markerUnits` | not applicable | not applicable | ✅ | — | `strokeWidth` (default) / `userSpaceOnUse` |
| `orient` | not applicable | not applicable | ✅ | — | `auto` (default) / `auto-start-reverse` / angle (`45` or `45deg`) |
| `viewBox` | not applicable | not applicable | ✅ | — | Optional marker viewport |

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | **Required** — referenced by `marker-start`/`marker-mid`/`marker-end` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors (no effect on marker rendering) |
| `style` | not applicable | not applicable | ✅ | ❌ | Valid, but no effect — marker attributes are not CSS |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; ignored by Servo |

### Not read (valid per spec)

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `preserveAspectRatio` | not applicable | not applicable | ❌ | — | Valid per spec; Servo does not read it |
| `overflow` | not applicable | not applicable | ❌ | — | Valid per spec (default `hidden`); Servo does not read it |

### Behavior notes

- **Children:** only shapes are collected into the marker (`build_shape`);
  non-shape children are ignored.
- **Marker inheritance not supported:** per spec a marker can chain to another
  via `href`/`xlink:href`; Servo's `MarkerParser` does not read `href`.

## `<image>`

A raster image drawn into the SVG. `<image>` is a rendered leaf (not a shape,
not a container): it builds an `SvgImage` from its attributes
([builder.rs:699-768](components/layout/svg/builder.rs#L699-L768)) and draws the
decoded bitmap. Its geometry and source are read from **raw DOM attributes only**
— unlike the shapes, `x`/`y` are *not* read from the CSS cascade here.

### Image source & geometry

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `href` | not applicable | not applicable | ✅ | — | Image URL, resolved against the document base; wins over `xlink:href` |
| `xlink:href` | not applicable | not applicable | ✅ | — | Legacy SVG 1.1 spelling — fallback when `href` is absent |
| `x` | ❌ | ❌ | ✅ | ❌ | Attribute-only; default `0` |
| `y` | ❌ | ❌ | ✅ | ❌ | Attribute-only; default `0` |
| `width` | ❌ | ❌ | ✅ | ❌ | Attribute-only; **required** — `width ≤ 0` discards the image |
| `height` | ❌ | ❌ | ✅ | ❌ | Attribute-only; **required** — `height ≤ 0` discards the image |
| `preserveAspectRatio` | not applicable | not applicable | ✅ | — | Default `xMidYMid meet` |

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Presentation & positioning

`<image>` is styled like the shapes, so the shared shape tables apply — with two
exceptions:

- **`fill`, `stroke`, `fill-rule`, `stroke-*` are not applicable** to `<image>`
  (there is no fillable geometry to paint).
- **`x`/`y`/`width`/`height` are attribute-only here** — read from the raw
  attribute, not the CSS cascade (see the table above).

Everything else — `transform`, `opacity`, `display`, `visibility`, `clip-path`,
`mask`, `filter`, `color-interpolation`, `color-rendering`, `paint-order` — behaves
exactly as in the shared shape tables.

### Behavior notes

- **Raster only:** a vector image (e.g. an SVG referenced by `href`) is not
  rasterized — it yields `image_key = None` and the renderer draws a placeholder
  ([builder.rs:744](components/layout/svg/builder.rs#L744)).
- **Pending load:** a not-yet-loaded image also renders a placeholder; a reflow
  re-runs the build once it loads.
- **Required geometry:** `width ≤ 0` or `height ≤ 0` discards the image entirely.

## `<text>` & `<tspan>`

Rendered text. `<text>` is the text container; `<tspan>` is a nested run that
can override its own style, font, and position. Both go through `build_style` /
`synthesize_presentational_hints` like the shapes, so `fill`, `stroke`, and the
`font-*` properties are read from the CSS cascade — but the *positioning*
attributes below are read from **raw DOM attributes only** (via
`build_text`/`build_text_run` in
[geometry.rs](components/layout/svg/geometry.rs#L62-L127)).

### Text positioning (shared)

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `x` | ❌ | ❌ | ✅ | ❌ | Line origin X; default `0`; `<tspan>` inherits the parent `<text>`'s `x` if unset |
| `y` | ❌ | ❌ | ✅ | ❌ | Line origin Y; default `0`; `<tspan>` inherits the parent `<text>`'s `y` if unset |
| `dx` | not applicable | not applicable | ✅ | — | Per-character X offsets (space/comma-separated list) |
| `dy` | not applicable | not applicable | ✅ | — | Per-character Y offsets; accumulates across `<tspan>`s |
| `rotate` | not applicable | not applicable | ✅ | — | Per-character rotation angle (degrees) |
| `text-anchor` | ❌ | ❌ | ✅ | ❌ | `start` (default) / `middle` / `end`; CSS property per spec, but read attribute-only |
| `dominant-baseline` | ❌ | ❌ | ✅ | ❌ | `auto` (default) / `hanging` / `middle` / `central`; CSS property per spec, but read attribute-only |
| `direction` | ❌ | ❌ | ✅ | ❌ | `rtl` reverses per-character offsets; read attribute-only |

### Not read (valid per spec)

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `alignment-baseline` | ❌ | ❌ | ❌ | ❌ | Valid per spec (and a CSS property), but not read — Servo only honors `dominant-baseline` |
| `textLength` | not applicable | not applicable | ❌ | — | Valid per spec; not read — no text stretching |
| `lengthAdjust` | not applicable | not applicable | ❌ | — | Valid per spec (pairs with `textLength`); not read |

### Presentation

`<text>`/`<tspan>` use the same `build_style` + `synthesize_presentational_hints`
as the shapes, so the shared shape tables apply — with these differences:

- **Font properties apply** — `font-family`/`font-size`/`font-style`/`font-weight`
  set the rendered font (on shapes they are no-ops, so they are listed here only).
- **Only the *color* of `fill`/`stroke` applies to glyphs** — `fill-opacity`,
  `fill-rule`, and all `stroke-*` sub-properties are parsed into the style but
  ignored for text (they only affect shapes). Stroke on text is a filled-glyph
  recolor, not a true outline
  ([text.rs](components/svg_engine/src/renderer/text.rs#L49-L68)).
- **`text-anchor`/`dominant-baseline` are attribute-only** (see above) — their
  CSS form is not applied.

#### Font (applies to text)

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `font-family` | ✅ | ✅ | ✅ | ✅ | Sets the shaped font |
| `font-size` | ✅ | ✅ | ✅ | ✅ | Sets the shaped font size |
| `font-style` | ✅ | ✅ | ✅ | ✅ | Sets the shaped font style |
| `font-weight` | ✅ | ✅ | ✅ | ✅ | Sets the shaped font weight |

Everything else — `transform`, `opacity`, `display`, `visibility`, `clip-path`,
`mask`, `filter`, `color`, `font-stretch`, … — behaves exactly as in the shared
shape tables.

### Global / core attributes

| Attribute | Inline style | CSS | Present. attr | Inherited | Notes |
|---|---|---|---|---|---|
| `id` | not applicable | not applicable | ✅ | ❌ | Identity hook — used by `url(#id)`, CSS `#id`, `<use href="#id">` |
| `class` | not applicable | not applicable | ✅ | ❌ | Hook for CSS selectors |
| `style` | not applicable | not applicable | ✅ | ❌ | This **is** the inline-style mechanism — all CSS properties go through it |
| `xmlns` | not applicable | not applicable | ✅ | ❌ | XML namespace declaration (required for the element to be parsed as SVG) |
| `version` | not applicable | not applicable | ❌ | ❌ | Legacy SVG 1.1 version marker — removed in SVG 2; no effect; ignored by Servo |

### Behavior notes

- Each `<tspan>` becomes its **own run** with its own style and font; a `<text>`
  with mixed bare-text + `<tspan>` children is assembled as a `Container::Text`
  of runs flowing left-to-right on one line
  ([builder.rs:146-252](components/layout/svg/builder.rs#L146-L252)).
- Bare text nodes inherit the parent `<text>`'s `x`/`y`; a `<tspan>` overrides
  them only if it sets `x`/`y` explicitly.
- **Whitespace is trimmed unconditionally** — `xml:space="preserve"` is not
  honored; pure-whitespace between `<tspan>`s is dropped.
