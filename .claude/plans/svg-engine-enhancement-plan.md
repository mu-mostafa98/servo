# SVG Engine Enhancement Plan

**Context:** The SVG engine (`components/svg_engine/`) is a custom rendering subsystem that converts SVG DOM elements into WebRender display list commands. It implements 7 basic shapes with a clean trait-based architecture (`FromAttributes` + `Render`), a style system with fill/stroke/transform, polygon tessellation via `lyon`, and a recursive tree walker. However, the codebase was built incrementally and has several quality gaps:

- **No tests** — zero unit or integration tests across all 27 source files
- **No error types** — all fallible operations return `Option`, silently dropping failures
- **SOLID violations** — `extract.rs` mixes tag dispatch, CSS parsing, and color parsing; `Shape` enum and `Render` trait require modifying 3 files to add one shape; `Render` trait has 6+ parameters
- **Rust standards gaps** — no newtype pattern for SVG lengths/coordinates, no `MallocSizeOf`, no doc comments on most public items, no use of `Copy`/`Default` where applicable
- **Servo integration gaps** — memory not tracked (`#[ignore_malloc_size_of]` in fragment.rs), no hit testing, no graceful fallback when feature-gated
- **SVG spec gaps** — paint servers (gradients/patterns) silently resolve to None, `<text>`/`<use>`/`<defs>`/`<clipPath>`/`<mask>`/`<filter>` not supported, `skewX`/`skewY`/`matrix` transforms missing, no `preserveAspectRatio`

This plan addresses all of these in incremental phases.

---

## Phase 1 — Unified `Build` Trait Refactoring

**Goal:** Replace the fragmented extraction system (`FromAttributes`, `FromComputedValues`, `FromCssAttrs` + free functions) with a single uniform `Build` trait (Factory Method pattern). The caller passes one `SvgBuildInput` struct and gets back a fully-constructed `SvgRenderNode` with `SvgTag`, `NodeStyle`, and transforms all resolved internally.

**Design decisions (agreed in discussion):**
- **Full unification** — one `Build` trait everywhere, `FromAttributes` is removed entirely
- **`Result` return** — `build()` returns `Result<Self>` with error types, mapped to `Option` at the public API boundary
- **Transforms internal to `NodeStyle`** — `NodeStyle::build()` reads `get_attr("transform")`, no separate merge step for caller
- **No `css_style` in input** — `NodeStyle` internally reads `get_attr("style")` as fallback
- **Children recursion stays in `replaced.rs`** — keeps `svg_engine` DOM-agnostic
- **`extract_viewbox` stays in `render_tree.rs`** — `SvgRenderTree::build()` is a later addition

**Priority:** Must-have | **Effort:** Medium | **Risk:** Low

### Changes

1. **Create `error.rs`** — crate-level error type following Servo's hand-rolled enum pattern:
   ```rust
   pub enum SvgEngineError { MissingAttribute(String), ParseError(String), UnsupportedFeature(String) }
   pub type SvgResult<T> = std::result::Result<T, SvgEngineError>;
   ```
   - File: New `svg_engine/src/error.rs`

2. **Rewrite `extract.rs`** — add `Build` trait + `SvgBuildInput` struct + impls for `SvgRenderNode`, `SvgTag`, `NodeStyle`:
   ```rust
   pub trait Build: Sized {
       fn build(input: &SvgBuildInput) -> SvgResult<Self>;
   }
   pub struct SvgBuildInput<'a> {
       pub element_name: &'a str,
       pub get_attr: &'a dyn Fn(&str) -> Option<String>,
       pub computed_values: Option<&'a ComputedValues>,
   }
   ```
   - `impl Build for SvgRenderNode` — composite: calls `SvgTag::build()` + `NodeStyle::build()`, returns node with id=None and empty children (set by caller)
   - `impl Build for SvgTag` — dispatches: `"svg"`/`"g"` → Container, else `Shape::build()` → Shape
   - `impl Build for NodeStyle` — prefers `ComputedValues`, falls back to `get_attr("style")`, extracts transforms internally
   - Legacy wrappers (`extract_tag`, `extract_node_style`, `extract_node_style_from_css`) remain as thin public wrappers
   - Internal helpers (`resolve_svg_paint`, `parse_css_color`) unchanged
   - File: `svg_engine/src/extract.rs`

