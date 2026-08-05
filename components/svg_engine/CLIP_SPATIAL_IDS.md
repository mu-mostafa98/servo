# Clip IDs, Clip Chains, and Spatial IDs in `svg_engine`

A complete map of how `ClipId`, `ClipChainId`, and `SpatialId` flow through the
`components/svg_engine` rendering pipeline. All three are WebRender identifier
types (from `webrender_api`) — they are **separate concerns that travel
together** through the recursive traversal.

> Scope: `components/svg_engine`. The unrelated clip machinery in
> `components/layout/` (HTML/CSS layout) is noted separately at the end.

---

## 1. The Two WebRender ID Systems

| Type            | Meaning                                                                                       | Created by                                        | Lives in       |
| --------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------- | -------------- |
| **`SpatialId`** | Identity of a coordinate-space node in WebRender's spatial tree (a reference frame). Children inherit the parent's unless a transform/viewBox pushes a new one. | `wr.push_reference_frame(...)`                     | spatial tree   |
| **`ClipId`**    | Identity of a single clip region (a rect or rounded rect). Parent of a clip chain.           | `wr.define_clip_rect` / `define_clip_rounded_rect` | clip store      |
| **`ClipChainId`** | A *chain* that composes one or more `ClipId`s (with an optional parent chain) — the actual thing passed to display items & stacking contexts. | `wr.define_clip_chain(parent_option, [clip_ids])` | clip store      |

### The `clip_chain_option` helper

