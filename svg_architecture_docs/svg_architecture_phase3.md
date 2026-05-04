# SVG Architecture Study - Phase 3: Serialization Pipeline

## Overview

This document traces the complete serialization pipeline for SVG elements in Servo. The pipeline converts inline SVG DOM subtrees into rasterized bitmaps through a multi-step process involving XML serialization, base64 encoding, image caching, and `resvg`-based rasterization. This serialization approach is the root cause of the three SVG issues (CSS inheritance, web fonts, crisp transforms) because it breaks the connection between SVG content and the document's rendering context.

## Key Files and Their Roles

| File | Purpose | Importance |
|------|---------|------------|
| [components/script/dom/svg/svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs) | `serialize_and_cache_subtree()` method, data URL creation | **Most Critical** |
| [components/net/image_cache.rs](components/net/image_cache.rs) | `rasterize_vector_image()` with `resvg::render()`, SVG parsing | **Most Critical** |
| [components/shared/net/image_cache.rs](components/shared/net/image_cache.rs) | `VectorImage` struct definition, image cache API | High |
| [components/layout/fragment_tree/fragment.rs](components/layout/fragment_tree/fragment.rs) | `ImageFragment` struct for final rendered output | High |
| [components/layout/display_list/mod.rs](components/layout/display_list/mod.rs) | Display list building for `ImageFragment` | High |
| [components/layout/replaced.rs](components/layout/replaced.rs) | `make_fragments()` calling `rasterize_vector_image()` | Medium |
| [components/layout/context.rs](components/layout/context.rs) | `ImageResolver::rasterize_vector_image()` bridge | Medium |

## Serialization Pipeline - End to End

### High-Level Pipeline

```
DOM (script thread)                   Layout (layout thread)            Image Cache (net thread)
                                     
SVGSVGElement                         
  │                                    
  ├── serialize_and_cache_subtree()    
  │     ├── xml_serialize()            │
  │     ├── base64 encode              │
  │     └── data:image/svg+xml;base64  │
  │           │                        │
  │           └──► cached_serialized_data_url
  │                                    │
  │◄───────────────────────────────────┘
  │    (via ReflowResult)
  │
  └── data URL ──────────────────────► ImageCache
                                            │
                                            ├── parse_svg_document_in_memory()
                                            │     └── usvg::Tree
                                            │
                                            ├── VectorImage storage
                                            │     └── svg_tree + metadata
                                            │
                                            └── rasterize_vector_image()
                                                  ├── tiny_skia::Pixmap
                                                  ├── resvg::render()
                                                  └── RasterImage → ImageKey
                                                        │
                                                        └──► WebRender
```

### Step-by-Step Flow

#### Step 1: XML Serialization in Script Thread

`SVGSVGElement::serialize_and_cache_subtree()` in [svgsvgelement.rs:79](components/script/dom/svg/svgsvgelement.rs:79):

1. **Process `<use>` elements**: Clone referenced nodes to include them in serialization
2. **XML Serialization**: Call `Node::xml_serialize(TraversalScope::IncludeNode)`
3. **Base64 Encoding**: Convert XML string to base64
4. **Data URL Creation**: Format as `data:image/svg+xml;base64,...`
5. **Caching**: Store in `cached_serialized_data_url` field

**Critical Issue**: This serialization strips document context:
- CSS inheritance from parent elements is lost
- `@import` statements in `<style>` elements lose document base URL
- External resources (fonts, images) may not load from data URL context

#### Step 2: Image Cache Integration

When layout requests the SVG via `get_cached_image_for_url()`:

1. **Data URL Parsing**: `ServoUrl::parse("data:image/svg+xml;base64,...")`
2. **Cache Lookup**: Image cache checks if this data URL is already loaded
3. **SVG Parsing**: `parse_svg_document_in_memory()` in [image_cache.rs:67](components/net/image_cache.rs:67):
   - Uses `usvg::Tree::from_data()` to parse SVG XML
   - Configures `usvg::Options` with font database
   - Disables local file loading for `<image href>` elements
4. **VectorImage Creation**: Stores parsed `usvg::Tree` with metadata

#### Step 3: Rasterization on Demand

`rasterize_vector_image()` in [image_cache.rs:967](components/net/image_cache.rs:967):

