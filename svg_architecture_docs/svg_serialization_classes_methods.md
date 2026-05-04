# SVG Serialization Pipeline - Classes and Methods Documentation

## Overview

This document provides detailed documentation of every class, struct, enum, trait, and method involved in the SVG serialization pipeline in Servo. The serialization pipeline converts inline SVG DOM subtrees into rasterized bitmaps through XML serialization, base64 encoding, image caching, and `resvg`-based rasterization.

## Files Covered

1. [components/script/dom/svg/svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs) - SVG serialization in DOM
2. [components/net/image_cache.rs](components/net/image_cache.rs) - Image cache with SVG parsing/rasterization
3. [components/shared/net/image_cache.rs](components/shared/net/image_cache.rs) - Shared image cache types
4. [components/layout/fragment_tree/fragment.rs](components/layout/fragment_tree/fragment.rs) - ImageFragment for rendering
5. [components/layout/display_list/mod.rs](components/layout/display_list/mod.rs) - Display list building for images

---

## 1. components/script/dom/svg/svgsvgelement.rs

### SVGSVGElement Struct

**Location**: Line 35-47

```rust
pub struct SVGSVGElement {
    svggraphicselement: SVGGraphicsElement,
    uuid: String,
    // The XML source of subtree rooted at this SVG element, serialized into
    // a base64 encoded `data:` url. This is cached to avoid recomputation
    // on each layout and must be invalidated when the subtree changes.
    #[no_trace]
    cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl, ()>>>,
}
```

**Purpose**: Represents an `<svg>` element in the DOM with serialization caching.

**Fields**:
- `svggraphicselement`: Base SVG graphics element
- `uuid`: Unique identifier for cache invalidation (UUID v4)
- `cached_serialized_data_url`: Cached data URL of serialized SVG subtree

### Methods

#### `serialize_and_cache_subtree()` - Line 79

```rust
pub(crate) fn serialize_and_cache_subtree(&self)
```

**Purpose**: Serializes the SVG subtree to XML, encodes as base64, creates data URL, and caches it.

**Flow**:
1. Process `<use>` elements (clone referenced nodes)
2. Call `Node::xml_serialize(TraversalScope::IncludeNode)`
3. Base64 encode XML string
4. Format as `data:image/svg+xml;base64,...`
5. Parse as `ServoUrl`, store in `cached_serialized_data_url`

**Inputs**: `&self` (SVGSVGElement)
**Outputs**: None (updates `cached_serialized_data_url`)

**Critical Issues**:
- Strips document context (CSS inheritance broken)
- External resources (`@import`, `url()`) lose document base URL
- Requires processing `<use>` elements separately

#### `process_use_elements()` - Line 105

```rust
fn process_use_elements(&self, cx: &mut JSContext) -> Vec<DomRoot<Node>>
```

**Purpose**: Clone nodes referenced by `<use href="#id">` elements for inclusion in serialization.

**Inputs**: `&self`, `cx: &mut JSContext`
**Outputs**: `Vec<DomRoot<Node>>` of cloned nodes to clean up later

#### `process_single_use_element()` - Line 122

```rust
fn process_single_use_element(
    &self,
    cx: &mut JSContext,
    use_element: &Element,
) -> Option<DomRoot<Node>>
```

**Purpose**: Process a single `<use>` element, cloning its referenced element.

**Inputs**: `&self`, `cx: &mut JSContext`, `use_element: &Element`
**Outputs**: `Option<DomRoot<Node>>` cloned node if reference valid

#### `cleanup_cloned_nodes()` - Line 153

```rust
fn cleanup_cloned_nodes(&self, cx: &mut JSContext, cloned_nodes: &[DomRoot<Node>])
```

**Purpose**: Remove cloned `<use>` element nodes after serialization.

**Inputs**: `&self`, `cx: &mut JSContext`, `cloned_nodes: &[DomRoot<Node>]`
**Outputs**: None

#### `invalidate_cached_serialized_subtree()` - Line 164

```rust
fn invalidate_cached_serialized_subtree(&self)
```

**Purpose**: Invalidate cached serialization when SVG content changes.

**Actions**:
1. Sets `cached_serialized_data_url = None`
2. Calls `node.dirty(NodeDamage::Other)` to trigger reflow

