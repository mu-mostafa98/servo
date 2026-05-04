# SVG Pipeline Debug Guide

## Setup

### 1. Required VS Code Extensions

Install these extensions in VS Code:

| Extension | ID | Purpose |
|-----------|----------|---------|
| **CodeLLDB** | `vadimcn.vscode-lldb` | Rust debugger (step through, breakpoints, variable inspection) |
| **rust-analyzer** | `rust-lang.rust-analyzer` | Rust language support, code navigation |

### 2. Build Servo for Debugging

```bash
# From the repo root (d:\Projects\servo)
cargo build -p servoshell
```

This produces `target/debug/servoshell.exe`.

> **Note:** The first build takes a while. Subsequent builds are incremental.

### 3. How to Debug

**Option A — VS Code (recommended):**

1. Open the `d:\Projects\servo` folder in VS Code
2. Open [simple_svg.html](svg_tests/simple_svg.html)
3. Set breakpoints (see pipeline stages below)
4. Press `F5` or go to Run → "Debug Servo (simple SVG)"

**Option B — Terminal with logging:**

```bash
# Run with verbose logging
RUST_LOG=debug ./target/debug/servoshell.exe svg_tests/simple_svg.html -d -o
```

The `-d` flag prints diagnostic output. `RUST_LOG=debug` enables detailed logging across all components.

---

## Pipeline Stages — Breakpoints & Variables to Watch

The SVG pipeline has 10 stages. Stages marked with ⚠ are SVG-specific (the main debugging targets).

### Stage 1 — DOM Construction

**File:** [components/script/dom/create.rs](../components/script/dom/create.rs)
**Function:** `create_svg_element`
**Line:** 96
**Breakpoint:** Line 114 (`local_name!("svg") => make!(SVGSVGElement)`)
**What to watch:**
- `name.local` — should be `"svg"`
- Confirm `SVGSVGElement::new` is called (not `SVGElement` generic fallback)

**Result:** `SVGSVGElement` DOM node is created and inserted into the DOM tree.

---