3. **Update `shapes/mod.rs`**:
   - Remove `FromAttributes` trait entirely
   - Add `impl Build for Shape` — dispatches by `element_name` to individual shapes
   - `parse_length()` → returns `SvgResult<f32>` (was `Option<f32>`)
   - `parse_points()` → returns `SvgResult<Vec<Point>>`
   - `parse_path()` → returns `SvgResult<BezPath>`
   - File: `svg_engine/src/shapes/mod.rs`

4. **Update all 7 shape files** — replace `impl FromAttributes` with `impl Build`:
   - `rectangle.rs`, `circle.rs`, `ellipse.rs`, `line.rs`, `polyline.rs`, `polygon.rs`, `path.rs`
   - Each uses `parse_length`/`parse_points`/`parse_path` with `?` for required attrs and `.ok()` for optional attrs

5. **Update `style/transform.rs`**:
   - Add `impl Build for Vec<TransformOp>` — reads `get_attr("transform")`, delegates to existing `extract_transforms`
   - Make `extract_transforms` visibility `pub(crate)` (was `pub`)
   - File: `svg_engine/src/style/transform.rs`

6. **Update `lib.rs`** — add `pub mod error;`, re-export `Build` + `SvgBuildInput`
   - File: `svg_engine/src/lib.rs`

7. **Simplify `replaced.rs`** — `build_svg_render_node()` uses `SvgRenderNode::build(&input)` instead of 3 separate calls:
   ```rust
   let input = SvgBuildInput {
       element_name: name,
       get_attr: &get_attr,
       computed_values: element.style_data()
           .map(|_| element.style(&context.style_context)),
   };
   let mut node = SvgRenderNode::build(&input).ok()?;
   node.id = ...; // set externally
   node.children = ...; // recurse externally
   ```
   - File: `components/layout/replaced.rs`

### Verification
`cargo build -p svg_engine` + `cargo build -p layout` succeed. `SvgRenderNode::build()` is a single call in `replaced.rs`. No `FromAttributes` references, no manual transform merge.
   - File: `svg_engine/src/style/transform.rs`

6. **Update `lib.rs`** — add `pub mod error;` and import `Extract` trait where needed
   - File: `svg_engine/src/lib.rs`

7. **Simplify `replaced.rs`** — `build_svg_render_node()` uses `SvgRenderNode::extract(&input)` instead of 3 separate calls:
   ```rust
   let input = SvgExtractInput {
       element_name: name,
       get_attr: &get_attr,
       computed_values: element.style_data()
           .map(|_| element.style(&context.style_context)),
   };
   let mut node = SvgRenderNode::extract(&input).ok()?;
   node.id = ...; // set externally
   node.children = ...; // recurse externally
   ```
   - File: `components/layout/replaced.rs`

### Verification
`cargo build -p svg_engine` + `cargo build -p layout` succeed. `SvgRenderNode::extract()` is a single call in `replaced.rs`. No `FromAttributes`, no manual transform merge.

---

## Phase 2 — Testing Infrastructure and Error Handling

**Goal:** Refactor to respect SOLID principles — reduce coupling, make the system extensible, clean up the delegation chain.

**Priority:** Must-have | **Effort:** Medium | **Risk:** Medium

### Changes

1. **Split `extract.rs` into focused modules** (Single Responsibility):
   - `extract.rs` retains only `extract_tag` and `extract_node_style`
   - New `color.rs` — `resolve_svg_paint`, `parse_css_color`
   - New `css_parser.rs` — `FromCssAttrs for NodeStyle`
   - Files: `extract.rs` (shrink), new `color.rs`, new `css_parser.rs`

