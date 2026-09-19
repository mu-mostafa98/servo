# `usvg` — the render-tree model, and our local changes

`usvg` is resvg's "model" half. resvg is split into three crates:

| Crate | Role |
|---|---|
| `usvg` | Parse SVG **into a resolved, render-ready tree** (`usvg::Tree`). |
| `tiny-skia` / `tiny-skia-path` | CPU rasterization + 2D path geometry primitives. |
| `resvg` | The renderer: walks a `usvg::Tree` and draws it into a `tiny-skia` pixmap. |

Servo does **not** use usvg's XML parser. Servo already has a fully cascaded SVG DOM
(styles resolved, `fill`/`stroke`/geometry computed), so it builds the *same*
`usvg::Tree` **programmatically** through usvg's public constructors, then hands the
finished tree to `resvg::render`. usvg is therefore the interchange format between
Servo's layout engine and resvg's renderer.

---

## 1. The type hierarchy

### 1.1 The tree

```
usvg::Tree
 ├─ size : Size                         (the SVG viewport / image size)
 ├─ root : Group                        (the renderable document subtree)
 ├─ linear_gradients : Vec<Arc<LinearGradient>>
 ├─ radial_gradients : Vec<Arc<RadialGradient>>
 ├─ patterns        : Vec<Arc<Pattern>>
 ├─ clip_paths      : Vec<Arc<ClipPath>>
 ├─ masks           : Vec<Arc<Mask>>
 ├─ filters         : Vec<Arc<Filter>>
 └─ fontdb          : Arc<fontdb::Database>      (text feature)
```

`Tree` separates two things:

* **`root`** — the *renderable* node tree (the actual drawn content).
* **the `Vec<Arc<…>>` storage** — *referenced* definitions (paint servers, clip paths,
  masks, filters) collected out of the tree so the renderer can look them up by
  pointer. These are shared via `Arc`, so many shapes can point at one gradient.

### 1.2 The node enum

A `Tree`'s renderable content is a forest of `usvg::Node`s — a 4-variant enum:

```rust
pub enum Node {
    Group(Box<Group>),   // container: <g>, <svg>, <a>, <use>, clip/mask/pattern roots
    Path(Box<Path>),     // strokeable/fillable geometry
    Image(Box<Image>),   // raster image
    Text(Box<Text>),     // laid-out text (glyph outlines)
}
```

That is the whole vocabulary of things resvg can draw. Everything else in SVG —
gradients, clip paths, masks, filters, patterns, markers — is either a *property* on
one of these nodes (`fill`, `stroke`, `clip_path`, `mask`, `filters`, …) or a
*referenced subtree* whose own `root` is another `Group`.

### 1.3 The four node kinds

**`Group`** — the universal container. Carries:

| Field | Meaning |
|---|---|
| `id`, `transform`, `abs_transform` | identity + local/absolute transforms |
| `opacity`, `blend_mode`, `isolate` | group compositing |
| `clip_path`, `mask`, `filters` | the group's effects (each is an `Arc` to a `ClipPath`/`Mask`/`Filter`) |
| `children : Vec<Node>` | the sub-tree |

A `Group` is used for `<g>`, `<a>`, nested `<svg>`, `<use>`, and as the `root` of every
`<clipPath>`/`<mask>`/`<pattern>`/`<marker>`/`<filter>` subtree.

**`Path`** — geometry + paint. Holds `data : Arc<tiny_skia_path::Path>` (the actual
outline), plus `fill : Option<Fill>`, `stroke : Option<Stroke>`, `paint_order`,
`rendering_mode` and `abs_transform`. Every SVG basic shape collapses into a `Path`.

**`Image`** — `size`, `rendering_mode`, `kind` (PNG/JPEG/… bytes) and `abs_transform`.

**`Text`** — the highest-level node. A `Text` is *not* drawn from DOM text; usvg lays it
out at build time into a `flattened : Group` of positioned **glyph outlines** (each glyph
a `Path`). Its logical structure is retained for writing/editing:

```
Text
 ├─ dx / dy / rotate              per-character positioning lists
 ├─ writing_mode / direction      layout mode + LTR/RTL (see §5.3)
 ├─ chunks : Vec<TextChunk>       runs sharing a position
 │    └─ spans : Vec<TextSpan>    styled sub-ranges (one <tspan> per span)
 │         ├─ start / end         byte range into chunk.text
 │         ├─ fill / stroke / font / font_size / decoration …
 └─ flattened : Group             the resolved glyph-outline tree (what renders)
```

---

## 2. SVG element → `usvg` type mapping

### Renderable nodes

