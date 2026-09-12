# Servo ↔ usvg integration layer

`components/layout/svg/` is the **Phase-2 bridge** that builds a `usvg::Tree` directly
from Servo's SVG DOM and rasterizes it with resvg.

## The core idea

Servo's earlier SVG path (the "Phase-1" / vector-image cache path) serialized the SVG
subtree back to XML and let usvg re-parse it. That discards the CSS cascade: usvg's
parser only sees *presentation attributes* (`fill="red"`), so anything set through a
stylesheet or inherited through CSS is lost.

The bridge instead walks the DOM **on the layout thread**, where **computed styles are
already available**, and builds the `usvg::Tree` via usvg's public constructors. Reading
`ComputedValues` means `fill`, `stroke`, `opacity`, and the geometry longhands
(`cx`/`cy`/`r`/`rx`/`ry`/`x`/`y`) come from the **post-cascade result**, so stylesheets
and presentation attributes both apply — exactly like the rest of Servo's SVG styling.

The finished tree is then handed to `resvg::render`, which paints it into a pixmap.
Servo treats resvg as a black box: **tree in, pixels out.** No resvg internals are
relied upon beyond the `usvg::Tree` model (documented in [`USVG.md`](USVG.md)).

---

## 1. Layering

```
components/layout/svg/
├─ mod.rs                  re-exports: build_usvg_tree, rasterize_svg_tree
├─ usvg_builder/           ENTRY + per-element build facades + SvgContext
│   ├─ mod.rs              SvgContext, build_usvg_tree, build_usvg_node (dispatch),
│   │                      needs_carry, carry_group
│   ├─ group.rs            build_group, build_svg, build_use, build_use_symbol
│   │                                              → usvg::Group
│   ├─ path.rs             build_shape        → usvg::Path  (+ marker siblings)
│   ├─ image.rs            build_image        → usvg::Image
│   ├─ text.rs             build_text         → usvg::Text
│   └─ marker.rs           build_markers      → sibling usvg::Group s
├─ effects/                recursive node wrappers (may call back into usvg_builder)
│   ├─ mod.rs              Effects, resolve_effects (clip+mask+filter triple)
│   ├─ paint.rs            build_pattern      → usvg::Pattern
│   ├─ clip.rs             resolve_clip_path  → usvg::ClipPath
│   ├─ mask.rs             resolve_mask       → usvg::Mask
│   └─ filter.rs           resolve_filter     → usvg::Filter
├─ primitives/             leaf parsing/geometry/text (NO upward dependencies)
│   ├─ attrs.rs            attribute/value parsing, is_group_element, parse_view_box
│   ├─ geometry.rs         normalized_diagonal, path parsing, pure math
│   ├─ shape.rs            resolve_shape_path (element → tiny_skia_path::Path)
│   ├─ paint.rs            build_fill/build_stroke, gradients, collect_paint_servers
│   ├─ image.rs            decode_image, image_geometry
│   └─ text/               font.rs (SvgFonts), whitespace.rs, mod.rs (chunks/spans)
└─ raster.rs               rasterize_svg_tree (resvg render → WebRender pixels)
```

**Dependency direction is strictly downward**, with one sanctioned exception:

```
usvg_builder  ──────────────►  effects  ──────────────►  primitives
      ▲                            │
      └────────── (mutual recursion) ◄─────────────── (pattern/clip/mask/filter
                                                       content is built via build_usvg_node)
```

* `primitives` depend on nothing above them — they are Servo-style + usvg-constructor
  leaves.
* `effects` build **recursive** definitions (`<pattern>`, `<clipPath>`, `<mask>`,
  `<filter>` all have child subtrees), so they call back up into
  `usvg_builder::build_usvg_node`. This cycle is *module-level*, not type-level, so it is
  fine in Rust.
* `usvg_builder` is the facade: thin `build_*` functions that read a little per-element
  state, delegate the parsing/math/placement to the lower layers, and assemble the
  resulting `usvg::Node`(s).

---

## 2. Building the tree — the DOM scan

Everything starts at [`usvg_builder::build_usvg_tree`](usvg_builder/mod.rs), called from
`replaced.rs` when the replaced-content machinery sees an `<svg>` element. It does, in
order:

1. **Guard + size** — confirm the node is an `SVGSVGElement`; resolve the
   `width`/`height` into a `usvg::Size` and parse `viewBox` → `Option<usvg::ViewBox>`.
   The viewBox transform is deliberately **not** baked in here — content stays in viewBox
   coordinates and the mapping is applied at raster time (so `preserveAspectRatio` is
   honoured uniformly rather than distorted by a CSS-box stretch).

