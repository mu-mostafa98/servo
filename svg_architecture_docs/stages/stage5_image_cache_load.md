# Stage 5 — Image Cache Load (SVG Parsing)

> **Thread:** Network / Image Cache (async)
> **Also known as:** SVG data URL → usvg::Tree → VectorImage
> **Key files:**
> - [components/net/image_cache.rs](../../components/net/image_cache.rs)
> - [components/layout/context.rs](../../components/layout/context.rs)

---

## Overview

Stage 5 is where the serialized SVG data URL is **parsed into a `usvg::Tree`** by the `resvg` library. This happens asynchronously — the image cache receives the data URL, creates a load request, and when the response arrives, it parses the SVG and stores the resulting `VectorImage`.

This stage bridges the gap between:
- **Stage 4** (which produces the data URL)
- **Stage 6** (which rasterizes the parsed SVG tree)

---

## Sub-stage 5.1 — Image Cache Request

**File:** [context.rs](../../components/layout/context.rs)
**Function:** `ImageResolver::get_or_request_image_or_meta()`
**Lines:** 127-170

When `svg_kind_size()` calls `get_cached_image_for_url()` with the data URL, it first checks the image cache:

```rust
let cache_result = self.image_cache
    .get_cached_image_status(url.clone(), self.origin.clone(), None);
```

**Possible results:**

| Result | Meaning | Action |
|--------|---------|--------|
| `Available(img)` | Already parsed | Return the image immediately |
| `Pending(id)` | Request in progress | Add to pending_images list, return Pending |
| `ReadyForRequest(id)` | Not yet requested | Add to pending_images as Unrequested, the image cache will start loading |
| `FailedToLoad` | Previous load failed | Return LoadError |

For a data URL on the first access, the result is `ReadyForRequest` — the image cache hasn't seen this URL before. The pending image is queued for the script thread to start the actual load.

---

## Sub-stage 5.2 — Complete Load (VectorImage)

**File:** [image_cache.rs](../../components/net/image_cache.rs)
**Function:** `complete_load()`
**Lines:** 597-641

When the SVG data URL is loaded (which is effectively instant for `data:` URLs), `complete_load()` is called:

```rust
fn complete_load(&mut self, key: LoadKey, load_result: LoadResult) {
    let pending_load = match self.pending_loads.remove(&key) {
        Some(load) => load,
        None => return,     // already processed
    };
    let url = pending_load.final_url.clone();

    let image_response = match load_result {
        LoadResult::LoadedVectorImage(vector_image) => {
            // Store the parsed SVG tree
            self.vector_images.insert(key, vector_image.clone());

            // Extract natural dimensions from the usvg::Tree
            let natural_dimensions = vector_image.svg_tree.size().to_int_size();

            let vector_image = VectorImage {
                id: key,                            // PendingImageId
                svg_id: None,                       // tagged later in svg_kind_size
                metadata: ImageMetadata {
                    width: natural_dimensions.width(),   // 200 for our test
                    height: natural_dimensions.height(), // 200 for our test
                },
                cors_status: vector_image.cors_status,
            };
            ImageResponse::Loaded(Image::Vector(vector_image), url.unwrap())
        },
        LoadResult::LoadedRasterImage(_) => { /* not our path */ },
        LoadResult::FailedToLoadOrDecode => ImageResponse::FailedToLoadOrDecode,
    };

    // Store in completed loads and notify listeners
    self.completed_loads.insert((...), completed_load);
    for listener in pending_load.listeners {
        listener.respond(image_response.clone());
    }
}
```

**Input:**
```rust
key = PendingImageId(1)
load_result = LoadResult::LoadedVectorImage(VectorImage {
    svg_tree: usvg::Tree { size: Size(200.0, 200.0), ... },
    cors_status: CorsStatus::Uncached,
})
```

**Output:**
```rust
VectorImage {
    id: PendingImageId(1),
    svg_id: None,
    metadata: ImageMetadata { width: 200, height: 200 },
    cors_status: CorsStatus::Uncached,
}
```

