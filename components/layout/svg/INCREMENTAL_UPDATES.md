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

There is a second, parallel input path: a style change arriving through CSS (a
`<style>` rule, a `:hover` rule, a CSS animation) rather than an attribute. Stylo
classifies SVG paint properties (`fill`, `stroke`, …) as paint-only, so such a
change produces only `Repaint` damage, which never propagates up to the `<svg>`
box and would be silently dropped. Layout repairs that in
[`compute_damage_and_rebuild_box_tree_below_dirty_root`](../../layout/traversal.rs):
an SVG element whose own damage is paint/relayout-level is upgraded to
`DescendantHasBoxDamage`, which walks up to the `<svg>` replaced element and
re-runs `build_svg_tree` exactly like the attribute path.

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
  stale. A tri-state [`CrossRef`](builder/mod.rs) is threaded through the build
  ([`build_render_node`](builder/mod.rs) → `resolve_children` /
  `build_text_node`): `None` (no cross-reference — cache normally), `Simple`
  (a single `<use>` whose target is itself cross-reference-free — cache keyed on
  the target's [`CrossRefFingerprint`](mod.rs)), and `Complex` (a `<textPath>`,
  or a `<use>` whose target contains a cross-reference, or any ancestor of one —
  never cached).
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

## Testing

### Observing rebuilds vs. reuse

Every cache decision is logged at `debug` level under the `svg` target. Build and
run with:

```
./mach build --features servo-svg -d                                  # build once (dev + SVG engine)
RUST_LOG=svg=debug ./mach run -d <page>                               # sh / bash
$env:RUST_LOG = "svg=debug"; ./mach run -d <page>                     # PowerShell
```

`-d` selects the dev/debug build on both commands — `debug!` is compiled out of
release builds, and `mach run` picks the matching dev binary. `--features
servo-svg` is a build-time flag: it compiles the SVG engine in, so it goes on
`mach build`, not `mach run`. Point `<page>` at a specific `.html` file, not a
directory.

Each cacheable element emits exactly one line per rebuild:

```
svg: reuse <rect#badge>
svg: build <g#legend> [SubtreeDirty]
svg: build <path> [ComputedStyle]
```

`reuse` means the subtree was served from [`SvgSubtreeCache`](mod.rs) unchanged.
`build` means it was rebuilt, with the `CacheMiss` reason in brackets:

| Reason | What changed |
|---|---|
| `NoEntry` | First build of this node (no cached subtree yet). |
| `SubtreeDirty` | The node or one of its descendants was mutated. |
| `Viewport` | The viewport reference dimensions (`vw`/`vh`) changed. |
| `CurrentColor` | The inherited `currentColor` changed. |
| `ComputedStyle` | The node's computed style changed (own/inherited property, stylesheet rule, or pseudo-class). |
| `ReferencedTarget` | A `<use>`'s referenced target (its version or computed style) changed. |

The root `<svg>` and `<use>` shadow content (built with `inherited = Some`) are
never looked up, so they emit no line. A `Complex` cross-reference — a
`<textPath>`, a `<use>` whose target itself contains a cross-reference, or an
ancestor of one — is looked up but never stored, so it emits
`build <tag> [NoEntry]` on every rebuild and never `reuse`. A *simple* `<use>`
(whose target is cross-reference-free) is cached keyed on its target's
[`CrossRefFingerprint`](mod.rs): it reuses while its target is unchanged and logs
`build [ReferencedTarget]` when its target's version or style changes.

### Manual test cases

Each case starts from a page whose SVG is cached by one clean reflow, then makes
one change and triggers layout. The expected `svg`-target log tells you which
subtrees were reused versus rebuilt. Companion pages with a button per case live
alongside this work (`01_noop_reflow.html` … `07_currentcolor_change.html`);
click the button to change in-document — a full reload rebuilds the DOM and
starts the cache cold, so it never shows `reuse`.

1. **No-op reflow** — mutate a harmless `data-*` attribute on the root `<svg>`.
   `build_svg_tree` runs more than once per reflow (the replaced element's
   contents are constructed from several layout call sites), so the first call
   rebuilds and the next reuses it. Note also that mutating the root re-cascades
   the whole subtree — every descendant gets a fresh `ComputedValues` Arc — so
   the first call over-rebuilds each descendant with `[ComputedStyle]` (the
   conservative side of `ptr_eq`, safe but not minimal) and the second call logs
   `reuse`. For the clean "one node rebuilds, its sibling reuses" signal, see
   case 2.
2. **Leaf attribute change** — change one shape's own attribute, e.g.
   `<rect width="10">` → `width="20"`, or `fill="red"` → `fill="blue"`. That
   rect logs `build [SubtreeDirty]`; its siblings and ancestors log `reuse`.
3. **Inherited style change** — change an ancestor's presentation attribute,
   e.g. `<g fill="blue">` → `fill="green"`. The `<g>` and every styled
   descendant log `build [ComputedStyle]` (their computed style changed without
   a DOM mutation), while unrelated siblings still log `reuse`. This is the case
   that exercises the `Arc::ptr_eq` style-identity check.
4. **External stylesheet change** — edit a CSS rule that targets an SVG element
   but is *not* a class rule in an SVG `<style>` (a class rule there is collected
   by `collect_svg_css_rules`, and editing it clears the whole root cache via
   `ensure_css_rules`; use an id/type selector or a truly external stylesheet).
   The matched elements log `build [ComputedStyle]`. In practice the *siblings*
   also log `build [ComputedStyle]` — a `<style>` text edit makes Servo
   re-cascade styles broadly, reallocating every element's `ComputedValues`, so
   the `ptr_eq` check over-rebuilds them. A fill-only style change produces only
   paint damage in Stylo, so layout upgrades it to box damage for SVG elements
   (see `compute_damage_and_rebuild_box_tree_below_dirty_root` in layout) — no
   helper mutation is needed to observe it.
5. **Cross-reference change** — change a `<use href="#x">` target's definition
   (e.g. a referenced `<path>`). The referenced element logs `build [SubtreeDirty]`;
   the `<use>` that points at it logs `build [ReferencedTarget]` (its target changed),
   while a second `<use>` pointing at an *untouched* target logs `reuse`. The
   companion page makes the contrast sharp: two `<use>` elements reference two
   *different* `<path>` definitions and the button mutates only one. The mutated
   `<path>` logs `build [SubtreeDirty]`, its untouched sibling logs `reuse` (a plain
   `<path>` *is* cacheable), the `<use>` of the mutated target logs
   `build [ReferencedTarget]`, and the `<use>` of the untouched target logs `reuse` — a
   `<use>` is reused exactly when neither it nor its target changed.
6. **Viewport change** — change a nested `<svg>`'s `viewBox` so its child's
   `vw`/`vh` reference dimensions change (percentages resolve against the
   `viewBox` extent). The mutated `<svg>` logs `build [SubtreeDirty]` (its own
   attribute changed), its child logs `build [Viewport]`, while an untouched
   sibling `<svg>` and its child log `reuse`.
7. **`currentColor` change** — change a `color` that a `fill="currentColor"`
   element inherits. That subtree logs `build [CurrentColor]` (or
   `[ComputedStyle]`, since the restyled node's computed style changes too).

The [`#[cfg(test)] mod tests`](mod.rs) in `mod.rs` cover the cache's decision
logic in isolation: the computed-style identity rule (`ptr_eq` matches only the
same allocation) and the store/lookup round-trip with each miss reason.

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