1. **Size Calculation**: Clamp dimensions to `MAX_SVG_PIXMAP_DIMENSION` (5000px)
2. **Transform Setup**: Compute scaling from natural size to requested size
3. **Pixmap Allocation**: `tiny_skia::Pixmap::new()` for target dimensions
4. **resvg Rendering**: `resvg::render(&vector_image.svg_tree, transform, &mut pixmap)`
5. **RasterImage Creation**: Convert pixmap to `RasterImage` with `ImageKey`

**Critical Limitation**: Rasterization happens at a specific size. CSS transforms applied later scale the bitmap, not the vector data.

#### Step 4: Fragment Tree Integration

`make_fragments()` in [replaced.rs:474](components/layout/replaced.rs:474) for `ReplacedContentKind::SVGElement`:

1. **Size Determination**: Uses `vector_image.metadata.width/height` (ignores viewBox)
2. **DPI Scaling**: Applies device pixel ratio
3. **Rasterization Request**: Calls `layout_context.image_resolver.rasterize_vector_image()`
4. **ImageFragment Creation**: `Fragment::Image(ImageFragment)` with `ImageKey`

#### Step 5: Display List Generation

`build_display_list()` in [display_list/mod.rs:680](components/layout/display_list/mod.rs:680) for `Fragment::Image`:

1. **WebRender Image Push**: `builder.wr().push_image()` with `ImageKey`
2. **Contentful Detection**: Marks element as "contentful" for paint timing (not broken images)
3. **Broken Image Fallback**: Renders border if `showing_broken_image_icon` is true

## Key Data Structures

### Cached Serialization State ([svgsvgelement.rs:46](components/script/dom/svg/svgsvgelement.rs:46))

```rust
#[no_trace]
cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl, ()>>>,
```