**Inputs**: `&self`
**Outputs**: None

#### `data()` (LayoutDom implementation) - Line 170

```rust
fn data(&self, element: LayoutDom<'dom, Self>) -> SVGElementData<'dom>
```

**Purpose**: Create `SVGElementData` for layout thread (see Phase 2 documentation).

**Inputs**: `element: LayoutDom<'dom, Self>`
**Outputs**: `SVGElementData<'dom>` with source URL and dimensions

---

## 2. components/shared/net/image_cache.rs

### Image Enum - Line 32

```rust
pub enum Image {
    Raster(#[conditional_malloc_size_of] Arc<RasterImage>),
    Vector(VectorImage),
}
```

**Purpose**: Represents either a raster image (bitmap) or vector image (SVG).

**Variants**:
- `Raster(Arc<RasterImage>)`: Pixel-based image with RGBA data
- `Vector(VectorImage)`: SVG vector image requiring rasterization

### VectorImage Struct - Line 38

```rust
pub struct VectorImage {
    pub id: VectorImageId,
    pub svg_id: Option<String>,
    pub metadata: ImageMetadata,
    pub cors_status: CorsStatus,
}
```

**Purpose**: Represents a parsed SVG image in the cache.

**Fields**:
- `id: VectorImageId`: Unique identifier for cache lookup (`PendingImageId`)
- `svg_id: Option<String>`: UUID from `SVGSVGElement.uuid` for invalidation
- `metadata: ImageMetadata`: Natural width/height from SVG
- `cors_status: CorsStatus`: CORS status of the image

### Image Methods - Line 46

#### `metadata()`

```rust
pub fn metadata(&self) -> ImageMetadata
```

**Purpose**: Get image metadata (width/height).

**Inputs**: `&self`
**Outputs**: `ImageMetadata`

#### `cors_status()`

```rust
pub fn cors_status(&self) -> CorsStatus
```

**Purpose**: Get CORS status.

**Inputs**: `&self`
**Outputs**: `CorsStatus`

#### `as_raster_image()`

```rust
pub fn as_raster_image(&self) -> Option<Arc<RasterImage>>
```

**Purpose**: Get raster image if available (returns `None` for vector images).

**Inputs**: `&self`
**Outputs**: `Option<Arc<RasterImage>>`

### ImageOrMetadataAvailable Enum - Line 70

```rust
pub enum ImageOrMetadataAvailable {
    ImageAvailable { image: Image, url: ServoUrl },
    MetadataAvailable(ImageMetadata, PendingImageId),
}
```

**Purpose**: Result of image cache lookup - either full image or just metadata.

**Variants**:
- `ImageAvailable { image: Image, url: ServoUrl }`: Full image loaded
- `MetadataAvailable(ImageMetadata, PendingImageId)`: Only metadata available (for vector images)

### ImageCache Trait - Line 174

**Purpose**: Public API for image cache operations.

#### Key Methods:

##### `rasterize_vector_image()` - Line 201

```rust
fn rasterize_vector_image(
    &self,
    image_id: VectorImageId,
    size: DeviceIntSize,
    svg_id: Option<String>,
) -> Option<RasterImage>
```

**Purpose**: Rasterize vector image to specific size.

**Inputs**:
- `image_id: VectorImageId`: ID of vector image to rasterize
- `size: DeviceIntSize`: Target rasterization size
- `svg_id: Option<String>`: SVG UUID for cache mapping

**Outputs**: `Option<RasterImage>` if already rasterized at this size, `None` triggers async rasterization

##### `evict_rasterized_image()` - Line 220

```rust
fn evict_rasterized_image(&self, svg_id: &str)
```

**Purpose**: Remove rasterized image from cache by SVG ID.

**Inputs**: `svg_id: &str` (SVGSVGElement.uuid)
**Outputs**: None

---

## 3. components/net/image_cache.rs

### Constants

#### `MAX_SVG_PIXMAP_DIMENSION` - Line 51

```rust
const MAX_SVG_PIXMAP_DIMENSION: u32 = 5000;
```

**Purpose**: Maximum dimension for SVG rasterization pixmap (prevents memory exhaustion from large viewBox values).

### Helper Functions

#### `parse_svg_document_in_memory()` - Line 67

