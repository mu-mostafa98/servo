# Servo SVG — Architecture & Data Flow

> A high-level overview of the current SVG PR: the shape of the work and how data
> moves through it, not the implementation details.

## Overview

Native SVG rendering for Servo is split into two clean halves:

1. **A pure SVG data model + render pass** — the `servo-svg` crate
   (`components/svg/`), which holds no Servo/layout/DOM types.
2. **A DOM → model integration layer** — `components/layout/svg/`, inside
   `servo-layout`, which walks Servo's layout DOM, parses SVG geometry, and
   assembles the model.

Build and render are two separate passes, with an incremental cache in between so
unchanged SVG isn't rebuilt on every reflow.

## Architecture

```
                        servo-layout
┌──────────────────────────────────────────────────────┐
│  layout/svg/                                          │
│  · walks the Servo layout DOM                         │
│  · parses <rect> geometry (computed style + attrs)    │
│  · consults the incremental subtree cache             │
│  · assembles a pure SvgTree                           │
└───────────────────────────┬──────────────────────────┘
                            │  Arc<SvgTree>
                            ▼
                        servo-svg
┌──────────────────────────────────────────────────────┐
│  model/  — pure data model                           │
│  · SvgTree → SvgNode → SvgTag                        │
│    (shape / text / image / container)                │
│  · geometry, style, units, transforms                │
│  · no Servo / layout / WebRender types               │
│                                                      │
│  render/  — render pass                              │
│  · walks the SvgTree (currently a debug-log stub)    │
└──────────────────────────────────────────────────────┘
```

### The two layers

- **`servo-svg` (`components/svg`)** — the engine. A `SvgTree` of `SvgNode`s, each
  carrying a tag (shape / text / image / container), a style, geometry, and units.
  The model is pure — it depends only on small standalone crates, never on Servo's
  DOM or layout. The `render/` half currently walks the tree and `debug!`-logs each
  node's tag; WebRender display-list emission lands with the real renderer.

- **`layout/svg` (`components/layout/svg`)** — the bridge. Lives in `servo-layout`
  and knows Servo's layout DOM. It parses `<rect>` geometry, turns each element
  into an `SvgNode`, and stitches them into an `SvgTree`. Internally layered as: a
  builder (orchestration + caching), primitives (attribute / geometry parsing),
  and a cache.

## Data flow

Two passes, cleanly separated:

```
layout DOM
     │  build
     ▼
build_svg_tree()  ──►  Arc<SvgTree>   (pure model)
     │
     │  stored on the replaced element
     ▼
display-list  visit_svg
     │  render
     ▼
render_svg_tree()  ──►  (debug-log walk — display items in a later PR)
```

1. **Build** — `build_svg_tree` walks the DOM, maps each SVG element to an
   `SvgNode`, parses `<rect>` geometry (`x`/`y`/`rx`/`ry` from computed style,
   `width`/`height`/`pathLength` from attributes), and returns an `Arc<SvgTree>`.
2. **Hand-off** — the tree is kept on the replaced element's `SvgFragment` (its
   `tree` field), wrapped in a `Fragment::Svg`.
3. **Render** — when the fragment paints, `render_svg_tree` walks the tree. It is
   currently a `debug!`-log traversal; emitting display items is the next PR.

## Inside `build_svg_tree`

The builder walks the DOM depth-first, consulting the subtree cache at every
non-root node so unchanged subtrees are reused rather than rebuilt.

```
build_svg_tree(node, ctx, w, h)
  └─ SvgTreeBuilder::build()
       └─ build_node(root)                     // root is never cached
            ├─ as_element()?                   // skip text / whitespace / comments
            ├─ computed_style(node)            // cache key · Arc<ComputedValues>
            ├─ inclusive_descendants_version() // cache key · dirty version
            ├─ [non-root] cache.lookup ── hit ──► return Arc<SvgNode> (no rebuild)
            │                         └─ miss ──► rebuild below
            ├─ build_tag(element, computed, vw, vh)
            │     ├─ "svg" / "g"  → Container
            │     └─ "rect"       → build_shape → parse_rect
            │                         x/y/rx/ry ← computed.get_svg()
            │                         width/height/pathLength ← attributes
            ├─ recurse children   → Vec<Arc<SvgNode>>
            ├─ Arc::new(SvgNode { id, tag, style, transforms, viewport, children })
            └─ [non-root] cache.store(node, version, style, Arc<SvgNode>)
  → SvgTree { root, viewport, empty def maps }
```

### Cache key

| Input | Type | Invalidate on |
|---|---|---|
| node identity | `OpaqueNode` | — (the map key) |
| `inclusive_descendants_version` | `u64` | any mutation in the subtree |
| computed style | `Arc<ComputedValues>` (compared by `ptr_eq`) | own/inherited property, stylesheet rule, pseudo-class |

A `lookup` miss reports which input changed — `NoEntry` / `SubtreeDirty` /
`ComputedStyle` — so the debug log can attribute a rebuild to its cause.

## Incremental updates

Rebuilding all of SVG on every reflow would be wasteful, so the builder keeps a
**subtree cache**. Each node's subtree is keyed by its DOM identity, its
`inclusive_descendants_version`, and its computed style:

- **Reuse** a subtree while nothing relevant changed — e.g. a no-op reflow, or an
  untouched sibling.
- **Rebuild** when the node (or a descendant) was mutated, or when its computed
  style changed (inherited property, stylesheet rule, pseudo-class).

The current PR covers these two signals. Referenced-target, viewport, and
`currentColor` tracking are follow-ups.

## Current scope

**In this PR**

- `<rect>` geometry: `x`/`y`/`rx`/`ry` from computed style (attribute fallback),
  `width`/`height`/`pathLength` from attributes.
- The full build → cache pipeline, producing an `Arc<SvgTree>`.
- The render pass is a debug-log walk (no display items yet).

**Stubbed / follow-up**

- Other shapes, `<text>`, `<image>`; gradients, patterns, clip / mask / filter /
  markers.
- `fill` / `stroke`, opacity, transforms, nested `<svg>` viewports.
- The renderer: emit WebRender display items (currently only a `debug!` traversal).
