# Stage 6 — Vector Image Rasterization

> **Thread:** Image Cache (thread pool — async)
> **Also known as:** usvg::Tree → tiny_skia pixmap → RasterImage
> **Key files:**
> - [components/net/image_cache.rs](../../components/net/image_cache.rs)
> - [components/layout/context.rs](../../components/layout/context.rs)

---

## Overview

Stage 6 converts the parsed `usvg::Tree` (from Stage 5) into a **rasterized pixel buffer** using `tiny_skia`. This is the actual SVG rendering step — the `resvg` library executes the SVG drawing commands and produces RGBA pixel data.

Rasterization runs **asynchronously** on the image cache's thread pool. The function returns `None` initially, and the result is stored in a cache for subsequent calls.

---

## Sub-stage 6.1 — Rasterization Request

**File:** [context.rs](../../components/layout/context.rs)
**Function:** `ImageResolver::rasterize_vector_image()`
**Lines:** 218-238

**Called from:** `make_fragments()` in Stage 8 when `vector_image.is_some()`

```rust
pub(crate) fn rasterize_vector_image(
    &self,
    image_id: PendingImageId,
    size: DeviceIntSize,
    node: OpaqueNode,
    svg_id: Option<String>,
) -> Option<RasterImage> {
    let result = self.image_cache.rasterize_vector_image(image_id, size, svg_id);
    if result.is_none() {
        self.pending_rasterization_images.lock().push(
            PendingRasterizationImage { id: image_id, node: node.into(), size }
        );
    }
    result
}
```

**Input:**
```rust
image_id: PendingImageId(1)
size: DeviceIntSize(200, 200)
svg_id: Some("9435b93e-ea8a-4323-996d-b048ab24a3ab")
```

**Output:** `None` on first call (async), `Some(RasterImage)` on subsequent calls (cached).

When `None` is returned, the pending rasterization is tracked for the script thread to re-request later.

---

## Sub-stage 6.2 — Image Cache Rasterization

**File:** [image_cache.rs](../../components/net/image_cache.rs)
**Function:** `rasterize_vector_image()`
**Lines:** 967-1067

### Step 1 — Look up the VectorImage

```rust
let mut store = self.store.lock();
let Some(vector_image) = store.vector_images.get(&image_id).cloned() else {
    warn!("Unknown image id {image_id:?} requested for rasterization");
    return None;
};
```

Looks up the `usvg::Tree` stored in Stage 5 by `PendingImageId`.

### Step 2 — Check for Cached Result

```rust
let entry = store.rasterized_vector_images
    .entry((image_id, requested_size))
    .or_default();
if let Some(result) = entry.result.as_ref() {
    return Some(result.clone());   // ← cached hit
}
```

On **first call**: cache miss, continue to rasterize.
On **subsequent calls**: cache hit, return the already-rasterized image immediately.

### Step 3 — Update ID Maps

```rust
if let Some(svg_id) = svg_id {
    if let Some(old_mapped_image_id) =
        self.svg_id_image_id_map.lock().insert(svg_id, image_id)
    {
        // Remove old mapping if this SVG ID was previously mapped elsewhere
        if old_mapped_image_id != image_id {
            store.vector_images.remove(&old_mapped_image_id);
            store.rasterized_vector_images.remove(&(old_mapped_image_id, requested_size));
        }
    }
}
```

Maps the SVG's UUID to the `PendingImageId` for cache management. If the same UUID was previously rendered at a different size, the old cache entries are cleaned up.

### Step 4 — Spawn Thread Pool Task (Async Rasterization)

```rust
let store = self.store.clone();
self.thread_pool.spawn(move || {
    let natural_size = vector_image.svg_tree.size().to_int_size();
    // natural_size = (200, 200)

    let tinyskia_requested_size = {
        let width = requested_size.width.try_into().unwrap_or(0)
            .min(MAX_SVG_PIXMAP_DIMENSION);    // safety clamp
        let height = requested_size.height.try_into().unwrap_or(0)
            .min(MAX_SVG_PIXMAP_DIMENSION);
        tiny_skia::IntSize::from_wh(width, height).unwrap_or(natural_size)
    };
    // tinyskia_requested_size = (200, 200)

    let transform = tiny_skia::Transform::from_scale(
        tinyskia_requested_size.width() as f32 / natural_size.width() as f32,
        tinyskia_requested_size.height() as f32 / natural_size.height() as f32,
    );
    // transform = scale(1.0, 1.0) since requested == natural

    let mut pixmap = tiny_skia::Pixmap::new(
        tinyskia_requested_size.width(),
        tinyskia_requested_size.height(),
    ).unwrap();

    resvg::render(&vector_image.svg_tree, transform, &mut pixmap.as_mut());
    // Renders the SVG into the pixmap buffer

    let bytes = pixmap.take();
    // 160000 bytes = 200 × 200 × 4 (RGBA)
```

The `resvg::render()` call is where the SVG is actually drawn — it processes all SVG elements (circles, rectangles, paths, text, etc.) and renders them into the pixmap using `tiny_skia`'s 2D graphics library.

