# Incremental SVG Updates

How Servo's `servo-svg` engine avoids rebuilding the whole SVG render tree on
every small DOM change.

## The problem

Servo re-runs layout whenever the DOM changes. Before this work, *any* SVG
change caused a complete rebuild of the SVG render tree (`SvgTree`):
`build_svg_tree` re-walked the entire DOM and reconstructed every node and every
definition (gradients, clip-paths, patterns, masks, filters, markers) from
scratch.

For a static or lightly-animated SVG this is acceptable. But when only one
attribute of one shape changes — or a single `<text>` glyph is updated —
rebuilding the entire tree is wasted work, and the cost scales with document
size.

## The idea: incremental updates in levels

We decomposed the fix into a ladder of four levels, each building on the
previous:

| Level | Name | What it does | Status |
|---|---|---|---|
| 0 | Full rebuild | The baseline: rebuild the whole tree every reflow. | — |
| 1 | Correct invalidation | Rebuild *only when something actually changed*. | ✅ done |
| 2 | Incremental build | Even when rebuilding, *reuse unchanged subtrees*. | ✅ done |
| 3 | Incremental render | Repaint *only dirty pixels*. | ⏭️ out of scope |

Levels 1 and 2 are implemented. Level 3 (dirty-pixel rendering) is deliberately
out of scope for this work.

## Background: the two-stage pipeline

The engine is split in two:

1. **Build** — `build_svg_tree` walks the DOM and produces an immutable
   `Arc<SvgTree>` (a tree of `SvgNode`s plus typed definition maps). This lives
   in [`components/layout/svg/`](.) and runs during layout.
2. **Render** — the `servo-svg` model is drawn to pixels by vello.

Incremental work targets the **build** stage: make it cheap to re-produce the
`SvgTree` after a small DOM change.

## Level 1 — correct invalidation

The first problem was under-rebuilding. SVG presentation attributes (`fill`,
`stroke`, `transform`, …) don't affect CSS layout, so layout didn't treat their
changes as a reason to rebuild the box.

The fix: [`SVGElement::attribute_mutated`](../../script/dom/svg/svgelement.rs)
now dirties the node with `NodeDamage::Other` after the super-class handling:

```rust
self.upcast::<Node>().dirty(cx.no_gc(), NodeDamage::Other);
```

`NodeDamage::Other` produces `RebuildAncestor` damage, which walks up to the
nearest `<svg>` replaced element and re-runs `build_svg_tree`. The result: a
rebuild happens exactly when an SVG attribute changes, and not otherwise.

## Level 2 — incremental build

Level 1 decides *whether* to rebuild. Level 2 makes the rebuild itself cheap:
when `build_svg_tree` re-runs, it reuses the subtrees that haven't changed.

### The cache

A persistent [`SvgSubtreeCache`](mod.rs) lives on the layout thread (surfaced
per reflow through `ImageResolver::svg_subtree_cache`). It is keyed by
`OpaqueNode` — the DOM node that produced each subtree — and each entry stores:

- the built subtree, shared as `Arc<SvgNode>`;
- the inputs it was built against (below).

Because the cache is persistent (it survives reflows, unlike per-reflow state),
a subtree built in one reflow can be reused in the next.

### The dirty signal

A cached subtree is valid only if nothing in it changed. For that we use
`inclusive_descendants_version()` — a monotonically-increasing counter on every
`Node`, bumped on every DOM mutation for the node, all its ancestors, and the
document. When a subtree is looked up, its node's version is compared to the
version recorded when it was built; if they differ, it is rebuilt.

The method is exposed on the [`LayoutNode`](../../shared/layout/layout_node.rs)
trait and delegates to
[`Node::inclusive_descendants_version`](../../script/dom/node/node.rs).

That version counter only catches DOM *mutations*; it does not catch a style
that changed because an *ancestor* changed — for example `<g fill="blue">`
changes the inherited `fill` of every descendant without mutating them. For
those, each entry also stores the node's computed style (`ComputedValues`, an
`Arc`), compared by `Arc::ptr_eq` on lookup. Because the entry keeps the old
`Arc` alive, its address cannot be reused, so a real style change always
compares unequal. The same signal covers external-stylesheet changes and
pseudo-class flips, which restyle the node and allocate a new `ComputedValues`.

### Build inputs

"Unchanged" is not enough — a subtree also depends on its *build context*. A
cache entry is only reused when all of these match:

- **viewport reference dimensions** (`vw`/`vh`) — percentage lengths resolve
  against these;
- **inherited `currentColor`** — `currentColor` is threaded down the tree;
- **the node's computed style** — compared by `Arc::ptr_eq` identity, so a
  subtree is rebuilt when its own or an inherited property, a stylesheet rule,
  or a pseudo-class changes its computed values;
- **the root's class-based `<style>` rules** — a `<style>` edit clears the
  whole root's cache (`ensure_css_rules`).

### What can't be cached

Some subtrees are unsafe to cache and are always rebuilt:

- **Cross-references** (`<use>`, `<textPath>`). The referenced target can change
  without bumping the referencing node's version, so a cached copy could go
  stale. A `has_cross_ref` flag is threaded through the build
  ([`build_render_node`](builder/mod.rs) → `resolve_children` /
  `build_text_node`) and disables caching for any subtree containing one.
- **The root `<svg>`** — returned by value as `SvgTree::root`.
- **`<use>` shadow content** — built with a per-use inherited style, never
  shared between uses.

Definitions stay live: `url(#id)` paint/clip/mask/filter/marker references are
left *transient* in the built tree and resolved at render time against freshly
rebuilt definition maps, so cached subtrees always see definition edits.

### Sharing subtrees

Reuse requires shared ownership, so [`SvgNode::children`](../../svg/model/element/mod.rs)
changed from `Vec<SvgNode>` to `Vec<Arc<SvgNode>>`, and clean subtrees are
stored and returned as `Arc<SvgNode>`.

## Limitations

- **The cache has no eviction.** Entries are replaced only when the same node is
  rebuilt; entries for nodes removed from the document linger until the root's
  cache is cleared. In practice this is a bounded memory concern, not a
  correctness one: a reused `OpaqueNode` pointer is guarded by both the
  `inclusive_descendants_version` check and the computed-style `Arc::ptr_eq`
  check (a new node always computes a fresh `ComputedValues`).
- **Level 3 (incremental render) is not implemented.** The *build* is
  incremental, but rendering still rasterizes the whole tree; only dirty-pixel
  repainting would remove that cost.