```rust
fn parse_svg_document_in_memory(
    bytes: &[u8],
    fontdb: Arc<fontdb::Database>,
) -> Result<usvg::Tree, &'static str>
```

**Purpose**: Parse SVG XML bytes into `usvg::Tree`.

**Inputs**:
- `bytes: &[u8]`: SVG XML data
- `fontdb: Arc<fontdb::Database>`: Font database for text rendering

**Outputs**: `Result<usvg::Tree, &'static str>`

**Configuration**:
- Disables local file loading for `<image href>` elements
- Uses shared font database
- Returns error string on parse failure

### VectorImageData Struct - Line 244

```rust
struct VectorImageData {
    #[conditional_malloc_size_of]
    svg_tree: Arc<usvg::Tree>,
    cors_status: CorsStatus,
}
```

**Purpose**: Internal representation of parsed SVG tree in cache.

**Fields**:
- `svg_tree: Arc<usvg::Tree>`: Parsed SVG tree from `usvg`
- `cors_status: CorsStatus`: CORS status

### ImageCacheImpl Struct

**Location**: Various fields throughout file

**Key Fields**:
- `store: Arc<Mutex<ImageStore>>`: Main image storage
- `svg_id_image_id_map: Mutex<HashMap<String, VectorImageId>>`: Maps SVG UUID to image ID
- `image_id_size_map: Mutex<HashMap<VectorImageId, Vec<DeviceIntSize>>>`: Tracks requested rasterization sizes
- `thread_pool: ThreadPool`: For async rasterization

### ImageStore Struct

**Key Fields**:
- `vector_images: HashMap<VectorImageId, VectorImageData>`: Parsed SVG trees
- `rasterized_vector_images: HashMap<(VectorImageId, DeviceIntSize), RasterizedEntry>`: Size-specific rasterizations
- `pending_loads: HashMap<PendingImageId, PendingLoad>`: In-flight image loads

### Methods

#### `rasterize_vector_image()` - Line 967

```rust
fn rasterize_vector_image(
    &self,
    image_id: PendingImageId,
    requested_size: DeviceIntSize,
    svg_id: Option<String>,
) -> Option<RasterImage>
```

**Purpose**: Core SVG rasterization method using `resvg`.

**Flow**:
1. Look up `VectorImageData` by `image_id`
2. Check cache for existing rasterization at `requested_size`
3. Update SVG ID mapping if `svg_id` provided
4. Spawn thread pool task for async rasterization
5. Return `None` (triggers pending rasterization tracking)

**Rasterization Task** (lines 1011-1064):
1. Get natural size from `svg_tree.size().to_int_size()`
2. Clamp requested size to `MAX_SVG_PIXMAP_DIMENSION`
3. Compute transform: `requested_size / natural_size`
4. Create `tiny_skia::Pixmap` for target size
5. Call `resvg::render(&svg_tree, transform, &mut pixmap)`
6. Convert pixmap to `RasterImage`
7. Store in cache via `load_image_with_keycache()`

**Inputs**:
- `image_id: PendingImageId`: Vector image ID
- `requested_size: DeviceIntSize`: Target rasterization size
- `svg_id: Option<String>`: SVG element UUID

**Outputs**: `Option<RasterImage>` if cached, `None` triggers async rasterization

#### `evict_rasterized_image()` - Line 1086

```rust
fn evict_rasterized_image(&self, svg_id: &str)
```

**Purpose**: Remove all cached data for an SVG by its UUID.

**Actions**:
1. Remove from `svg_id_image_id_map`
2. Remove from `vector_images`
3. Remove all size entries from `rasterized_vector_images`
4. Remove from `image_id_size_map`

**Inputs**: `svg_id: &str`
**Outputs**: None

---

## 4. components/layout/fragment_tree/fragment.rs

### ImageFragment Struct - Line 86

```rust
pub(crate) struct ImageFragment {
    pub base: BaseFragment,
    pub clip: PhysicalRect<Au>,
    pub image_key: Option<ImageKey>,
    pub showing_broken_image_icon: bool,
    pub url: Option<ServoUrl>,
}
```

**Purpose**: Fragment for rendered images, including rasterized SVGs.