2. **Normalized diagonal** — `√(w² + h²) / √2`, the reference length for `<percentage>`
   `stroke-width` / `stroke-dasharray` / `stroke-dashoffset`.

3. **Font resolver** — `SvgFonts::new(context)` builds usvg's fontdb + resolver, pulling
   faces from the script thread's `FontContext` on demand (so `<text>` lays out into
   glyph outlines with real fonts).

4. **Collect element ids (document-wide)** — `collect_element_ids` walks the *whole
   document* (not just the `<svg>` subtree) building an `id → element` map, so
   `<use href="#id">` can resolve references that live in sibling subtrees or `<defs>`.

5. **Collect paint servers** — `collect_paint_servers` gathers `<linearGradient>` /
   `<radialGradient>` definitions (and their `<stop>` children) into a `Gradients`
   struct, and records `<pattern>` elements for a second pass. Gradients are collected
   first; patterns are built second so their content can reference the already-collected
   gradients.

6. **Build patterns** — each recorded `<pattern>` is built immediately via
   `effects::paint::build_pattern` and inserted, so a nested `<pattern>` can resolve a
   previously built one.

7. **Main walk** — a fresh `usvg::Group::empty()` root, then every child of the `<svg>`
   element is converted via `build_usvg_node` and pushed onto the root.

8. **Finalize** — `usvg::Tree::new(size, root)` then `tree.finalize()` (see
   [`USVG.md` §4](USVG.md)): recompute bounding boxes, collect paint servers / clip /
   mask / filter, and rewrite `objectBoundingBox` units to `userSpaceOnUse`.

The result is `(tree, Option<ViewBox>)`.

### The dispatcher: `build_usvg_node`

Every DOM node funnels through one function that decides its fate by
`LayoutElementType`:

| Element | Action |
|---|---|
| `<svg>` (nested) | `build_svg` — new viewport |
| `<defs>`, `<symbol>` | **skipped** (definition-only; reached only via `<use>`) |
| group-like (`<g>`, `<a>`, `<clipPath>`, `<mask>`) | `build_group` |
| `<use>` | `build_use` / `build_use_symbol` |
| `<text>` | `build_text` |
| `<image>` | `build_image` |
| any shape | `build_shape` |

`build_usvg_node` returns `Vec<usvg::Node>` (not a single node) because a shape carrying
markers emits the path *plus one sibling group per placed marker* — markers are siblings
of the path in usvg, not children.

### The `SvgContext`

`SvgContext<'a, 'dom>` is the shared handle passed to every builder:

```rust
pub(crate) struct SvgContext<'a, 'dom> {
    context:   &'a LayoutContext<'a>,                    // style/device access
    gradients: &'a Gradients,                            // collected paint servers
    defs:      &'a HashMap<String, ServoLayoutElement<'dom>>,  // id → element
    diagonal:  f32,                                      // normalized diagonal
    fonts:     &'a SvgFonts,                             // font resolver
}
```

It exposes three small helpers the builders rely on: `computed_style` (the post-cascade
`ComputedValues`), `transform_attr` (parse the `transform` attribute), and `opacity`.

---

## 3. Per-node builders

### Shapes → `usvg::Path` ([`path.rs`](usvg_builder/path.rs))

`build_shape` handles all basic shapes uniformly. It:

1. resolves the geometry (`primitives::shape::resolve_shape_path` — reads the CSS
   geometry longhands from `computed`, the rest from attributes) into a
   `tiny_skia_path::Path`;
2. resolves `fill`/`stroke` (`primitives::paint::build_fill` / `build_stroke`), with a
   `<use>`-host inheritance rule: when reachable through `<use>`, `fill`/`stroke` come
   from the host unless the element explicitly sets them;
3. computes the stroke width for marker scaling;
4. builds the `usvg::Path` node, then appends **marker sibling groups**
   (`marker::build_markers`);
5. resolves effects and, if the element carries a non-identity transform / opacity /
   clip / mask / filter, wraps the nodes in a **carrying group** (`needs_carry` +
   `carry_group`).

### Markers → sibling groups ([`marker.rs`](usvg_builder/marker.rs))