### Stage 2 — Style Resolution & Dispatch

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Function:** `svg_kind_size`
**Line:** 221
**Breakpoint:** Line 271 (`match svg_data.source`)
**What to watch:**
- `svg_data.source` — should be `None` on **first pass** (the SVG hasn't been serialized yet)
- `svg_data.width` / `svg_data.height` — the CSS-resolved dimensions
- `svg_data.svg_id` — unique UUID for this SVG element
- `natural_size` — the computed natural width/height

**File:** [components/shared/layout/lib.rs](../components/shared/layout/lib.rs)
**Struct:** `SVGElementData` at line 152

**Result:** `ReplacedContentKind::SVGElement(None)` — first layout pass with no source.

---

### Stage 3 ⚠ — DOM Serialization Request

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Breakpoint:** Line 277 — `queue_svg_element_for_serialization(node)` is called.
**Watch:** The function is called when `svg_data.source` is `None`.

**File:** [components/layout/context.rs](../components/layout/context.rs)
**Function:** `queue_svg_element_for_serialization`
**Line:** 240
**Breakpoint:** Line 241-243 — watch the node being queued.

**File:** [components/script/dom/window.rs](../components/script/dom/window.rs)
**Function:** `handle_pending_images_post_reflow`
**Line:** 3523-3588
**Breakpoint:** Line 3583 — SVG serialization triggered.
**What to watch:**
- `pending_svg_element_for_serialization` — the list of SVGs that need serialization
- Line 3586: `svg.serialize_and_cache_subtree()`

**Result:** SVG node is queued for serialization. Layout pass 1 ends.

---

### Stage 4 ⚠ — Serialized Source Injection

**File:** [components/script/dom/svg/svgsvgelement.rs](../components/script/dom/svg/svgsvgelement.rs)
**Function:** `serialize_and_cache_subtree`
**Line:** 79
**Breakpoint:** Line 79 (function entry)
**What to watch:**
- Line 85: `xml_serialize` — the DOM subtree is serialized to XML string
- Line 97: `base64_encoded_source` — the XML becomes base64
- Line 98: `data_url` — final data URL like `data:image/svg+xml;base64,...`
- Line 99-100: the result is stored in `cached_serialized_data_url`

**File:** [components/script/dom/window.rs](../components/script/dom/window.rs)
**Line:** 3587 — `node.dirty(NodeDamage::Other)` triggers a second reflow.

**Result:** `cached_serialized_data_url = Some(Ok("data:image/svg+xml;base64,..."))`. Second layout pass triggered.

**Second layout pass** — back to [layout/replaced.rs](../components/layout/replaced.rs):
- **Breakpoint:** Line 271 again
- Now `svg_data.source` should be `Some(Ok(data_url))`
- Line 286: `get_cached_image_for_url()` — the image cache processes the data URL

---

### Stage 5 — SVG Tree Parsing

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `complete_load` (handles LoadResult::LoadedVectorImage)
**Line:** 597
**Breakpoint:** Line 609-622 — when `LoadResult::LoadedVectorImage` is processed.
**What to watch:**
- `vector_image.svg_tree` — the parsed `usvg::Tree` object
- `natural_dimensions` — parsed from the tree size

**Result:** `VectorImage` stored in `store.vector_images`. Ready for rasterization.

---

### Stage 6 ⚠ — CPU Rasterization

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `rasterize_vector_image`
**Line:** 967
**Breakpoint:** Line 967 (function entry) and Line 1011 (`self.thread_pool.spawn`)
**What to watch:**
- `image_id` — the pending image ID
- `requested_size` — the pixel dimensions requested
- Line 974: `vector_image` fetched from `store.vector_images`
- Line 1012: `natural_size` from `vector_image.svg_tree.size()`
- Line 1030: `tiny_skia::Pixmap::new(...)` — the pixel buffer allocated
- **Line 1035: `resvg::render(&vector_image.svg_tree, transform, &mut pixmap.as_mut())`** — the actual rasterization call
- Line 1037: `pixmap.take()` — raw RGBA pixel bytes extracted

**File:** [components/layout/context.rs](../components/layout/context.rs)
**Function:** `rasterize_vector_image`
**Line:** 218
**Breakpoint:** Line 225-237
**Watch:** `result` — `None` means rasterization is async (spawned on thread pool)

**Result:** `RasterImage` with raw pixel bytes, sent back via `load_image_with_keycache`.

---

### Stage 7 ⚠ — Image Key Delivery

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `set_key_and_finish_load`
**Line:** 484
**Breakpoint:** Line 490 (`PendingKey::Svg(...)`)
**What to watch:**
- `image_key` — the `WebRenderImageKey` assigned
- `raster_image` — the rasterized result (bytes, metadata, format)

**Function:** `load_image_with_keycache`
**Line:** 499
**Breakpoint:** Line 499 — entry point when SVG rasterization finishes.

**Function:** `complete_load_svg`
**Line:** 569
**Breakpoint:** Line 569-593
**Watch:** The callback notifies layout with `VectorImageRasterizationComplete`.

**Result:** `ImageFragment.image_key = Some(ImageKey(N))` — GPU handle ready.

---

### Stage 8 — Fragment Tree Integration

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Function:** `make_fragments`
**Line:** 474
**Breakpoint:** Line 566-605 (the `ReplacedContentKind::SVGElement(vector_image)` arm)
**What to watch:**
- `vector_image.id` — should match what was rasterized
- `raster_size` — the device-pixel-scaled size for rasterization
- `image_key` — the `WebRenderImageKey` returned from `rasterize_vector_image`
- Line 603: `Fragment::Image(ArcRefCell::new(ImageFragment { ... }))` — the final fragment

**File:** [components/layout/fragment_tree/fragment.rs](../components/layout/fragment_tree/fragment.rs)
**Struct:** `ImageFragment` at line 86
**Enum:** `Fragment::Image(...)` at line 49

**Result:** `Fragment::Image(RefCell { image_key: Some(Key(N)), ... })` in the box tree.

---

### Stage 9 — Display List Emission

**File:** [components/layout/display_list/mod.rs](../components/layout/display_list/mod.rs)
**Breakpoint:** Line 680-709 (the `Fragment::Image(image)` arm)
**What to watch:**
- `image.image_key` — the `WebRenderImageKey`
- `image.base.rect` — the rectangle on screen
- Line 699: `builder.wr().push_image(...)` — the WebRender display command emitted

**Result:** `WebRenderCmd::DrawImage(ImageKey(N), transform, clip)` — sent to WebRender.

---

### Stage 10 — GPU Compositing

No Rust breakpoints for this stage — it's handled by WebRender's GPU pipeline (compiled shader code).

**What you can verify:**
- The window shows a blue circle on a white background.
- You can check WebRender's debug flags if enabled (extra setup needed).

---

## Quick Reference: Key Files by Stage

| Stage | File | Key Function | Line |
|-------|------|-------------|------|
| 1 | [script/dom/create.rs](../components/script/dom/create.rs) | `create_svg_element` | 114 |
| 2 | [layout/replaced.rs](../components/layout/replaced.rs) | `svg_kind_size` | 221 |
| 2 | [shared/layout/lib.rs](../components/shared/layout/lib.rs) | `SVGElementData` | 152 |
| 3 | [layout/replaced.rs](../components/layout/replaced.rs) | (match on `svg_data.source`) | 271 |
| 3 | [layout/context.rs](../components/layout/context.rs) | `queue_svg_element_for_serialization` | 240 |
| 3 | [script/dom/window.rs](../components/script/dom/window.rs) | `handle_pending_images_post_reflow` | 3523 |
| 4 | [script/dom/svg/svgsvgelement.rs](../components/script/dom/svg/svgsvgelement.rs) | `serialize_and_cache_subtree` | 79 |
| 5 | [net/image_cache.rs](../components/net/image_cache.rs) | `complete_load` | 597 |
| 6 | [net/image_cache.rs](../components/net/image_cache.rs) | `rasterize_vector_image` | 967 |
| 6 | [layout/context.rs](../components/layout/context.rs) | `rasterize_vector_image` | 218 |
| 7 | [net/image_cache.rs](../components/net/image_cache.rs) | `set_key_and_finish_load` | 484 |
| 7 | [net/image_cache.rs](../components/net/image_cache.rs) | `complete_load_svg` | 569 |
| 8 | [layout/replaced.rs](../components/layout/replaced.rs) | `make_fragments` | 474 |
| 9 | [layout/display_list/mod.rs](../components/layout/display_list/mod.rs) | (Fragment::Image handling) | 680 |

## Common Debugging Tips

### Thread Identification
Servo is multi-threaded. When a breakpoint hits, identify which thread:

| Thread | Role |
|--------|------|
| `Script` / `ScriptThread` | DOM, JS, style resolution |
| `Layout` | Fragment tree, display list |
| `Net` / `ImageCache` | Image loading, SVG parsing, rasterization |
| `WRRender` / `WebRender` | GPU rendering |

In CodeLLDB, the **Call Stack / Threads** pane shows active threads. Stages 1-2 run on Script. Stage 3-4 span Layout→Script. Stage 5-7 run on Net/ImageCache thread pool. Stages 8-10 run on Layout/WebRender.

### Key Variables Template
When debugging any stage, always check these three things first:
1. **What is the INPUT?** (thread handoff data)
2. **Is the STATE correct?** (e.g., `source: None` vs `Some(...)`)
3. **What is the OUTPUT?** (did the expected value get produced?)

### Restarting Debug Sessions
After making code changes, rebuild:
```bash
cargo build -p servoshell
```
Then press `F5` again in VS Code.

### SVG Test File
The test file is at [svg_tests/simple_svg.html](svg_tests/simple_svg.html). It contains:
```html
<svg width="200" height="200" viewBox="0 0 200 200">
    <circle cx="100" cy="100" r="50" fill="blue" />
</svg>
```

This is intentionally minimal — no `<use>`, no external resources, no fonts, no CSS transforms.
