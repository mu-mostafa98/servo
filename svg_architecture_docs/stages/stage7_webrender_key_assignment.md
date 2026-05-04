# Stage 7 — WebRender Image Key Assignment

> **Thread:** Image Cache
> **Also known as:** GPU texture binding for rasterized SVG
> **Key files:**
> - [components/net/image_cache.rs](../../components/net/image_cache.rs)

---

## Overview

Stage 7 assigns a **WebRender `ImageKey`** to the rasterized SVG pixel buffer, making it available for GPU rendering. This is the final step in the image cache pipeline — after this, the SVG is just like any other texture in WebRender's atlas.

The `ImageKey` is a handle that WebRender uses to reference a GPU-side texture. Without it, the display list can reference the image but WebRender won't have the pixel data to render.

---

## Sub-stage 7.1 — Set Key and Finish Load

**File:** [image_cache.rs](../../components/net/image_cache.rs)
**Function:** `set_key_and_finish_load()`
**Lines:** 484-495

**Called from:** `load_image_with_keycache()` (which was called at the end of Stage 6)

```rust
fn set_key_and_finish_load(&mut self, pending_image: PendingKey, image_key: WebRenderImageKey) {
    match pending_image {
        PendingKey::RasterImage((pending_id, mut raster_image)) => {
            set_webrender_image_key(&self.paint_api, &mut raster_image, image_key);
            self.complete_load(pending_id, LoadResult::LoadedRasterImage(raster_image));
        },
        PendingKey::Svg((pending_id, mut raster_image, requested_size)) => {
            // Our path — SVG rasterization complete
            set_webrender_image_key(&self.paint_api, &mut raster_image, image_key);
            // raster_image.id is now Some(ImageKey(IdNamespace(1), 90))
            self.complete_load_svg(raster_image, pending_id, requested_size);
            // → notifies listeners
        },
    }
}
```

**Input:**
```rust
pending_image = PendingKey::Svg((
    PendingImageId(1),
    RasterImage { metadata: 200×200, bytes: 160000, id: None, ... },
    200×200,
))
image_key = ImageKey(IdNamespace(1), 90)
```

### Step 1 — Bind WebRender Texture

```rust
set_webrender_image_key(&self.paint_api, &mut raster_image, image_key);
```

This function (defined elsewhere in the image cache module) uploads the rasterized pixel data to WebRender and associates it with the provided `ImageKey`. After this call:

```rust
raster_image.id = Some(ImageKey(IdNamespace(1), 90))
```

The `ImageKey` struct:
```rust
ImageKey(IdNamespace(1), 90)
//   └── namespace      └── unique ID within the namespace
```

The namespace identifies the WebRender session, and the unique ID identifies this specific texture within that session.

### Step 2 — Notify Listeners

```rust
self.complete_load_svg(raster_image, pending_id, requested_size);
```

This notifies all registered listeners (pipeline `(1,1)` in our case) that the SVG is ready. The notification triggers another reflow, where:
- `make_fragments()` now has a valid `image_key` for `Fragment::Image`
- The display list can reference `ImageKey(IdNamespace(1), 90)` for GPU rendering

---

## Data Flow

```
Thread pool task completes (Stage 6)
           │
           ▼
load_image_with_keycache(PendingKey::Svg(...))
           │
           ▼
set_key_and_finish_load()
           │
     ┌─────┴─────┐
     │           │
     ▼           ▼
set_webrender_image_key()    complete_load_svg()
(pixels → GPU texture)       (notify pipeline)
     │                       │
     ▼                       ▼
raster_image.id =        pipeline (1,1) notified
Some(ImageKey(1, 90))    → triggers reflow
                              │
                              ▼
                    make_fragments() sees ImageKey
                    → Fragment::Image { image_key: Some(ImageKey(1, 90)) }
                              │
                              ▼
                    Display list → WebRender push_image(ImageKey(1, 90))
                              │
                              ▼
                    GPU renders the 200×200 blue circle
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 7.1-i | Entry | [image_cache.rs:484](../../components/net/image_cache.rs#L484) | `pending_image` variant, `image_key` |
| 7.1-ii | SVG branch | [image_cache.rs:490](../../components/net/image_cache.rs#L490) | `pending_id`, `requested_size` |
| 7.1-iii | After key set | [image_cache.rs:492](../../components/net/image_cache.rs#L492) | `raster_image.id` should be `Some(ImageKey(...))` |

### Trace Output

```
[SVG_TRACE_STAGE_7] set_key_and_finish_load() image_key=ImageKey(IdNamespace(1), 90) variant="Svg"
[SVG_TRACE_STAGE_7] set_key_and_finish_load() SVG variant, pending_id=PendingImageId(1) requested_size=200x200
```

### Key Variables

| Variable | Before Stage 7 | After Stage 7 |
|----------|----------------|---------------|
| `raster_image.id` | `None` | `Some(ImageKey(IdNamespace(1), 90))` |
| `image_key` | N/A | `ImageKey(IdNamespace(1), 90)` |

### The ImageKey

`WebRenderImageKey` (aliased from `webrender_api::ImageKey`) is a lightweight handle:

```rust
pub struct ImageKey(pub IdNamespace, pub u32);
```

- `IdNamespace`: Identifies the WebRender API connection (usually `IdNamespace(1)` for the main display list builder)
- `u32`: A monotonically increasing ID within that namespace, assigned when the image is uploaded

The actual texture data lives in WebRender's texture cache (GPU memory). The `ImageKey` is the only reference needed to push an image into a display list.
