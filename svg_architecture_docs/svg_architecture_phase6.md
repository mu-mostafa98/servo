# SVG Architecture Study - Phase 6: SVG Cache Lifecycle & Memory Management

## Overview

This document covers the complete lifecycle of SVG data in Servo's caching system — from the moment an SVG is serialized in script, through image cache storage, rasterization, and eventual eviction. The cache lifecycle is critical because SVG data flows through three distinct caching layers (DOM-level, image cache, rasterized output) and must be properly invalidated when SVG content changes.

## Key Files

| File | Purpose | Importance |
|------|---------|------------|
| [components/net/image_cache.rs](components/net/image_cache.rs) | Core image cache with SVG storage tiers | **Most Critical** |
| [components/script/dom/svg/svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs) | SVG serialization invalidation triggers | **Most Critical** |
| [components/shared/net/image_cache.rs](components/shared/net/image_cache.rs) | Shared cache types (VectorImage, Image) | High |
| [components/layout/context.rs](components/layout/context.rs) | ImageResolver with pending SVG tracking | High |
| [components/layout/layout_impl.rs](components/layout/layout_impl.rs) | Reflow flushing for pending SVGs | Medium |
| [components/script/dom/window.rs](components/script/dom/window.rs) | Post-reflow SVG serialization processing | High |

## Three-Tier SVG Cache Architecture

### Tier 1: DOM-Level Cache (SVGSVGElement)

**Location**: `SVGSVGElement.cached_serialized_data_url` — a `DomRefCell<Option<Result<ServoUrl, ()>>>`

**Purpose**: Avoids re-serializing the SVG subtree on every reflow.

**States**:
```
None                    → Not serialized yet (triggers queue in layout)
Some(Ok(ServoUrl))      → Successfully serialized data URL
Some(Err(()))           → Serialization failed (no retry)
```

**Concurrent Access**: Uses `DomRefCell`, only accessible from the script thread. Layout accesses it through `borrow_for_layout()` on the `JSContext` via `SVGSVGElement::data()`.

### Tier 2: Image Cache — Vector Images

**Location**: `ImageCacheStore.vector_images: FxHashMap<PendingImageId, VectorImageData>`

**Purpose**: Stores parsed `usvg::Tree` objects for reuse at multiple rasterization sizes.

**Key Type**: `PendingImageId` (which is a `LoadKey`)
**Value Type**: `VectorImageData { svg_tree: Arc<usvg::Tree>, cors_status: CorsStatus }`

**Uniqueness**: Stored by the data URL's hash key. Multiple `<svg>` elements with identical content would share the same `VectorImageData`.

### Tier 3: Image Cache — Rasterized Images

**Location**: `ImageCacheStore.rasterized_vector_images: FxHashMap<(PendingImageId, DeviceIntSize), RasterizationTask>`

**Purpose**: Caches rasterized bitmaps at specific sizes to avoid re-rasterization on every paint.

**Key Type**: `(PendingImageId, DeviceIntSize)` — image ID + requested rasterization size
**Value Type**: `RasterizationTask { result: Option<RasterImage>, listeners: Vec<(PipelineId, ImageCacheResponseCallback)> }`

**Size Variants**: The same SVG may be cached at multiple sizes (normal, Hi-DPI, zoomed, etc.).

## Complete Cache Lifecycle

### Phase 1: Initial Layout — Miss

```
Layout encounters <svg> element
    ↓
svg_kind_size() checks SVGElementData.source
    ↓ source is None (not serialized)
queue_svg_element_for_serialization(node)
    ↓
Fragments = [] (no content rendered)
    ↓
After reflow, pending_svg_elements returned to script
    ↓
Script calls serialize_and_cache_subtree()
    ↓
cached_serialized_data_url = Some(Ok(data_url))
node.dirty(NodeDamage::Other)
    ↓
Second reflow triggered
```

### Phase 2: Second Layout — Data URL Fetch