2. **Introduce `RenderContext` struct** (Interface Segregation):
   ```rust
   pub(crate) struct RenderContext<'a> {
       pub style: &'a NodeStyle,
       pub svg_origin: &'a LayoutPoint,
       pub spatial_id: SpatialId,
       pub clip_chain_id: ClipChainId,
       pub wr: &'a mut DisplayListBuilder,
   }
   ```
   - Change `Render` trait: `fn render(&self, ctx: &mut RenderContext)` — reduces params from 6→1
   - Update all 7 renderer implementations
   - Files: `renderer/mod.rs`, all `renderer/*.rs`

3. **Add `ShapeKind` trait** to formalize shape metadata (Open/Closed):
   ```rust
   pub(crate) trait ShapeKind {
       fn is_closed() -> bool;
   }
   ```
   - `Polygon` → `true`, `Polyline` → `false`, other shapes return `true` for fill-capable
   - This removes invisible delegation behavior (polygon appending first point) from renderers
   - Files: `shapes/mod.rs`, `renderer/polygon.rs`, `renderer/polyline.rs`

4. **Remove circle→ellipse→rect and polygon→polyline delegation** (Liskov Substitution):
   - Circle and ellipse both delegate to a shared `render_ellipse()` function in `renderer/mod.rs`
   - Polygon calls `tessellator` directly and iterates segments for stroke with a closed flag
   - Files: `renderer/circle.rs`, `renderer/ellipse.rs`, `renderer/polygon.rs`, `renderer/mod.rs`

5. **Decouple traversal from WebRender** via `DisplayListSink` trait (Dependency Inversion):
   ```rust
   pub(crate) trait DisplayListSink {
       fn push_rect(...);
       fn push_border(...);
       fn push_reference_frame(...) -> SpatialId;
       fn pop_reference_frame(&mut self);
       fn define_clip_rect(...);
       fn define_clip_chain(...);
   }
   ```
   - Implement via newtype wrapper on `DisplayListBuilder`
   - Files: `traversal.rs`, `renderer/mod.rs`, new `display_list_sink.rs`

### Verification
`cargo test -p svg_engine` passes; `cargo build -p svg_engine` with no warnings; sample SVG renders identically before and after.

---

## Phase 3 — Servo Integration Standards

**Goal:** Bring svg_engine up to Servo's codebase conventions for memory tracking, feature gates, and integration patterns.

**Priority:** Should-have | **Effort:** Medium | **Risk:** Low

### Changes

1. **Add `MallocSizeOf` derives** on all SVG types:
   - `SvgRenderTree`, `SvgRenderNode` (use `#[ignore_malloc_size_of]` for `BezPath` from kurbo)
   - `Shape` + all 7 shape structs
   - `NodeStyle`, `FillParams`, `StrokeParams`, `TransformOp`, `RenderHints`, `NodeEffects`
   - Add `malloc_size_of_derive = { workspace = true }` to `Cargo.toml`
   - Files: `Cargo.toml`, `render_tree.rs`, `shapes/*.rs`, `style/*.rs`

2. **Add render tree caching** — skip rebuild when DOM subtree hasn't changed:
   - Add a generation counter/check on `SvgRenderTree`
   - At minimum, add a FIXME documenting this as future optimization
   - Files: `render_tree.rs`, `traversal.rs`, `components/layout/replaced.rs`

3. **Add graceful fallback logging** — `tracing::warn!` when old serialization path is taken due to disabled pref
   - Files: `components/layout/replaced.rs`

4. **Hit testing stubs** — `contains_point(x, y) -> bool` method on each shape, wired into traversal
   - Files: `shapes/*.rs`, `renderer/*.rs`, `traversal.rs`

### Verification
`cargo build -p svg_engine` succeeds with `MallocSizeOf` derives; `cargo test -p svg_engine` passes.

---