`clip_chain_option` in [`renderer/helpers.rs:57-63`](src/renderer/helpers.rs#L57-L63)
converts a `ClipChainId` to `Option<ClipChainId>`, mapping
`ClipChainId::INVALID` → `None`. This is what "no parent clip" means to
WebRender.

```rust
/// Convert a [`ClipChainId`] to an [`Option`], returning `None` for
/// [`ClipChainId::INVALID`] and `Some(id)` otherwise.
pub(crate) fn clip_chain_option(id: ClipChainId) -> Option<ClipChainId> {
    if id == ClipChainId::INVALID {
        None
    } else {
        Some(id)
    }
}
```

---

## 2. `SpatialId` — created in `renderer/transform.rs`

### `TransformResult`

[`renderer/transform.rs:22-27`](src/renderer/transform.rs#L22-L27) — the unit
that threads spatial state through each transform:

```rust
pub(crate) struct TransformResult {
    pub child_origin: LayoutPoint,
    pub child_spatial_id: SpatialId,
    pub pushed_frame: bool,   // caller must wr.pop_reference_frame()
}
```

### `apply_transform_op`

[`renderer/transform.rs:33-104`](src/renderer/transform.rs#L33-L104) — the
spatial-id factory. The rule is simple:

- **Translate** → no new frame; `child_spatial_id` stays the **parent's**
  `spatial_id`, origin shifts by `(tx, ty)`. ([L40-47](src/renderer/transform.rs#L40-L47))
- **Scale / Rotate / SkewX / SkewY / Matrix** → push a reference frame via
  `push_reference_frame`, return the **new** `frame_id` as `child_spatial_id`,
  origin becomes `(0,0)`. ([L48-103](src/renderer/transform.rs#L48-L103))

### `push_reference_frame`

[`renderer/transform.rs:107-124`](src/renderer/transform.rs#L107-L124) — the
single chokepoint that calls `wr.push_reference_frame(origin,
parent_spatial_id, …)` and returns the new `SpatialId`. Note the
`ReferenceFrameKind::Transform` with `is_2d_scale_translation: false,
should_snap: false`.

```rust
fn push_reference_frame(
    origin: LayoutPoint,
    parent_spatial_id: SpatialId,
    transform: LayoutTransform,
    wr: &mut DisplayListBuilder,
) -> SpatialId {
    wr.push_reference_frame(
        origin,
        parent_spatial_id,
        TransformStyle::Flat,
        PropertyBinding::Value(transform),
        ReferenceFrameKind::Transform {
            is_2d_scale_translation: false,
            should_snap: false,
            paired_with_perspective: false,
        },
    )
}
```

### `push_viewbox_frame` (second reference-frame push)

A **second** reference-frame push happens outside transforms:
`push_viewbox_frame` in [`traversal.rs:89-123`](src/traversal.rs#L89-L123)
computes the viewBox→viewport scale/translate and pushes its own frame,
returning `(new_origin, new_spatial_id, should_pop=true)`.

> ⚠️ **Known FIXME** at [`transform.rs:68-69`](src/renderer/transform.rs#L68-L69):
> skew/rotate/matrix breaks the SVG viewport clip inside reference frames,
> currently falling back to a `get_attr` path.

---

## 3. `ClipId` & `ClipChainId` — created in `effects/clip.rs`

[`effects/clip.rs`](src/effects/clip.rs) is the **only** place svg_engine
defines clips. Two functions, both taking `(spatial_id, parent_clip_chain)`
and returning a `ClipChainId`:

### `resolve_node_clip_path` — clip-path (AND / intersection)

[`effects/clip.rs:24-70`](src/effects/clip.rs#L24-L70) — resolves an SVG
`clip-path` reference into a clip chain. For each shape in the clip-path
definition:

1. Get `ClipGeometry` (RoundedRect or Polygon) from `shape.clip_info(...)`.
2. Define a `ClipId`:
   - `RoundedRect` → `wr.define_clip_rounded_rect(spatial_id,
     ComplexClipRegion{…, ClipMode::Clip})` ([L50-57](src/effects/clip.rs#L50-L57))
   - `Polygon` → `wr.define_clip_rect(spatial_id, bounds)` — a
     **bounding-rect fallback** because WebRender 0.69 has no native polygon
     clip ([L58-63](src/effects/clip.rs#L58-L63), [L115-118](src/effects/clip.rs#L115-L118))
3. Chain it: `current_chain = wr.define_clip_chain(clip_chain_option(current_chain), [clip_id])`
   ([L66-67](src/effects/clip.rs#L66-L67)) — so multiple clip-path shapes are
   **intersected** (AND), each child chained to the previous.

If the node has no `clip-path`, the `parent_clip_chain` is returned unchanged
([L32-37](src/effects/clip.rs#L32-L37)).

### `build_mask_clips` — mask (OR / union)

[`effects/clip.rs:82-126`](src/effects/clip.rs#L82-L126) — builds a
**`Vec<ClipChainId>`** — one chain per mask shape (not one combined chain).
Each chain = parent clip **AND** one mask shape ([L121](src/effects/clip.rs#L121)).
Rendering the shape once per chain achieves **union (OR)** masking, as the doc
comment at [L78-81](src/effects/clip.rs#L78-L81) explains. Returns `None` when
no mask.

### `build_viewport_clip` — root of the clip-chain tree

[`traversal.rs:71-85`](src/traversal.rs#L71-L85) — defines a rect clip over the
SVG viewport bounds and chains it onto the incoming `clip_chain_id` — **unless**
`overflow: visible`, in which case it passes the parent chain through
untouched.

---

## 4. `ClipGeometry` — the bridge (`shapes/mod.rs`)

`ClipGeometry` is the enum that bridges SVG shape bounds to WebRender clip
definitions:

```rust
enum ClipGeometry { RoundedRect { bounds, radii }, Polygon { bounds } }
```

Each shape implements `clip_info(svg_origin, units) -> Option<ClipGeometry>`:

| Shape     | File                       | Clip geometry produced                                           |
| --------- | -------------------------- | ---------------------------------------------------------------- |
| Circle    | `shapes/circle.rs`         | rounded rect, corner radius = circle radius                     |
| Ellipse   | `shapes/ellipse.rs`        | rounded rect with independent rx/ry corner radii                 |
| Rectangle | `shapes/rectangle.rs`       | rounded rect with clamped radii                                   |
| Path      | `shapes/path.rs`           | axis-aligned **bounding box** of path segment endpoints           |
| Polyline  | `shapes/polyline.rs`       | axis-aligned **bounding box** via shared `clip_points`            |
| Polygon   | `shapes/polygon.rs`        | delegates to `clip_points` (bounding box)                         |
| Line      | `shapes/line.rs`           | none — lines have no area                                         |

All rounded-rect paths funnel through `all_equal_radius`
([`shapes/mod.rs`](src/shapes/mod.rs)) to build uniform `LayoutSize` corner
radii.

---

## 5. How They Flow Together — `traversal.rs` orchestrator

The two IDs are **threaded as a pair** through the recursive walk. Full
dataflow, entry-point down:

```
render_svg_tree(tree, svg_origin, svg_size, spatial_id, clip_chain_id, wr)   [traversal.rs:33]
  │
  ├─ svg_clip_chain = build_viewport_clip(..., spatial_id, clip_chain_id, …)  [L41]  → ClipChainId (viewport rect chained onto parent)
  ├─ (root_origin, root_spatial_id, pop_frame) = push_viewbox_frame(...)     [L44]  → new SpatialId (viewBox frame)
  │
  └─ render_node(root, root_origin, root_spatial_id, svg_clip_chain, …)      [L52]
       │
       ├─ apply_node_transforms(node, svg_origin, spatial_id, …)             [L165] → (origin, cur_spatial_id, pushed_count, scale)
       │      └─ for each TransformOp: transform::apply_transform_op(op, cur_origin, cur_spatial_id, wr)
       │              └─ push_reference_frame → new SpatialId when pushed_frame=true
       │
       ├─ resolve_node_effects(node, …, cur_spatial_id, clip_chain_id, wr)   [L169] → ResolvedEffects
       │      ├─ resolve_node_clip_path(node, clips, origin, cur_spatial_id, parent_clip_chain, wr) → ClipChainId
       │      ├─ build_mask_clips(node, clips, origin, cur_spatial_id, node_clip_chain, wr) → Option<Vec<ClipChainId>>
       │      └─ get_filter_ops(node, filters) → Option<Vec<FilterOp>>
       │
       ├─ emit_element(node, origin, cur_spatial_id, resolved.clip_chain, …) [L184]  ← clip chain is resolved.clip_chain
       │      ├─ emit_geometry(shape, …, cur_spatial_id, node_clip_chain, …)  [L331]
       │      │     ├─ push_filter_context(filter_ops, cur_spatial_id, node_clip_chain, wr)  ← stacking context uses clip_chain
       │      │     └─ for each mask_chain: emit_shape(shape, …, cur_spatial_id, mask_chain, …)
       │      │           └─ RenderContext { spatial_id, clip_chain_id, … } → shape.render(ctx)
       │      └─ emit_leaf(text/image, …, cur_spatial_id, clip_chain_id, …)  [L433]
       │            └─ RenderContext { spatial_id: cur_spatial_id, clip_chain_id: effective_clip, … }
       │
       ├─ recurse_children(node, origin, cur_spatial_id, resolved.clip_chain, …) [L195]  ← children inherit BOTH cur_spatial_id and resolved.clip_chain
       │
       └─ for _ in 0..pushed_count { wr.pop_reference_frame() }              [L206]  ← pop the spatial frames we pushed
```

### The threading contract

1. **`SpatialId`** changes **only** when a transform or viewBox pushes a
   reference frame. Otherwise the parent's id is reused (translate). It's
   always popped symmetrically ([`traversal.rs:206-208`](src/traversal.rs#L206-L208)).
2. **`ClipChainId`** is **monotonic down the tree** — each node's
   `resolve_node_effects` may extend the chain (clip-path intersects;
   viewport clips), and children inherit the *resolved* chain via
   `recurse_children` ([L195-203](src/traversal.rs#L195-L203), passing
   `resolved.clip_chain`).
3. Both IDs converge at the **`RenderContext`** struct
   ([`traversal.rs:395-403`](src/traversal.rs#L395-L403) and
   [L455-463](src/traversal.rs#L455-L463)) — this is what every shape renderer
   receives:

   ```rust
   RenderContext { style, svg_origin, spatial_id, clip_chain_id, wr, paints, accumulated_scale }
   ```

### The mask OR-vs-clip-path AND distinction

- `clip-path` → **one** combined chain, shapes **intersected** (AND) —
  [`effects/clip.rs:66-67`](src/effects/clip.rs#L66-L67)
- `mask` → **N** separate chains, shape emitted **N times** (OR) —
  [`effects/clip.rs:100-123`](src/effects/clip.rs#L100-L123), consumed at
  [`traversal.rs:353-365`](src/traversal.rs#L353-L365)
- When both exist, masks build on top of the already-clip-pathed
  `node_clip_chain` ([`traversal.rs:235-242`](src/traversal.rs#L235-L242)
  passes `node_clip_chain` as the parent to `build_mask_clips`).

### `ResolvedEffects` / `EffectParams` structs

[`traversal.rs:212-216`](src/traversal.rs#L212-L216):

```rust
struct ResolvedEffects {
    clip_chain: ClipChainId,
    mask_clips: Option<Vec<ClipChainId>>,
    filter_ops: Option<Vec<webrender_api::FilterOp>>,
}
```

[`traversal.rs:135-139`](src/traversal.rs#L135-L139):

```rust
struct EffectParams<'a> {
    mask_clips: &'a Option<Vec<ClipChainId>>,
    filter_ops: &'a Option<Vec<webrender_api::FilterOp>>,
    paints: &'a dyn PaintResourceProvider,
}
```

---

## 6. One-Hop Dependency Subgraph

From the knowledge-graph edges, the call/import structure around these IDs:

- **External entry:** `components/layout/svg/builder.rs` imports both
  [`shapes/mod.rs`](src/shapes/mod.rs) and [`traversal.rs`](src/traversal.rs) —
  it's what calls `render_svg_tree`, supplying the **initial** `SpatialId` +
  `ClipChainId`.
- **`traversal.rs`** → calls → `build_viewport_clip`, `push_viewbox_frame`,
  `render_node`; `render_node` → `apply_node_transforms`
  (→ `transform::apply_transform_op` → `push_reference_frame`),
  `resolve_node_effects`, `emit_element` → `emit_geometry`/`emit_leaf` →
  `emit_shape`; `recurse_children` → `render_node` (recursion).
- **`effects/clip.rs`** imports [`shapes/mod.rs`](src/shapes/mod.rs) (for
  `ClipGeometry`) and [`render_tree.rs`](src/render_tree.rs); imported by
  [`traversal.rs`](src/traversal.rs) and [`effects/mod.rs`](src/effects/mod.rs).
- **`transform.rs`** imports [`style/transform_ops.rs`](src/style/transform_ops.rs)
  (for `TransformOp`) and [`style/mod.rs`](src/style/mod.rs).

### Call graph (clip/spatial only)

```
layout/svg/builder.rs
  └─ (imports) traversal.rs, shapes/mod.rs

traversal.rs
  ├─ render_svg_tree ─┬─ build_viewport_clip
  │                   ├─ push_viewbox_frame ── compute_viewbox_transform, transform::to_layout_transform
  │                   └─ render_node ─┬─ apply_node_transforms ── transform::apply_transform_op
  │                                  │                          └─ transform::compute_transform_scale
  │                                  │                          └─ transform::push_reference_frame
  │                                  ├─ resolve_node_effects ── effects::clip::resolve_node_clip_path
  │                                  │                       └─ effects::clip::build_mask_clips
  │                                  │                       └─ effects::filter::get_filter_ops
  │                                  ├─ emit_element ─┬─ emit_geometry ── push_filter_context, emit_shape
  │                                  │                └─ emit_leaf ────── push_filter_context, RenderContext
  │                                  ├─ recurse_children ── render_node  (recursion)
  │                                  └─ pop_reference_frame (×pushed_count)

effects/clip.rs
  └─ (imports) shapes/mod.rs (ClipGeometry), render_tree.rs, renderer::{ClipMaskProvider, clip_chain_option}
```

---

## 7. Source → ClipGeometry summary table

| Shape     | File                       | `clip_info` returns                                                | `all_equal_radius` used? |
| --------- | -------------------------- | ------------------------------------------------------------------ | ------------------------ |
| Circle    | `shapes/circle.rs`         | `RoundedRect`, corner r = circle radius                            | yes                      |
| Ellipse   | `shapes/ellipse.rs`        | `RoundedRect`, independent rx/ry radii                             | yes                      |
| Rectangle | `shapes/rectangle.rs`      | `RoundedRect`, radii clamped to half-dimension when both rx+ry set | yes                      |
| Path      | `shapes/path.rs`           | `RoundedRect` bounding box (zero radii) from segment endpoints     | no (zero radii)          |
| Polyline  | `shapes/polyline.rs`       | `RoundedRect` bounding box via shared `clip_points`                | no (zero radii)          |
| Polygon   | `shapes/polygon.rs`        | delegates to `clip_points` (bounding box)                          | no (zero radii)          |
| Line      | `shapes/line.rs`           | `None` — no clip geometry                                          | n/a                      |

---

## 8. `ClipGeometry` enum & `clip_info` dispatch

[`shapes/mod.rs`](src/shapes/mod.rs):

```rust
enum ClipGeometry {
    RoundedRect { bounds: LayoutRect, radii: BorderRadius },
    Polygon     { bounds: LayoutRect },
}

// Dispatch:
fn clip_info(shape: &Shape, origin, units) -> Option<ClipGeometry>
```

`define_clip_*` consumer side ([`effects/clip.rs:49-64`](src/effects/clip.rs#L49-L64)):

```rust
let clip_id = match geometry {
    ClipGeometry::RoundedRect { bounds, radii } => wr.define_clip_rounded_rect(
        spatial_id,
        ComplexClipRegion { rect: bounds, radii, mode: ClipMode::Clip },
    ),
    ClipGeometry::Polygon { bounds } => {
        // WebRender 0.69 has no arbitrary polygon clip → bounding-rect fallback
        wr.define_clip_rect(spatial_id, bounds)
    }
};
```

---

## 9. Full file inventory (svg_engine, clip/spatial-relevant)

| File                          | Role in clip/spatial system                                              |
| ----------------------------- | ----------------------------------------------------------------------- |
| `src/effects/clip.rs`         | **Defines all clips**: `resolve_node_clip_path`, `build_mask_clips`     |
| `src/renderer/transform.rs`   | **Defines all spatial ids**: `apply_transform_op`, `push_reference_frame`, `TransformResult` |
| `src/renderer/helpers.rs`     | `clip_chain_option` (ClipChainId → Option)                              |
| `src/traversal.rs`            | **Orchestrator**: threads spatial id + clip chain; `build_viewport_clip`, `push_viewbox_frame`, `render_node`, `resolve_node_effects`, `ResolvedEffects`, `EffectParams`, `RenderContext` construction |
| `src/shapes/mod.rs`           | `ClipGeometry` enum, `clip_info` dispatch, `all_equal_radius`           |
| `src/shapes/circle.rs`        | `clip_info` → rounded rect                                              |
| `src/shapes/ellipse.rs`       | `clip_info` → rounded rect (independent rx/ry)                          |
| `src/shapes/rectangle.rs`     | `clip_info` → rounded rect (clamped radii)                               |
| `src/shapes/path.rs`          | `clip_info` → bounding box                                              |
| `src/shapes/polyline.rs`      | `clip_points` → bounding box (shared with polygon)                      |
| `src/shapes/polygon.rs`       | delegates to `clip_points`                                               |
| `src/shapes/line.rs`          | no clip geometry                                                         |
| `src/style/node_effects.rs`   | `NodeEffects` struct holding `clip_path`/`mask`/`filter` URL references  |
| `src/render_tree.rs`          | `SvgRenderNode`, `ClipPathUnits`, resource provider interfaces          |
| `src/renderer/mod.rs`         | `RenderContext`, `ClipMaskProvider` trait, `Render` trait                |

---

## 10. Related (but separate) clip machinery in `components/layout/`

The knowledge graph also surfaced **unrelated** clip-id machinery in
`components/layout/` — this belongs to Servo's **HTML/CSS layout engine**,
not the svg_engine. It's a separate clip system and does **not** interact with
the svg_engine's `ClipChainId`/`SpatialId` (which come from `webrender_api`).

| Symbol                                | File                                              | Notes                                    |
| ------------------------------------- | ------------------------------------------------- | ---------------------------------------- |
| `ClipId`                              | `components/layout/display_list/clip.rs`           | Clip region identifier in stacking-context tree |
| `StackingContextTreeClipStore`        | `components/layout/display_list/clip.rs`          | Stores clip regions indexed by `ClipId`   |
| `hit_test_individual_clip_id`         | `components/layout/display_list/hit_test.rs`      | Point-in-clip test                        |
| `HitTest`                             | `components/layout/display_list/hit_test.rs`      | Hit-testing state (point, clip chain, result) |
| `generated_clip_id` / `set_generated_clip_id` | `components/layout/fragment_tree/box_fragment.rs` | `BoxFragment` layout clip-id methods      |

---

## 11. Open issues / caveats

- **FIXME** ([`transform.rs:68-69`](src/renderer/transform.rs#L68-L69)): SVG
  viewport clip breaks reference frames for skew/rotate/matrix; currently falls
  back to a `get_attr` path.
- **Polygon clip fallback** ([`clip.rs:58-63`](src/effects/clip.rs#L58-L63),
  [`clip.rs:115-118`](src/effects/clip.rs#L115-L118)): WebRender 0.69 has no
  native arbitrary-polygon clip; svg_engine falls back to a bounding-rect clip
  (safe but less precise).
- **Initial `SpatialId` / `ClipChainId`** originate in
  `components/layout/svg/builder.rs` (the external caller of `render_svg_tree`).
