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
| [`builder`](builder/) | Orchestrates the build: the `SvgTreeBuilder` struct, tag dispatch, and the two build passes | Builder |
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

## The two-phase build

Building a tree is deliberately split so that `url(#id)` references resolve
after every definition is known, without a second DOM walk:

1. **Assemble** — `SvgTreeBuilder::build_render_node` +
   `resolve::resolve_children` walks the DOM and constructs `SvgNode`s.
   References are left *transient*: a fill/stroke is
   `PaintServer::Ref { id, fallback }`, and clip/mask/filter/marker are
   `DefRef::Ref(id)`.
2. **Resolve** — `references::resolve_references` runs once
   `collect_definitions` has built all the typed maps, rewriting every transient
   handle into a typed `Arc` handle. It follows SVG 2's broken-reference
   behavior: a paint server that resolves to neither a gradient nor a pattern
   uses its fallback color, or — with no fallback — drops the paint layer; a
   clip/mask/filter/marker that fails to resolve is dropped.

Within [`builder`](builder/):

| Module | Role |
|---|---|
| `mod` | The `SvgTreeBuilder` struct, tag dispatch (`build_tag`), and definition-map collection |
| `resolve` | Child resolution: walking children, cloning `<use>` content, `<switch>` selection, and the expansion guard (`ResolveState`) |
| `references` | The second pass: rewriting transient `PaintServer::Ref` / `DefRef::Ref` into typed `Arc` handles |
| `text` | `<text>`/`<tspan>` assembly and font shaping |
| `image` | `<image>` assembly and image-key resolution |

## Style layering

[`style`](style/) splits a top-level `build_style` (computed values →
`NodeStyle`) from per-attribute submodules: `fill`, `stroke`, `filter`, and
`marker`. The paint-server machinery itself (`fill`/`stroke` → `PaintServer`)
lives in the model (`servo_svg::style`), not here — this layer only *decides*
which `PaintServer` a given computed style produces.