## Phase 4 — SVG Specification Core Gaps

**Goal:** Address the most impactful SVG specification gaps: `preserveAspectRatio`, expanded transforms, and gradient paint servers.

**Priority:** Should-have | **Effort:** High | **Risk:** Medium

### Changes

1. **Implement `preserveAspectRatio`** — all 9 alignment combos + `none`:
   - Add `PreserveAspectRatio` struct with `align` and `meet_or_slice` fields
   - Modify `render_svg_tree` in `traversal.rs` to compute alignment transform
   - Files: `render_tree.rs`, `traversal.rs`

2. **Add `skewX`, `skewY`, `matrix` transform functions**:
   - Extend `TransformOp` enum with `SkewX(f32)`, `SkewY(f32)`, `Matrix([f32; 6])`
   - Implement `apply_transform_op` for new variants
   - Files: `style/transform.rs`

3. **Implement basic linear gradient support**:
   - Add `PaintServer` enum: `Color(ColorF)` | `Gradient(GradientDef)`
   - Modify `FillParams.color` to use `PaintServer` instead of `Option<ColorF>` for backward compat
   - Simple 2-stop gradient via direct pixel fill in tessellator
   - Files: `style/fill.rs`, `extract.rs`, `tessellator.rs`

### Verification
Test SVGs with `preserveAspectRatio`, skewX, and gradients render correctly.

---

## Phase 5 — Text and Structural Elements

**Goal:** Add support for `<text>`, `<use>`, and `<defs>` elements.

**Priority:** Nice-to-have | **Effort:** Very High | **Risk:** High

### Changes

1. **Implement `<text>` and `<tspan>`** — single-line, left-aligned initially:
   - New `shapes/text.rs` with `Text { x, y, content, font_size, font_family, text_anchor }`
   - New `renderer/text.rs` using WebRender's `push_text` with Servo font integration
   - Files: new `shapes/text.rs`, new `renderer/text.rs`, `shapes/mod.rs`, `renderer/mod.rs`

2. **Implement `<use>`** — `#id` reference resolution:
   - Add `SvgTag::Use(UseRef)` with `href`, `x`, `y`
   - Add ID-to-node lookup map on `SvgRenderTree`
   - Files: `render_tree.rs`, `traversal.rs`, `extract.rs`

3. **Implement `<defs>`** — skip direct rendering, populate definition table:
   - Add `Container::Defs` variant
   - Add `defs: HashMap<String, SvgRenderNode>` to `SvgRenderTree`
   - Files: `render_tree.rs`, `traversal.rs`, `extract.rs`

### Verification
SVG with `<text>Hello</text>`, `<use href="#r1"/>`, and `<defs>` renders correctly.

---

## Phase 6 — Newtype Pattern and Type-Level Validation

**Goal:** Replace raw `f32` with newtype wrappers encoding SVG constraints at the type level.

**Priority:** Nice-to-have | **Effort:** Medium | **Risk:** Low

### Changes

1. **Define newtype wrappers** in a new `types.rs`:
   ```rust
   pub struct SvgLength(f32);       // Non-negative for width/height
   pub struct SvgCoordinate(f32);   // Can be negative (x, y, cx, cy)
   pub struct SvgOpacity(f32);      // 0.0..=1.0
   pub struct NonNegative(f32);     // width, height, r, rx, ry, stroke-width
   pub struct AngleDeg(f32);        // rotation angle
   ```
   - Each implements `TryFrom<f32>` with validation
   - Files: new `types.rs`, `shapes/mod.rs`

2. **Update shape structs** to use newtypes (compile-time validation):
   - `Rectangle { width: NonNegative, height: NonNegative, ... }`
   - `Circle { r: NonNegative, ... }`, `Ellipse { rx: NonNegative, ry: NonNegative, ... }`
   - `StrokeParams.width: NonNegative`, `StrokeParams.opacity: SvgOpacity`
   - Files: All `shapes/*.rs`, `style/stroke.rs`, `style/fill.rs`, `render_tree.rs`

