# SVG Properties Status Summary

## CSS Inline Style (`svg_by_struct_test.html`) — 96 total

| Section | Property | Status |
|---------|----------|--------|
| **inherited_svg** | fill | enabled |
| | fill-opacity | enabled |
| | fill-rule | enabled |
| | clip-rule | enabled |
| | stroke | enabled |
| | stroke-width | enabled |
| | stroke-dasharray | enabled |
| | stroke-dashoffset | enabled |
| | stroke-linecap | enabled |
| | stroke-linejoin | enabled |
| | stroke-miterlimit | enabled |
| | stroke-opacity | enabled |
| | marker-start | enabled |
| | marker-mid | enabled |
| | marker-end | enabled |
| | paint-order | enabled |
| | shape-rendering | enabled |
| | color-interpolation | enabled |
| | color-interpolation-filters | enabled |
| | text-anchor | enabled |
| **svg** | cx | enabled |
| | cy | enabled |
| | r | enabled |
| | rx | enabled |
| | ry | enabled |
| | x | enabled |
| | y | enabled |
| | d | enabled |
| | vector-effect | enabled |
| | clip-path | enabled |
| | flood-color | enabled |
| | flood-opacity | enabled |
| | lighting-color | enabled |
| | stop-color | enabled |
| | stop-opacity | enabled |
| | mask-type | enabled |
| **position** | width | enabled |
| | height | enabled |
| | min-width | already existed |
| | max-width | already existed |
| | min-height | already existed |
| | max-height | already existed |
| | object-fit | already existed |
| | object-position | already existed |
| | aspect-ratio | already existed |
| | z-index | already existed |
| | box-sizing | already existed |
| **effects** | opacity | already existed |
| | filter | already existed |
| | mix-blend-mode | already existed |
| | box-shadow | already existed |
| **inherited_box** | direction | already existed |
| | writing-mode | already existed |
| | visibility | already existed |
| | image-rendering | already existed |
| | dominant-baseline | already existed |
| **box** | transform | already existed |
| | rotate | already existed |
| | scale | already existed |
| | translate | already existed |
| | display | already existed |
| | overflow | already existed |
| | isolation | already existed |
| | alignment-baseline | already existed |
| | baseline-shift | already existed |
| **inherited_text** | color | already existed |
| | letter-spacing | already existed |
| | word-spacing | already existed |
| | text-rendering | already existed |
| | text-align | already existed |
| | text-transform | already existed |
| | text-shadow | already existed |
| | word-break | already existed |
| | overflow-wrap | already existed |
| | white-space | already existed |
| | text-indent | already existed |
| | line-break | already existed |
| | tab-size | already existed |
| **text** | unicode-bidi | already existed |
| | text-decoration-line | already existed |
| | text-decoration-color | already existed |
| | text-decoration-style | already existed |
| | text-overflow | already existed (gated behind pref) |
| **font** | font-family | already existed |
| | font-style | already existed |
| | font-weight | already existed |
| | font-size | already existed |
| | font-stretch | already existed |
| | line-height | already existed |
| | font-kerning | already existed |
| | font-variant-caps | already existed |
| | font-optical-sizing | already existed (gated behind pref) |
| **inherited_ui** | pointer-events | already existed |
| | cursor | already existed |
| | caret-color | already existed |
| | color-scheme | already existed (gated behind pref) |

**Totals: 44 enabled (stylo), 52 already existed**

---

## Presentation Attribute (`svg_presentation_attr_test.html`) — 52 total

| Section | Property | Status |
|---------|----------|--------|
| **inherited_svg** | fill | enabled |
| | fill-opacity | enabled |
| | fill-rule | enabled |
| | clip-rule | enabled |
| | stroke | enabled |
| | stroke-width | enabled |
| | stroke-dasharray | enabled |
| | stroke-dashoffset | enabled |
| | stroke-linecap | enabled |
| | stroke-linejoin | enabled |
| | stroke-miterlimit | enabled |
| | stroke-opacity | enabled |
| | marker-start | enabled |
| | marker-mid | enabled |
| | marker-end | enabled |
| | paint-order | enabled |
| | shape-rendering | enabled |
| | color-interpolation | enabled |
| | color-interpolation-filters | enabled |
| | text-anchor | enabled |
| **svg** | cx | enabled |
| | cy | enabled |
| | r | enabled |
| | rx | enabled |
| | ry | enabled |
| | x | enabled |
| | y | enabled |
| | clip-path | enabled |
| | vector-effect | enabled |
| | flood-color | enabled |
| | flood-opacity | enabled |
| | lighting-color | enabled |
| | stop-color | enabled |
| | stop-opacity | enabled |
| | mask-type | enabled |
| **position** | width | enabled |
| | height | enabled |
| **effects** | opacity | enabled |
| **inherited_box** | direction | enabled |
| | writing-mode | enabled |
| | visibility | enabled |
| | image-rendering | enabled |
| **inherited_text** | color | enabled |
| | letter-spacing | enabled |
| | word-spacing | enabled |
| | text-rendering | enabled |
| **text** | unicode-bidi | enabled |
| **font** | font-family | enabled |
| | font-style | enabled |
| | font-weight | enabled |
| | font-size | enabled |
| **inherited_ui** | pointer-events | enabled |

**Totals: 52 enabled (element.rs)**

---

## Summary

| Category | Total | Enabled | Already Existed |
|----------|-------|---------|-----------------|
| CSS inline style | 96 | 44 | 52 |
| Presentation attribute | 52 | 52 | 0 |