**Fields**:
- `base: BaseFragment`: Common fragment properties
- `clip: PhysicalRect<Au>`: Clipping rectangle
- `image_key: Option<ImageKey>`: WebRender image key (from rasterization)
- `showing_broken_image_icon: bool`: Whether broken image icon is shown
- `url: Option<ServoUrl>`: Original image URL (data URL for SVG)

### Fragment Enum Variant - Line 114

```rust
Fragment::Image(ArcRefCell<ImageFragment>)
```

**Purpose**: Enum variant for image fragments in fragment tree.

---

## 5. components/layout/display_list/mod.rs

### DisplayListBuilder Struct

**Key Fields**:
- `image_resolver: Arc<ImageResolver>`: For image lookup/rasterization
- `device_pixel_ratio: Scale<f32, StyloCSSPixel, StyloDevicePixel>`: DPI scaling

### Methods

#### Image Fragment Handling - Line 680

```rust
Fragment::Image(image) => {
    let image = image.borrow();
    let style = image.base.style();
    // ... visibility check ...
    let image_rendering = style.get_inherited_box().image_rendering.to_webrender();
    let rect = image.base.rect.translate(...).to_webrender();
    let clip = image.clip.translate(...).to_webrender();
    let common = builder.common_properties(clip, &style);

    if let Some(image_key) = image.image_key {
        builder.wr().push_image(
            &common,
            rect,
            image_rendering,
            wr::AlphaType::PremultipliedAlpha,
            image_key,
            wr::ColorF::WHITE,
        );
        // ... paint timing logic ...
    }
}
```

**Purpose**: Build WebRender display list item for image fragment.

**Key Operations**:
1. Extract `image_key` from `ImageFragment`
2. Compute rendering rectangle and clip
3. Call `wr().push_image()` with WebRender image key
4. Handle broken image icon border if needed
5. Mark as "contentful" for paint timing (except broken images)

#### Background Image Rasterization - Line 1528

```rust
CachedImage::Vector(vector_image) => {
    let scale = builder.device_pixel_ratio.get();
    let default_size: DeviceIntSize = 
        Size2D::new(size.width * scale, size.height * scale).to_i32();
    // ... compute layer size ...
    node.and_then(|node| {
        let size = layer_size.unwrap_or(default_size);
        builder.image_resolver.rasterize_vector_image(
            vector_image.id,
            size,
            node,
            vector_image.svg_id,
        )
    })
    .and_then(|rasterized_image| rasterized_image.id)
}
```

**Purpose**: Rasterize vector images used in CSS backgrounds.

**Flow**:
1. Compute target size with DPI scaling
2. Call `image_resolver.rasterize_vector_image()`
3. Extract `ImageKey` from resulting `RasterImage`

---

## 6. components/layout/replaced.rs

### SVG-Related Methods (see Phase 2 for full details)

#### `svg_kind_size()` - Line 221

**Purpose**: Determine SVG natural size and check serialization status.

**SVG Source Handling** (lines 276-304):
```rust
match svg_data.source {
    // Not serialized yet - queue for serialization
    None => {
        context.image_resolver.queue_svg_element_for_serialization(node);
        (ReplacedContentKind::SVGElement(None), natural_size)
    },
    // Previous serialization failed
    Some(Err(())) => (ReplacedContentKind::SVGElement(None), natural_size),
    // Data URL available - get from image cache
    Some(Ok(url)) => {
        let image = context.image_resolver.get_cached_image_for_url(
            node.opaque(),
            url,
            LayoutImageDestination::Layout,
        );
        // ... create VectorImage if SVG ...
    }
}
```

#### `make_fragments()` for SVG - Line 474

**Purpose**: Create `ImageFragment` for SVG element.

**Key Operations**:
1. Extract `VectorImage` from `ReplacedContentKind::SVGElement`
2. Compute fragment rectangle from vector image metadata
3. Apply DPI scaling for rasterization size
4. Call `rasterize_vector_image()` via `image_resolver`
5. Create `Fragment::Image(ImageFragment)` with resulting `ImageKey`

---

## 7. components/layout/context.rs

### ImageResolver Struct - Line 78

**Key Fields**:
- `pending_svg_elements_for_serialization: Mutex<Vec<UntrustedNodeAddress>>`: SVGs needing serialization
- `pending_rasterization_images: Mutex<Vec<PendingRasterizationImage>>`: Vector images needing rasterization
- `image_cache: Arc<dyn ImageCache>`: Reference to image cache