```
Layout encounters <svg> again
    ↓
SVGElementData.source = Some(Ok(data_url))
    ↓
get_cached_image_for_url(data_url)
    ↓
ImageCache.get_cached_image_status(data_url)
    ↓
CacheResult::ReadyForRequest(id) or Pending(id)
    ↓
PendingImage added to pending_images
    ↓
Script processes pending image:
    data URL decoded by decode_bytes_sync()
    parse_svg_document_in_memory() → usvg::Tree
    DecodedImage::Vector(VectorImageData) → store in vector_images
    Image::Vector(VectorImage) → returned to layout
```

### Phase 3: Rasterization

```
Layout has VectorImage
    ↓
make_fragments() calls rasterize_vector_image(id, size, svg_id)
    ↓
ImageCache.rasterize_vector_image():
    1. Retrieve VectorImageData.svg_tree
    2. Clamp size to MAX_SVG_PIXMAP_DIMENSION (5000px)
    3. Spawn thread pool task:
        a. Compute transform (requested / natural size)
        b. tiny_skia::Pixmap::new(width, height)
        c. resvg::render(&svg_tree, transform, &mut pixmap)
        d. Convert pixmap → RasterImage
        e. load_image_with_keycache(Svg((id, RasterImage, size)))
        f. WebRender ImageKey assigned
        g. complete_load_svg() → notify listeners
    4. Return None (pending)
    ↓
Layout gets no image, creates pending_rasterization_images entry
    ↓
Script processes pending_rasterization_images
    ↓
When rasterization completes → listener notified → reflow/repaint
```

### Phase 4: Cache Hit

```
rasterize_vector_image() called again
    ↓
Entry exists in rasterized_vector_images with result
    ↓
Return Some(RasterImage) immediately
    ↓
Layout creates ImageFragment with ImageKey
    ↓
Display list pushes WebRender image
```

### Phase 5: Invalidation — Content Change

```
SVG child elements modified
    ↓
children_changed() on SVGSVGElement
    ↓
invalidate_cached_serialized_subtree():
    cached_serialized_data_url = None
    node.dirty(NodeDamage::Other)
    ↓
Next reflow starts again from Phase 1
```

### Phase 6: Full Eviction — DOM Removal

```
SVG element removed from document
    ↓
unbind_from_tree():
    ↓
    evict_rasterized_image(self.uuid)
        → Remove from svg_id_image_id_map
        → Remove from vector_images
        → Remove all sizes from rasterized_vector_images
        → Remove from image_id_size_map
    ↓
    remove_cached_image(&url)
        → Layout's resolved_images_cache
    ↓
    evict_completed_image(&url, origin)
        → ImageCache.completed_loads
    ↓
    invalidate_cached_serialized_subtree()
```

## SVG ID Mapping

The `svg_id_image_id_map` in the image cache tracks the relationship between `SVGSVGElement.uuid` and the image cache ID:

```rust
svg_id_image_id_map: Mutex<HashMap<String, VectorImageId>>
```

**Purpose**: When SVG content is invalidated, the UUID is used to look up the image cache entries to evict.

**Update Timing**: Updated during `rasterize_vector_image()` when `svg_id` is `Some`:

```rust
if let Some(svg_id) = svg_id {
    if let Some(old_mapped_image_id) =
        self.svg_id_image_id_map.lock().insert(svg_id, image_id)
    {
        if old_mapped_image_id != image_id {
            // Remove old entries
            store.vector_images.remove(&old_mapped_image_id);
            store.rasterized_vector_images
                .remove(&(old_mapped_image_id, requested_size));
        }
    }
}
```

This ensures that when an SVG is re-serialized (new UUID), the old cache entries are cleaned up.

## Rasterization Size Management

### Size Tracking

```rust
image_id_size_map: Mutex<HashMap<VectorImageId, Vec<DeviceIntSize>>>
```

Tracks all sizes at which a vector image has been requested. Used during eviction to remove all cached rasterization sizes.

### Size Clamping

```rust
const MAX_SVG_PIXMAP_DIMENSION: u32 = 5000;
```

Prevents memory exhaustion from excessively large viewBox values. The clamping happens in `rasterize_vector_image()`:

```rust
let width = requested_size.width.try_into()
    .unwrap_or(0)
    .min(MAX_SVG_PIXMAP_DIMENSION);
let height = requested_size.height.try_into()
    .unwrap_or(0)
    .min(MAX_SVG_PIXMAP_DIMENSION);
let tinyskia_requested_size = tiny_skia::IntSize::from_wh(width, height)
    .unwrap_or(natural_size);
```

## Memory Footprint

### Per-SVG Memory Usage

| Component | Memory | Location |
|-----------|--------|----------|
| XML String (script) | O(SVG subtree size) | `serialize_and_cache_subtree()` temp |
| Base64 Data URL | ~1.33x XML size + 28 bytes prefix | `cached_serialized_data_url` |
| `usvg::Tree` | Parsed tree + gradients + patterns + fonts | `VectorImageData` |
| Per-Size Pixmap | W × H × 4 bytes (RGBA) | `RasterizationTask.result` |
| RasterImage | Metadata + PixelData + WebRender key | Per-size cache entry |

### Scaling Concerns

1. **Multiple Sizes**: Each unique rasterization size creates a full RGBA pixmap
2. **Font Data**: Shared `fontdb::Database` is per-process, not per-image
3. **Large SVGs**: Complex SVGs with many nodes create large `usvg::Tree` objects
4. **Rasterization Storm**: SVG changes trigger full re-serialization + re-rasterization

## WebRender Image Key Assignment

### Asynchronous Flow

```
Thread pool rasterization completes
    ↓
load_image_with_keycache(PendingKey::Svg((id, raster_image, size)))
    ↓
KeyCache state:
    PendingBatch → queue for batch key assignment
    Ready → pop key, set_key_and_finish_load()
        → set_webrender_image_key(paint_api, &mut raster_image, key)
            → paint_api.add_image(key, descriptor, data, animate?)
        → complete_load_svg(raster_image, id, size)
            → notify listeners
```

The `paint_api.add_image()` call uploads pixel data to WebRender, associating it with an `ImageKey` that the display list uses.

## Key Cache Integration

The `KeyCache` manages `WebRenderImageKey` allocation:

```rust
struct KeyCache {
    cache: KeyCacheState,
    images_pending_keys: VecDeque<PendingKey>,
    evicted_images: HashSet<(PendingImageId, DeviceIntSize)>,
}

enum KeyCacheState {
    Ready(VecDeque<WebRenderImageKey>),
    PendingBatch,
}
```

- **Evicted images** are tracked to avoid re-uploading stale data
- **Batch key allocation** prevents flooding WebRender with individual key requests

## Synchronization Points

| Step | Thread | Sync Mechanism |
|------|--------|----------------|
| `cached_serialized_data_url` write | Script | `DomRefCell` (script-thread only) |
| `pending_svg_elements` push | Layout | `Mutex<Vec<UntrustedNodeAddress>>` |
| `svg_id_image_id_map` update | Net (async) | `Mutex<HashMap<...>>` |
| `rasterized_vector_images` write | Thread pool | `Mutex` via `ImageCacheStore` |
| `RasterizationTask.listeners` | Thread pool | Direct dispatch after lock |

## Cache Behavior Analysis

### Current Limitations

1. **No Shared Caching Between Elements**: Two identical SVGs create separate cache entries because each has a unique `uuid`
2. **Full Re-Serialization on Any Change**: Any child mutation triggers complete serialization of the entire SVG subtree
3. **No Partial Invalidation**: Attribute changes on a single child require re-parsing the entire SVG by `usvg`
4. **No Size Budget**: No limit on rasterized image memory; each unique size adds a full-resolution bitmap
5. **Serialization Churn**: Each reflow cycle that encounters an un-serialized SVG must wait for serialization + rasterization

### Comparison: Proper Implementation

| Aspect | Current | Proper |
|--------|---------|--------|
| **Cache Unit** | Entire SVG subtree | Individual SVG nodes |
| **Invalidation** | Full re-serialization | Targeted node updates |
| **Size Storage** | Separate bitmap per size | Vector data (scales infinitely) |
| **Memory** | XML + base64 + parse tree + bitmaps | Vector tree only |
| **Concurrent Access** | Mutex-protected multi-tier | Lock-free tree access |