| SVG element | `usvg` type |
|---|---|
| `<svg>` (root & nested) | `Tree` (root) / `Group` (nested, wrapped) |
| `<g>`, `<a>`, `<switch>` | `Group` |
| `<use>` | `Group` (children = referenced element's nodes, translated) |
| `<symbol>` | *not rendered* — its children are inlined by a `<use>` into a `Group` |
| `<path>`, `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>` | `Path` (all share one geometry path) |
| `<image>` | `Image` |
| `<text>` | `Text` |
| `<tspan>`, `<textPath>` | folded into `TextChunk` / `TextSpan` / `TextFlow::Path` |

### Referenced definitions (stored in `Tree`, referenced by `Arc`)

| SVG element | `usvg` type |
|---|---|
| `<linearGradient>` | `LinearGradient` |
| `<radialGradient>` | `RadialGradient` |
| `<stop>` | `Stop` (a stop inside a gradient's `stops : Vec<Stop>`) |
| `<pattern>` | `Pattern` |
| `<clipPath>` | `ClipPath` |
| `<mask>` | `Mask` |
| `<filter>` | `Filter`, with each child `Primitive` (`Kind`) |
| `<marker>` | *no dedicated node type* — markers become sibling `Group`s next to a `Path` |

### Filter primitives

`<feBlend>`, `<feColorMatrix>`, `<feComponentTransfer>`, `<feComposite>`,
`<feConvolveMatrix>`, `<feDisplacementMap>`, `<feDropShadow>`, `<feFlood>`,
`<feGaussianBlur>`, `<feImage>`, `<feDiffuseLighting>`, `<feSpecularLighting>`,
`<feMerge>`, `<feMorphology>`, `<feOffset>`, `<feTile>`, `<feTurbulence>` all map to a
`usvg::filter::Primitive`, whose `kind` is the `filter::Kind` enum (one variant per
primitive). Each has an `Input` (`SourceGraphic` / `SourceAlpha` / `FillPaint` /
`StrokePaint` / `BackgroundImage` / `BackgroundAlpha` / `Reference(name)`) wiring its
`in`/`in2` attributes.

### Paint and style

| Concept | `usvg` type |
|---|---|
| `fill` / `stroke` value | `Fill` / `Stroke` |
| solid colour | `Paint::Color(Color)` |
| `fill="url(#…)"` | `Paint::LinearGradient` / `Paint::RadialGradient` / `Paint::Pattern` |
| line caps / joins | `LineCap` / `LineJoin` |
| fill rule | `FillRule` (`NonZero` / `EvenOdd`) |
| paint order | `PaintOrder` (fill/stroke/markers ordering) |

### Supporting value types

`Transform` (2D affine), `ViewBox`, `Size`, `Rect` / `NonZeroRect`, `NonZeroF32`,
`PositiveF32`, `Opacity` (= `NormalizedF32`), `Color`, `Units`
(`UserSpaceOnUse` / `ObjectBoundingBox`), `Visibility`, `SpreadMethod`,
`ContextElement`, `MaskType` (`Luminance` / `Alpha`), `BlendMode`, `ShapeRendering`,
`ImageRendering`, `TextRendering`, `WritingMode`, `TextDirection`.

---

## 3. How the tree is represented

1. **Containment is by `Group.children : Vec<Node>`** — a plain recursive tree. Leaves
   are `Path` / `Image` / `Text`; interior nodes are `Group`.
2. **Transforms are pre-resolved.** Each node carries an `abs_transform` (the product of
   all ancestor transforms) in addition to its local `transform`. resvg renders directly
   from `abs_transform` without walking ancestors, so Servo's builder must propagate the
   accumulated transform top-down as it builds (it does — via the `parent_abs_transform`
   argument threaded through every `build_*`).
3. **Bounding boxes are cached per node**, in two spaces: object space (`bounding_box`,
   `stroke_bounding_box`) and absolute/canvas space (`abs_bounding_box`, …). Groups also
   keep a `layer_bounding_box` (their own rendering layer's bounds).
4. **Effects and paint servers are *out-of-line*.** A shape does not embed its gradient;
   it holds `Paint::LinearGradient(Arc<LinearGradient>)`. A group's `clip_path`/`mask`
   hold an `Arc` to a `ClipPath`/`Mask` whose *content* lives in the clip/mask's own
   `root : Group` — not in the main tree. `Node::subroots` exists to walk these hidden
   subtrees uniformly.
5. **Nothing is resolved at render time that could be resolved at build time.** Gradients,
   patterns, filters, text layout, and `objectBoundingBox` → `userSpaceOnUse` unit
   rewriting are all baked in during construction, so `resvg::render` is a straight
   paint.

---

## 4. The two-phase construction contract

usvg's XML parser has a "convert, then post-process" pipeline. Building programmatically
mirrors it as two explicit calls:

1. **`Tree::new(size, root)`** — create an empty tree.
2. **populate `root`** (via `Group::push_child` / field assignment) and the referenced
   definitions.
3. **`Tree::finalize()`** — the counterpart of the parser's post-parse pass. It:

   * recomputes **every bounding box bottom-up** (including inside mask/clip subtrees),
   * **collects** paint servers, clip paths, masks and filters into the `Tree` storage,
   * **rewrites `objectBoundingBox` paint-server units into `userSpaceOnUse`**
     (see §5.2).

Skipping `finalize()` produces a tree that looks fine but has empty bounding boxes and
uncollected paint servers — shapes silently vanish at render time.

---

## 5. What we changed (staged, in the local `usvg` checkout)

The upstream `usvg` is designed to be built *only* by its own XML parser: almost every
type and field is `pub(crate)`, and there is no public construction path. Servo needed a
**public, programmatic construction API**. The staged changes (`git diff --cached` in
`D:/Projects/resvg`, branch `usvg-for-servo`) fall into six groups:

### 5.1 Make everything public + add constructors

Every `pub(crate)` field/type/constructor Servo needs was made `pub`, and `new`/`empty`
constructors were added where none existed. Files touched:

* `tree/mod.rs` — `NonEmptyString`, `Units`, `Visibility`, `ContextElement`,
  `BaseGradient`, `LinearGradient`, `RadialGradient`, `Stop`, `Pattern`, `Stroke`,
  `Fill`, `ClipPath`, `Mask`, `Group`, `Path`, `Image` all get `pub` fields and a
  constructor.
* `tree/filter.rs` — `Filter`, `Primitive`, and **all** 16 `fe*` primitive structs get
  `pub` fields and constructors (`Blend::new`, `ColorMatrix::new`, `GaussianBlur::new`,
  …).
* `tree/text.rs` — `Font`, `TextDecorationStyle`, `TextDecoration`, `TextSpan`,
  `TextPath`, `TextChunk`, `Text` get `pub` fields and constructors.
* `tree/geom.rs` — `ViewBox` (was `pub(crate)`) → `pub`.

New construction entry points in `tree/mod.rs`:

* `Tree::new(size, root)` + `Tree::root_mut()` + **`Tree::finalize()`** (§4).
* `Group::empty()` (was `pub(crate)`), **`Group::push_child(child)`**,
  **`Group::compute_object_bbox()`** — recompute bounding boxes and return the object
  bbox *before* `finalize` runs, so mask/clip regions in `objectBoundingBox` units can
  be resolved while building.
* `Path::new(…)`, `Image::new(…)`, `Text::new(id, abs_transform)`.

### 5.2 The `finalize()` machinery (the biggest addition)

Servo builds shapes with `objectBoundingBox` paint-server units *preserved*, because a
shape's bounding box isn't known until its path data is set. The XML parser resolves
these during its post-parse pass; Servo now has the equivalent, added to
`tree/mod.rs`:

* `calculate_bounding_boxes_recursive(group)` — bottom-up bbox computation that also
  descends into mask/clip-path `root` subtrees (a shape wrapped for `opacity < 1` inside
  a mask would otherwise have no `layer_bounding_box` and be skipped).
* `resolve_object_bounding_box(group, counter)` + `resolve_paint_units(…)` — rewrite
  each `objectBoundingBox` linear/radial gradient and pattern to `userSpaceOnUse` by
  post-concatenating the shape's bbox transform. When a gradient/pattern is **shared** by
  shapes with *different* bboxes (and `Arc::get_mut` fails), it is **cloned** with a
  fresh generated id (`__linear_gradient_N` / `__radial_gradient_N` / `__pattern_N`) so
  each shape gets its own resolved copy.
* `push_pattern_transform(root, transform)` — wraps a pattern's content in a group
  carrying `patternContentUnits`/`viewBox` scaling (mirrors the parser's helper).

`Tree::finalize()` calls these in the same order the parser's `convert_doc` does.

### 5.3 Text direction (RTL) support

`tree/text.rs` adds a new `TextDirection` enum (`LeftToRight` / `RightToLeft`) and a
`direction` field on `Text`, plus `Text::direction()`.

`text/layout.rs` threads `direction` through the whole shaping engine so RTL text is
laid out correctly:

* `shape_text_with_font` now uses the direction to pick the BIDI **base level**
  (`unicode_bidi::Level::ltr()` vs `rtl()`).
* `process_anchor` flips `start`/`end` for `RightToLeft` (in RTL, "start" is the right
  edge).

`parser/text.rs` adds `convert_direction`, reading the `direction` attribute/ancestor
(the XML-parser counterpart).

### 5.4 Text layout entry points

* `text/mod.rs` — `text::layout(text, resolver, cache)` as the **public** wrapper over
  the internal `convert`, so a programmatically built `Text` can be laid out into glyph
  outlines. Also `text::path_length(path)` made public (needed to resolve a
  `<textPath startOffset="…%">` against the full path length).

### 5.5 Visibility of the parser plumbing

* `parser/mod.rs` — `parser::text` made `pub(crate)`; `converter::Cache` re-exported
  `pub`.
* `parser/converter.rs` — `Cache::new` made `pub`.
* `parser/text.rs` — `path_length` made `pub(crate)` (shared with `text::path_length`).

### 5.6 A regression test

`tests/programmatic.rs` (new) — builds a tree programmatically and asserts `finalize()`
produces the expected size, bounding boxes and child count, including a nested-group
absolute-transform case.