- `None`: Not yet serialized (triggers queueing in layout)
- `Some(Ok(url))`: Successfully serialized data URL
- `Some(Err(()))`: Serialization failed (won't retry)

### VectorImage ([shared/net/image_cache.rs:38](components/shared/net/image_cache.rs:38))

```rust
pub struct VectorImage {
    pub id: VectorImageId,      // PendingImageId for cache lookup
    pub svg_id: Option<String>, // UUID from SVGSVGElement
    pub metadata: ImageMetadata, // Natural width/height from SVG
    pub cors_status: CorsStatus,
}
```

### Image Enum ([shared/net/image_cache.rs:32](components/shared/net/image_cache.rs:32))

```rust
pub enum Image {
    Raster(Arc<RasterImage>),  // Bitmap image
    Vector(VectorImage),       // SVG vector image
}
```

### RasterImage (from pixels crate)

```rust
pub struct RasterImage {
    pub metadata: ImageMetadata,
    pub format: PixelFormat,
    pub frames: Vec<ImageFrame>,
    pub bytes: Arc<Vec<u8>>,   // RGBA pixel data
    pub id: Option<ImageKey>,  // WebRender image key
    pub cors_status: CorsStatus,
    pub is_opaque: bool,
}
```

### ImageFragment ([fragment.rs:86](components/layout/fragment_tree/fragment.rs:86))

```rust
pub(crate) struct ImageFragment {
    pub base: BaseFragment,
    pub clip: PhysicalRect<Au>,
    pub image_key: Option<ImageKey>,          // WebRender image key
    pub showing_broken_image_icon: bool,
    pub url: Option<ServoUrl>,                // Original data URL
}
```

## Image Cache Architecture

### Three-Level Storage

1. **Pending Loads**: `HashMap<PendingImageId, PendingLoad>` - In-flight requests
2. **Vector Images**: `HashMap<VectorImageId, VectorImage>` - Parsed SVG trees
3. **Rasterized Images**: `HashMap<(VectorImageId, DeviceIntSize), RasterizedEntry>` - Size-specific bitmaps

### SVG ID Mapping

```rust
svg_id_image_id_map: Mutex<HashMap<String, VectorImageId>>
```

Maps `SVGSVGElement.uuid` to `VectorImageId` for cache invalidation when SVG changes.

### Thread Pool Rasterization

Rasterization happens in `self.thread_pool.spawn()` to avoid blocking layout:
- Natural size from `vector_image.svg_tree.size().to_int_size()`
- Requested size clamped to `MAX_SVG_PIXMAP_DIMENSION`
- Transform computed as `requested_size / natural_size`
- `resvg::render()` does the actual vector-to-raster conversion

## Critical Serialization Issues

### 1. CSS Inheritance Breakage

**Root Cause**: Serialization captures computed SVG XML, not the live DOM with style context.

**Flow**:
```
<div style="fill: green">
    ├── <svg> (DOM element with parent style)
    │     └── <text> (inherits fill: green)
    │
    └── data:image/svg+xml;base64,... (serialized XML)
          └── <svg> (no parent context)
                └── <text> (default fill: black)
```

**Result**: CSS properties like `fill`, `stroke`, `font-family` don't inherit from HTML ancestors.

### 2. Web Fonts Failure

**Root Cause**: `@import` and `url()` in SVG `<style>` elements resolve against data URL context.

**Issues**:
1. **No Document Base URL**: Data URLs have no base URL for relative font URLs
2. **Cross-Origin Restrictions**: Font loading from data URLs may be blocked
3. **Font Database Isolation**: `usvg::Options` uses separate font database

**Example**:
```svg
<style>
@import url('https://fonts.googleapis.com/css?family=Roboto');
text { font-family: 'Roboto'; }
</style>
```
The `@import` won't execute when SVG is loaded as a data URL image.

### 3. Crisp Transforms Failure

**Root Cause**: SVG rasterized at natural size, CSS transforms apply to bitmap.

**Flow**:
```
SVG vector (100x100) ──resvg──► Bitmap (100x100) ──CSS transform scale(4)──► Bitmap (400x400) blurry
```

**Expected**:
```
SVG vector (100x100) ──CSS transform scale(4)──► Vector rendering at 400x400 ──rasterize──► Crisp bitmap
```

The vector data is lost after first rasterization. All subsequent transforms operate on pixels.

## Performance Implications

### Two-Reflow Minimum

1. **First reflow**: Discovers un-serialized SVG, queues it
2. **Script serialization**: `serialize_and_cache_subtree()`
3. **Node dirtying**: `node.dirty(NodeDamage::Other)`
4. **Second reflow**: Uses cached data URL

### Memory Overhead

1. **XML String**: Full SVG subtree as string
2. **Base64 Expansion**: ~33% size increase
3. **Data URL Prefix**: `data:image/svg+xml;base64,` overhead
4. **Parsed SVG Tree**: `usvg::Tree` in memory
5. **Rasterized Bitmaps**: Multiple sizes for Hi-DPI, zoom levels

### Cache Invalidation

When SVG content changes:
1. `invalidate_cached_serialized_subtree()` sets `cached_serialized_data_url = None`
2. `evict_rasterized_image(&self.uuid)` removes from image cache
3. Next reflow re-triggers serialization pipeline

## Comparison: Current vs. Proper Implementation

| Aspect | Current (Serialization) | Proper (Direct Vector) |
|--------|-------------------------|------------------------|
| **Rendering Path** | DOM → XML → base64 → data URL → parse → rasterize → bitmap | DOM → vector display items |
| **CSS Inheritance** | Broken after serialization | Full cascade participation |
| **Font Loading** | Data URL context fails | Shared document font loading |
| **Transform Quality** | Bitmap scaling (blurry) | Vector scaling (crisp) |
| **Memory** | XML + base64 + parsed tree + bitmaps | Vector tree only |
| **Performance** | 2+ reflows, thread pool rasterization | Single reflow, direct rendering |
| **Dynamic Updates** | Full re-serialization on change | Incremental vector updates |

## Key Code References

### Serialization Chain
```
script/dom/svg/svgsvgelement.rs:79
    → serialize_and_cache_subtree()
    → xml_serialize() → base64 → data URL
    → cached_serialized_data_url

layout/replaced.rs:221
    → svg_kind_size()
    → get_cached_image_for_url(data_url)

net/image_cache.rs:67
    → parse_svg_document_in_memory()
    → usvg::Tree::from_data()

net/image_cache.rs:967
    → rasterize_vector_image()
    → resvg::render() → RasterImage

layout/replaced.rs:474
    → make_fragments()
    → Fragment::Image(ImageFragment)

layout/display_list/mod.rs:680
    → build_display_list()
    → wr().push_image(ImageKey)
```

### Cache Management
```
script/dom/svg/svgsvgelement.rs:164
    → invalidate_cached_serialized_subtree()
    → evict_rasterized_image()

net/image_cache.rs:1086
    → evict_rasterized_image()
    → Remove from svg_id_image_id_map
```

## Summary

The serialization pipeline represents a fundamental architectural mismatch: treating SVG as a serializable image format rather than as a first-class document type. This approach creates insurmountable barriers for CSS integration, font loading, and high-quality rendering. A proper solution requires bypassing serialization entirely and integrating SVG directly into Servo's layout and rendering pipelines.