Markers are the one non-`usvg`-type file in the builder folder: they don't map to a
single `usvg` node type, they emit sibling `Group`s next to a shape's path. The whole
subsystem (vertex/angle tangent math, `marker-start`/`marker-mid`/`marker-end` reference
resolution, `refX`/`refY`/`markerWidth`/`markerHeight` viewport + orientation, and group
assembly) lives here, called only from `build_shape`.

### Images → `usvg::Image` ([`image.rs`](usvg_builder/image.rs))

`build_image` delegates to `primitives::image::decode_image` (data-URI → raw encoded
bytes + intrinsic size) and `image_geometry` (aspect/align transform), then assembles an
`Image` node wrapped in an inner align group and an outer effect group.

### Text → `usvg::Text` ([`text.rs`](usvg_builder/text.rs))

`build_text` delegates to `primitives::text` (position/rotation lists, chunk/span
collection, `textPath`, whitespace trimming, font conversion), builds the `Text`, then
lays it out into glyph outlines via usvg's own engine. It resolves clip/mask **and**
filter (a `<text>` can carry `filter`).

### Groups → `usvg::Group` ([`group.rs`](usvg_builder/group.rs))

* `build_group` — `<g>`/`<a>`/`<clipPath>`/`<mask>`: iterate children, compute the object
  bbox, resolve effects, and write transform/opacity/effects **directly onto the group**
  (no wrapper needed, unlike a shape).
* `build_svg` — nested `<svg>`: an outer group carries the `transform` attribute (and a
  synthetic viewport clip when `overflow` isn't `visible`), an inner group carries the
  `x`/`y` + `viewBox`→viewport transform.
* `build_use` / `build_use_symbol` — `<use>`: translate by `x`/`y`, then recurse into the
  referenced element's nodes (via the `defs` map). A `<use href="#symbol">` establishes a
  new viewport like a nested `<svg>`.

---

## 4. Rasterization

[`rasterize_svg_tree`](raster.rs) is the resvg boundary:

1. clamp the raster size to a 5000 px max, allocate a `tiny_skia::Pixmap`;
2. compute the **root transform**: with a viewBox, `view_box.to_transform(img_size)`
   (honouring `preserveAspectRatio`); without one, a non-uniform scale onto the tree's
   natural size;
3. call **`resvg::render(tree, transform, pixmap)`** — the whole "black box" step;
4. upload the raw pixels to WebRender's image cache, keyed by `(DOM node, raster size)` so
   a re-layout of the same SVG updates pixels in place instead of leaking image keys.

---

## 5. Data flow in Servo after our modification

The bridge is wired into Servo's replaced-content path in
[`components/layout/replaced.rs`](../replaced.rs):

```
layout of an <svg> element
        │
        ▼
ReplacedContents::for_element(...)
        │  node.as_svg() → svg_kind_size(...)
        ▼
svg_kind_size:
    • resolves width/height/natural size
    • (legacy) queues serialization + vector-image cache        [Phase-1 fallback]
    • crate::svg::build_usvg_tree(node, context)                [Phase-2, NEW]
          → Option<(usvg::Tree, Option<ViewBox>)>
    • stores Arc<usvg::Tree> in ReplacedContentKind::SVGElement.svg_tree
        │
        ▼
fragment construction (ReplacedContents::make_fragment)
    • computes raster_size = content_size × device_pixel_ratio
    • if svg_tree is Some:
          rasterize_svg_tree(image_cache, tree, node, raster_size, view_box)
              → Some(ImageKey)  →  Fragment::Image { image_key, … }   [preferred path]
    • else: legacy vector-image cache path                          [fallback]
        │
        ▼
WebRender draws the ImageKey like any other raster image.
```

Key points:

* **Everything happens synchronously on the layout thread.** The tree is built and
  rasterized in one pass; there is no async decode or re-serialization for the common
  case.
* **The CSS cascade applies** because the tree is built from `ComputedValues`, not from
  re-parsed XML.
* **The legacy path is kept as a fallback** (used when `build_usvg_tree` returns `None`,
  e.g. a non-`<svg>` node or an empty tree); the two paths are mutually exclusive and the
  new one takes precedence.
* **Raster result is just an `ImageKey`** — downstream (WebRender, compositor) sees an
  ordinary raster image and needs no SVG awareness.

The public surface is two functions, both `pub(crate)` and re-exported from
[`mod.rs`](mod.rs):

```rust
pub(crate) use raster::rasterize_svg_tree;
pub(crate) use usvg_builder::build_usvg_tree;
```
