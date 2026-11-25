# SVG security limits — quick reference

Companion to [`security.md`](security.md) and [`security-implementation.md`](security-implementation.md).
Every numeric guard, its value, and a minimal trigger example.

| Constant | Value | Layer | What it caps | Example |
| --- | --- | --- | --- | --- |
| `MAX_PARSE_ELEMENT_COUNT` | `1_000_000` | 1 · Parse | Elements created during tokenization; inserts past the cap are dropped | `<svg><rect/><rect/>… (1,000,001 elements)` |
| `MAX_PARSE_DEPTH` | `512` | 1 · Parse | Element nesting depth during tokenization — the "group depth" | `<svg><g><g>… 513 nested <g> …</g></g></svg>` |
| `MAX_USE_DEPTH` | `64` | 4 · Build | `<use>` reference-nesting depth (breaks mutual recursion) | `<g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g>` |
| `MAX_TOTAL_NODES` | `1_000_000` | 4 · Build | Total nodes the SVG builder may expand (bounds fan-out) | one `<g>` full of `<use>`s expanding past 1M nodes |
| `MAX_VIEWPORT_DIMENSION` | `1.0e7` | 4 · Build | `viewBox` / viewport width or height (user units) | `<svg viewBox="0 0 99999999 100">` |
| `MAX_PATH_SEGMENTS` | `1_000_000` | 4 · Build | `d` path segments; larger paths render nothing | `<path d="M0 0 L1 1 … 1,000,001 segments">` |
| `MAX_POINTS` | `1_000_000` | 4 · Build | Coordinate pairs in a `points` list; extras dropped | `<polygon points="0,0 1,1 … 1,000,001 pairs">` |
| `MAX_STROKE_WIDTH` | `1.0e7` | 4 · Build | Stroke width (user units) | `<rect stroke-width="99999999">` |
| `MAX_FONT_SIZE` | `10000.0` | 4 · Build | Resolved `font-size` (CSS px); matches Chrome's ~10000 clamp. Glyphs above WebRender's 320 px `FONT_SIZE_LIMIT` are rendered via glyph outlines (vello CPU), never rasterized-then-upscaled (which would band the antialiasing) | `<text font-size="1e6">A</text>` |
| `MAX_MITER_LIMIT` | `1000.0` | 4 · Build | `stroke-miterlimit` | `<path stroke-miterlimit="999999">` |
| `MAX_DASH_COUNT` | `1024` | 4 · Build | `stroke-dasharray` entries; extras dropped | `<path stroke-dasharray="1 2 3 … 1025 entries">` |
| `MAX_DASH_LENGTH` | `1.0e7` | 4 · Build | Magnitude of a dash length or offset | `<path stroke-dashoffset="99999999">` |
| `MAX_PAINT_SERVER_DEPTH` | `32` | 5 · Render | Recursive paint-server (gradient/pattern) reference depth | `<pattern id="p" …><rect fill="url(#p)"/></pattern>` |
| `MAX_MASK_DEPTH` | `16` | 5 · Render | Nested `<mask>` resolution depth | `<mask id="m"><rect mask="url(#m)"/></mask>` |
