# SVG Integration Layer — Architecture

This directory (`components/layout/svg/`) is Servo's **SVG integration layer**:
it bridges Servo's DOM + style system with the pure `servo-svg` render-tree data
model in `components/svg/`. It does not rasterize — it *builds* an
`Arc<SvgTree>`, which the vello renderer draws later.

## Public surface

[`mod.rs`](mod.rs) is the only public API. `build_svg_tree(node, context, w, h)`
is the single entry point (called from `crate::replaced`). Everything else is
`pub(crate)`.

## The four layers

| Layer | Role | Pattern |
|---|---|---|
| [`builder`](builder/) | Orchestrates the build: the `SvgTreeBuilder` struct, tag dispatch, and definition-map collection | Builder |
| [`defines`](defines/) | Collects `<defs>` contents into typed maps (gradients, patterns, clip-paths, masks, filters, markers) | Strategy — `DefinitionParser` trait + one parser type per definition |
| [`primitives`](primitives/) | Leaf parsers with no dependency on the build: attribute parsing, geometry, text, paint, CSS, viewport, transforms | Functions |
| [`style`](style/) | Turns a Servo `ComputedValues` into the model's `NodeStyle` (fill, stroke, paint servers, filter, marker, clip, mask) | Functions |

## Dependency direction

Dependencies point **downward** from orchestration toward leaf parsing:

```
             builder
           /    |     \
          v     v      v
      defines   style   primitives
```

Two edges are worth calling out explicitly:

- **`builder` ⇄ `defines` (the one sanctioned mutual recursion).** `builder`
  calls `defines::DefinitionCollector` to gather definitions; each
  `DefinitionParser` in turn calls `builder::SvgTreeBuilder::build_def_content`
  to build the *children* of a `<clipPath>` / `<mask>` / `<pattern>` /
  `<marker>` into full render nodes. This is the servo-svg analogue of usvg's
  `effects` ⇄ `builder` edge. It is the *only* upward call, and it is
  deliberate: a definition's content is itself a mini render tree.
- **`defines` keeps parsing leaf-only.** Node *assembly* always flows back
  through the builder — the `DefinitionParser` implementations never construct
  nodes themselves, only dispatch on tag/attributes and delegate.

`primitives` and `style` are leaves: neither depends on `builder` or `defines`,
and neither depends on the other.

## Build and render-time resolution

Building a tree leaves every `url(#id)` reference *transient* — a fill/stroke
is `PaintServer::Ref { id, fallback }`, and clip/mask/filter/marker are
`DefRef::Ref(id)`. Definitions are collected into typed maps during the build
(`collect_definitions`), but the transient handles are bound at *render time*:
`servo_svg::document::Defs` looks up each id against the maps it shares with the
tree, so the built `SvgTree` is immutable and clean subtrees can be reused
across incremental reflows.

Broken-reference behavior (SVG 2) is enforced at that lookup: a paint server
that resolves to neither a gradient nor a pattern uses its fallback color, or —
with no fallback — drops the paint layer; a clip/mask/filter/marker that fails
to resolve is dropped.

Within [`builder`](builder/):

| Module | Role |
|---|---|
| `mod` | The `SvgTreeBuilder` struct, tag dispatch (`build_tag`), and definition-map collection |
| `resolve` | Child resolution: walking children, cloning `<use>` content, `<switch>` selection, and the expansion guard (`ResolveState`) |
| `text` | `<text>`/`<tspan>`/`<textPath>` assembly and font shaping (including text-on-path placement) |
| `image` | `<image>` assembly and image-key resolution (raster and vector SVG sources) |

## Incremental build (Level 2)

`build_svg_tree` re-runs on every box rebuild. To avoid rebuilding unchanged
subtrees, the builder keeps a persistent cache ([`SvgSubtreeCache`](mod.rs)) on
the layout thread (surfaced per reflow through `ImageResolver::svg_subtree_cache`),
keyed by `OpaqueNode`. A subtree is reused when, for the DOM node that produced it:

- its `inclusive_descendants_version` is unchanged (no DOM mutation in it or
  below it),
- its computed style is the same allocation (compared by `Arc::ptr_eq`), which
  catches inherited-style changes from an ancestor, external-stylesheet rules,
  and pseudo-class flips,
- the viewport reference dimensions (`vw`/`vh`) match,
- the inherited `currentColor` matches, and
- the root's class-based `<style>` rules match (a `<style>` edit clears the
  whole root's cache).

Subtrees that contain a cross-reference (`<use>` or `<textPath>`) are never
cached, because a referenced target can change without bumping the referencing
node's version. `url(#id)` paint/clip/mask/filter/marker references stay
transient and resolve against the freshly rebuilt definition maps at render
time, so cached subtrees still see definition changes.

**Note on style invalidation.** The computed-style `Arc::ptr_eq` check is what
makes the cache correct in the face of *inherited* style changes: an edit like
`<g fill="blue">` restyles its descendants without mutating them, so their
`inclusive_descendants_version` is untouched, but Stylo allocates a new
`ComputedValues` for each restyled node and the cache rebuilds those subtrees.
The entry holds the old `Arc` alive, so a reused allocation address can never
make a changed style look unchanged.

## Style layering

[`style`](style/) splits a top-level `build_style` (computed values →
`NodeStyle`) from per-attribute submodules: `fill`, `stroke`, `filter`, and
`marker`. The paint-server machinery itself (`fill`/`stroke` → `PaintServer`)
lives in the model (`servo_svg::style`), not here — this layer only *decides*
which `PaintServer` a given computed style produces.