### Methods

#### `queue_svg_element_for_serialization()` - Line 240

```rust
pub(crate) fn queue_svg_element_for_serialization(&self, element: ServoLayoutNode<'_>)
```

**Purpose**: Queue SVG element for serialization when layout encounters un-serialized SVG.

**Inputs**: `element: ServoLayoutNode<'_>`
**Outputs**: None (adds to `pending_svg_elements_for_serialization`)

#### `rasterize_vector_image()` - Line 218

```rust
pub(crate) fn rasterize_vector_image(
    &self,
    image_id: PendingImageId,
    size: DeviceIntSize,
    node: OpaqueNode,
    svg_id: Option<String>,
) -> Option<RasterImage>
```

**Purpose**: Bridge method to image cache's `rasterize_vector_image()`.

**Inputs**:
- `image_id: PendingImageId`: Vector image ID
- `size: DeviceIntSize`: Target size
- `node: OpaqueNode`: Layout node (for pending tracking)
- `svg_id: Option<String>`: SVG UUID

**Outputs**: `Option<RasterImage>` if cached, `None` triggers pending tracking

**Pending Tracking**: If rasterization not ready, adds to `pending_rasterization_images`.

---

## Data Flow Summary

### Serialization Pipeline

```
SVGSVGElement (DOM)
    ├── serialize_and_cache_subtree()
    │     ├── xml_serialize()
    │     ├── base64 encode
    │     └── data:image/svg+xml;base64,...
    │           └──► cached_serialized_data_url
    │
    └── data() → SVGElementData
          └──► Layout thread

Layout Thread
    ├── svg_kind_size()
    │     ├── source None? → queue_svg_element_for_serialization()
    │     └── source Some(url) → get_cached_image_for_url()
    │           └──► ImageCache
    │
    ├── make_fragments()
    │     └── rasterize_vector_image()
    │           └──► ImageCache::rasterize_vector_image()
    │
    └── Fragment::Image(ImageFragment)
          └──► Display list

Image Cache
    ├── parse_svg_document_in_memory()
    │     └── usvg::Tree
    │
    ├── VectorImageData storage
    │
    └── rasterize_vector_image()
          ├── tiny_skia::Pixmap
          ├── resvg::render()
          └── RasterImage → ImageKey
                └──► WebRender

Display List
    └── push_image(ImageKey)
          └──► WebRender rendering
```

### Cache Invalidation Flow

```
SVG content changes
    └── invalidate_cached_serialized_subtree()
          ├── cached_serialized_data_url = None
          ├── node.dirty(NodeDamage::Other)
          └── evict_rasterized_image(uuid)
                └──► ImageCache::evict_rasterized_image()
                      ├── Remove from svg_id_image_id_map
                      ├── Remove vector_images entry
                      └── Remove all rasterized sizes
```

## Key Patterns and Issues

### 1. Two-Reflow Requirement
- **First reflow**: Discovers un-serialized SVG, queues it
- **Script serialization**: `serialize_and_cache_subtree()`
- **Node dirtying**: `node.dirty(NodeDamage::Other)`
- **Second reflow**: Uses cached data URL

### 2. CSS Inheritance Breakage
- Serialization captures computed XML, not live DOM
- Parent CSS properties (`fill`, `stroke`, `font-family`) lost
- SVG rendered in isolation from document context

### 3. Web Fonts Failure
- `@import` and `url()` resolve against data URL context
- Data URLs have no document base URL
- Font loading may be blocked from data URLs

### 4. Crisp Transforms Failure
- SVG rasterized at natural size
- CSS transforms apply to bitmap, not vector data
- Results in blurry scaled images

### 5. Memory Overhead
- XML string + base64 expansion + data URL prefix
- Parsed `usvg::Tree` in memory
- Multiple rasterized sizes for Hi-DPI/zoom

## Conclusion

The serialization pipeline represents a complex workaround for treating SVG as a replaced element. Each component plays a specific role in converting vector graphics to rasterized bitmaps, but the architectural mismatch creates fundamental limitations for CSS integration, font loading, and rendering quality. A proper solution requires bypassing this pipeline entirely and integrating SVG directly into Servo's layout and rendering systems.