3. **Update `parse_length`** to return typed newtypes instead of raw `Option<f32>`
   - Files: `shapes/mod.rs`

### Verification
`cargo build -p svg_engine` succeeds; compile errors appear if negative values are assigned to width.

---

## Phase 7 — Performance and Benchmarking

**Goal:** Add benchmark infrastructure and performance optimizations.

**Priority:** Nice-to-have | **Effort:** Medium | **Risk:** Low

### Changes

1. **Add Criterion benchmark targets:**
   - `benches/tessellation.rs` — 3-point, 100-point, 1000-point polygons
   - `benches/parsing.rs` — transform parsing, point parsing
   - Files: new `benches/tessellation.rs`, new `benches/parsing.rs`, `Cargo.toml`

2. **Optimize tessellator** — merge consecutive scanlines into taller rectangles
   - Investigate vertical coherence to reduce `push_rect` calls
   - Files: `tessellator.rs`

3. **Pre-size `children` Vec** in `SvgRenderNode` after counting DOM children
   - Files: `traversal.rs` or `components/layout/replaced.rs`

4. **Add `tracing` spans** at key points (`extract_tag`, `tessellate`, `render_node`)
   - Files: `extract.rs`, `tessellator.rs`, `traversal.rs`

### Verification
`cargo bench -p svg_engine` produces baseline numbers; render output is pixel-identical.

---

## Summary

| Phase | Priority | Effort | Risk | Key Outcome |
|-------|----------|--------|------|-------------|
| 0 — Docs & Audit | Must-have | Low | None | All public API documented, TODOs visible |
| 1 — Tests & Errors | Must-have | Medium | Low | 30+ tests, proper error types |
| 2 — SOLID Refactoring | Must-have | Medium | Medium | Open/closed via ShapeKind, RenderContext, DisplayListSink |
| 3 — Servo Integration | Should-have | Medium | Low | MallocSizeOf, hit testing stubs, caching FIXME |
| 4 — SVG Spec Core | Should-have | High | Medium | preserveAspectRatio, skewX/Y, basic gradients |
| 5 — Text & Struct | Nice-to-have | Very High | High | `<text>`, `<use>`, `<defs>` |
| 6 — Newtype System | Nice-to-have | Medium | Low | Compile-time SVG constraint enforcement |
| 7 — Performance | Nice-to-have | Medium | Low | Benchmarks, scanline merging, tracing |

### Critical Files to Modify

| File | Role | When |
|------|------|------|
| `svg_engine/src/lib.rs` | Crate root, public API surface | Phase 0 |
| `svg_engine/src/extract.rs` | Heaviest coupling — tag dispatch + CSS + colors | Phase 0, 1, 2 |
| `svg_engine/src/shapes/mod.rs` | Shape enum + FromAttributes + parsing helpers | Phase 1, 2, 6 |
| `svg_engine/src/renderer/mod.rs` | Render trait dispatch | Phase 2, 3 |
| `svg_engine/src/traversal.rs` | Tree walker + WebRender integration | Phase 2, 3, 4 |
| `svg_engine/src/render_tree.rs` | Tree types + viewport | Phase 1, 3, 4 |
| `svg_engine/src/style/transform.rs` | Transform types + parsing | Phase 0, 1, 4 |
| `svg_engine/src/tessellator.rs` | Polygon fill rasterization | Phase 1, 4, 7 |
| `components/layout/replaced.rs` | Render tree construction from DOM | Phase 3 |
| `components/layout/fragment_tree/fragment.rs` | `#[ignore_malloc_size_of]` removal | Phase 3 |

### Verification Approach
- Each phase: `cargo build -p svg_engine` + `cargo test -p svg_engine`
- After Phases 2-5: `servoshell --svg-engine <test-svg.html>` for visual regression
- After Phase 7: `cargo bench -p svg_engine` for baseline + comparison