### Step 5 — Build RasterImage

```rust
    let rasterized_image = RasterImage {
        metadata: ImageMetadata {
            width: tinyskia_requested_size.width(),    // 200
            height: tinyskia_requested_size.height(),   // 200
        },
        format: PixelFormat::RGBA8,
        frames: vec![frame],
        bytes: Arc::new(bytes),         // 160000 bytes
        id: None,                       // set when WebRender key is assigned
        cors_status: vector_image.cors_status,
        is_opaque: false,
    };

    let mut store = store.lock();
    store.load_image_with_keycache(PendingKey::Svg((
        image_id,
        rasterized_image,
        requested_size,
    )));
    // → triggers Stage 7 (set_key_and_finish_load)
});
```

**Output (async):** `None` — the function returns `None` immediately, and the thread pool task processes the SVG in the background.

**Final rasterized result:**
```rust
RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    bytes: Arc<[u8; 160000]>,    // 200 × 200 × 4 bytes RGBA
    id: None,                     // gets ImageKey in Stage 7
    is_opaque: false,             // SVG can have transparency
}
```

---

## Data Flow

```
make_fragments()
    │
    ▼
ImageResolver::rasterize_vector_image(id=1, size=200×200)
    │
    ▼
ImageCache::rasterize_vector_image(id=1, size=200×200)
    │
    ├── Already cached? → return Some(RasterImage) immediately
    │
    └── First call:
         │
     ┌───┴───┐
     │       │
     ▼       ▼
update ID maps    spawn thread pool task
svg_id→image_id   │
                  ▼
          usvg::Tree → tiny_skia
                  │
                  ▼
          Pixmap(200×200)
                  │
                  ▼
          resvg::render()
                  │
                  ▼
          RGBA pixels (160000 bytes)
                  │
                  ▼
          RasterImage { metadata: 200×200, bytes, id: None }
                  │
                  ▼
          load_image_with_keycache(Svg)
                  │
                  ▼
          Stage 7: set_key_and_finish_load()
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 6.1 | Resolution request | [context.rs:218](../../components/layout/context.rs#L218) | `image_id`, `size`, `svg_id` |
| 6.2-i | Entry | [image_cache.rs:967](../../components/net/image_cache.rs#L967) | `image_id`, `requested_size` |
| 6.2-ii | Cache check | [image_cache.rs:986](../../components/net/image_cache.rs#L986) | Cache hit vs miss |
| 6.2-iii | Thread pool spawn | [image_cache.rs:1035](../../components/net/image_cache.rs#L1035) | Starting async rasterization |
| 6.2-iv | Rasterization | [image_cache.rs:1059](../../components/net/image_cache.rs#L1059) | `resvg::render()` call |
| 6.2-v | Result stored | [image_cache.rs:1060](../../components/net/image_cache.rs#L1060) | `load_image_with_keycache` called |

### Trace Output

```
[SVG_TRACE_STAGE_6] rasterize_vector_image() ENTER image_id=PendingImageId(1) requested_size=200x200 svg_id=Some("...")
[SVG_TRACE_STAGE_6] rasterize_vector_image() found vector_image, usvg tree size=Size { width: 200.0, height: 200.0 }
[SVG_TRACE_STAGE_6] rasterize_vector_image() spawning thread pool task...
[SVG_TRACE_STAGE_6] rasterize_vector_image() returning None (async rasterization)
[SVG_TRACE_STAGE_6] rasterize_vector_image() spawned task: natural_size=200x200 requested=200x200
[SVG_TRACE_STAGE_6] rasterize_vector_image() rasterized 200x200 -> 160000 bytes
[SVG_TRACE_STAGE_6] rasterize_vector_image() spawned task: load_image_with_keycache done
[SVG_TRACE_STAGE_6] rasterize_vector_image() CACHED result, returning early   (subsequent calls)
```

### Key Variables

| Variable | Type | Meaning | Value |
|----------|------|---------|-------|
| `image_id` | `PendingImageId` | Which SVG to rasterize | `PendingImageId(1)` |
| `requested_size` | `DeviceIntSize` | Output pixel size | `200 × 200` |
| `natural_size` | `IntSize` | SVG's natural size | `200 × 200` |
| `tinyskia_requested_size` | `tiny_skia::IntSize` | Clamped render size | `200 × 200` |
| `pixmap` | `tiny_skia::Pixmap` | Pixel buffer | `200 × 200 × 4 = 160000 bytes` |
| `rasterized_image.bytes` | `Arc<[u8]>` | Raw RGBA pixel data | `160000 bytes` |
| `MAX_SVG_PIXMAP_DIMENSION` | const | Safety limit | Prevents OOM from huge viewBox values |

### Safety Clamping

The `MAX_SVG_PIXMAP_DIMENSION` constant (line 50) clamps the rasterization size to prevent memory exhaustion. Some SVG files use very large viewBox values that would otherwise cause `tiny_skia` to allocate a huge pixmap, potentially crashing the process:
```rust
// image_cache.rs:44-50
const MAX_SVG_PIXMAP_DIMENSION: i32 = 4096;
```