The `usvg::Tree` is stored in `self.vector_images` keyed by `PendingImageId(1)`. The `VectorImage` metadata struct (without the tree) is sent to listeners.

---

## Sub-stage 5.3 — Complete Load SVG (Rasterization Done)

**File:** [image_cache.rs](../../components/net/image_cache.rs)
**Function:** `complete_load_svg()`
**Lines:** 569-594

After Stage 7 assigns a WebRender key and the rasterized image is ready, this function notifies the pipeline:

```rust
fn complete_load_svg(&mut self, rasterized_image: RasterImage,
                      pending_image_id: PendingImageId,
                      requested_size: DeviceIntSize) {
    let listeners = self.rasterized_vector_images
        .get_mut(&(pending_image_id, requested_size))
        .map(|task| {
            task.result = Some(rasterized_image);
            std::mem::take(&mut task.listeners)
        })
        .unwrap_or_default();

    for (pipeline_id, callback) in listeners {
        callback(ImageCacheResponseMessage::VectorImageRasterizationComplete(
            RasterizationCompleteResponse {
                pipeline_id,
                image_id: pending_image_id,
                requested_size,
            },
        ));
    }
}
```

**Input:**
```rust
rasterized_image = RasterImage { metadata: 200×200, bytes: 160000, ... }
pending_image_id = PendingImageId(1)
requested_size = 200×200
```

**Output:** Pipeline `(1,1)` is notified that rasterization is complete. This triggers another reflow where `rasterize_vector_image()` will find the cached result.

---

## Data Flow

```
data:image/svg+xml;base64,... URL
           │
           ▼
get_cached_image_status() → ReadyForRequest
           │
           ▼
PendingImage created (state: Unrequested)
           │
           ▼
Image cache fetches data URL (instant for data: URLs)
           │
           ▼
complete_load(key=1, LoadedVectorImage)
           │
     ┌─────┴─────┐
     │           │
     ▼           ▼
store usvg::Tree   build VectorImage metadata
in vector_images   { id: 1, width: 200, height: 200 }
     │           │
     └─────┬─────┘
           ▼
    notify listeners
           │
           ▼
    next layout pass: get_cached_image_for_url() → "OK"
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 5.1 | Image cache request | [context.rs:127](../../components/layout/context.rs#L127) | `cache_result` — ReadyForRequest or Available |
| 5.2 | complete_load entry | [image_cache.rs:597](../../components/net/image_cache.rs#L597) | `key`, `load_result` — should be `LoadedVectorImage` |
| 5.2 | VectorImage stored | [image_cache.rs:610](../../components/net/image_cache.rs#L610) | `natural_dimensions` = 200×200 |
| 5.3 | complete_load_svg | [image_cache.rs:569](../../components/net/image_cache.rs#L569) | Listener count, pipeline_id |

### Trace Output

```
[SVG_TRACE_STAGE_5] complete_load() ENTER key=PendingImageId(1) is_vector=true
[SVG_TRACE_STAGE_5] complete_load() VectorImage detected, inserting into vector_images
[SVG_TRACE_STAGE_5] complete_load() VectorImage natural_dimensions=200x200
[SVG_TRACE_STAGE_5] complete_load_svg() ENTER pending_image_id=PendingImageId(1) requested_size=200x200 rasterized_size=200x200
[SVG_TRACE_STAGE_5] complete_load_svg() found 1 listener(s)
[SVG_TRACE_STAGE_5] complete_load_svg() notifying pipeline_id=(1,1)
```

### Key Variables

| Variable | Type | Meaning | Value |
|----------|------|---------|-------|
| `key` | `LoadKey` / `PendingImageId` | Identifies this image load | `PendingImageId(1)` |
| `vector_image.svg_tree.size()` | `Size<f32>` | Natural size from usvg parsing | `200.0 × 200.0` |
| `vector_image.metadata` | `ImageMetadata` | Cached image dimensions | `{ width: 200, height: 200 }` |
| `listeners` | Vec of callbacks | Who to notify when done | `[(PipelineId(1,1), fn)]` |
