# SVG Architecture Study - Phase 2: Layout Integration

## Overview

This document details how SVG elements are handled in Servo's layout system. The layout system treats SVG elements as **replaced elements** - similar to images, iframes, and videos. This is the root cause of the three issues (CSS inheritance, web fonts, crisp transforms), because SVG is serialized to a data URL, loaded through the image cache, and rasterized via `resvg` rather than being rendered as native vector content.

## Key Files and Their Roles

| File | Purpose | Importance |
|------|---------|------------|
| [components/layout/replaced.rs](components/layout/replaced.rs) | Core replaced element handling, SVG-specific size/fragment logic | **Most Critical** |
| [components/shared/layout/lib.rs](components/shared/layout/lib.rs) | `SVGElementData` struct definition, `ReflowResult` with pending SVG queue | High |
| [components/layout/context.rs](components/layout/context.rs) | `ImageResolver` with SVG serialization queue, vector image rasterization | High |
| [components/layout/layout_impl.rs](components/layout/layout_impl.rs) | Reflow orchestration, collecting pending SVGs after layout | High |
| [components/shared/layout/layout_node.rs](components/shared/layout/layout_node.rs) | `LayoutNode` trait with `svg_data()` method | Medium |
| [components/layout/dom.rs](components/layout/dom.rs) | `NodeExt` trait with `as_svg()` to extract SVGElementData | Medium |
| [components/script/layout_dom/servo_layout_node.rs](components/script/layout_dom/servo_layout_node.rs) | Script-side implementation of layout node trait | Medium |
| [components/script/dom/node/node.rs](components/script/dom/node/node.rs) | `Node::svg_data()` downcasts to `SVGSVGElement::data()` | Medium |
| [components/layout/fragment_tree/fragment.rs](components/layout/fragment_tree/fragment.rs) | `ImageFragment` struct (used for SVG rendering output) | Medium |
| [components/layout/display_list/mod.rs](components/layout/display_list/mod.rs) | Display list generation for SVG images | Medium |
| [components/layout/style_ext.rs](components/layout/style_ext.rs) | Style extensions (clip-path handling for SVG) | Low |

## SVG Layout Flow - End to End

### High-Level Pipeline

```
DOM (script thread)                   Layout (layout thread)            
                                     
SVGSVGElement                         
  │                                    
  ├── svg_data() ─────────────────► ReplacedContents::for_element()
  │     (SVGElementData)               │
  │                                    ├── svg_kind_size()
  │                                    │     ├── Parse width/height
  │                                    │     ├── If no source: queue for serialization
  │                                    │     │     └── queue_svg_element_for_serialization()
  │                                    │     └── If source: get cached image
  │                                    │           └── get_cached_image_for_url()
  │                                    │
  │                                    └── ReplacedContentKind::SVGElement(Option<VectorImage>)
  │                                          │
  │                                          └── make_fragments()
  │                                                ├── TODO: viewBox issue
  │                                                ├── rasterize_vector_image() via resvg
  │                                                └── Fragment::Image(ImageFragment)
  │                                    
  │                                    After layout:
  │                                    ┌─ pending_svg_elements_for_serialization
  │                                    └─ pending_rasterization_images
  │                                          │
  │◄──────────────────────────────────────────┘
  │    (via ReflowResult)
  │
  ├── serialize_and_cache_subtree()   (called for each pending SVG)
  └── Node::dirty(NodeDamage::Other)  (triggers second reflow)
```

### Step-by-Step Flow

#### Step 1: DOM Node Provides SVG Data

When layout encounters an `<svg>` element, it calls `node.as_svg()` which:
1. `NodeExt::as_svg()` in [dom.rs](components/layout/dom.rs:378) calls `self.svg_data()`
2. `LayoutNode::svg_data()` in [layout_node.rs](components/shared/layout/layout_node.rs:197) delegates to script
3. Script's `Node::svg_data()` in [node.rs](components/script/dom/node/node.rs:2377) downcasts to `SVGSVGElement`
4. `SVGSVGElement::data()` in [svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs:172) creates `SVGElementData` with:
   - `source`: The cached data URL (or `None` if not yet serialized)
   - `width`, `height`: SVG dimension attributes
   - `view_box`: SVG viewBox attribute
   - `svg_id`: UUID for caching

#### Step 2: Layout Creates ReplacedContents

`ReplacedContents::for_element()` in [replaced.rs](components/layout/replaced.rs:149) detects SVG via:
```rust
} else if let Some(svg_data) = node.as_svg() {
    Self::svg_kind_size(svg_data, context, node)
}
```

#### Step 3: SVG Size Determination

`svg_kind_size()` in [replaced.rs](components/layout/replaced.rs:221):
1. Computes natural size from `width`, `height`, and `viewBox` attributes
2. Checks if SVG source is available:
   - **Not serialized** (`None`): Queues SVG element for serialization, returns `None` image
   - **Error** (`Err(())`): Previous attempt failed, don't retry
   - **Available** (`Ok(url)`): Looks up cached image via `get_cached_image_for_url()`
3. If cached image found as `Image::Vector`, assigns `svg_id` to `VectorImage`
4. Returns `(ReplacedContentKind::SVGElement(vector_image), natural_size)`

#### Step 4: Fragment Generation

`make_fragments()` in [replaced.rs](components/layout/replaced.rs:474) handles `ReplacedContentKind::SVGElement`:
```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else {
        return vec![];  // No image yet, produce no fragments
    };

    // TODO: This is incorrect if the SVG has a viewBox.
    base.rect = PhysicalSize::new(
        vector_image.metadata.width.try_into().map_or(MAX_AU, Au::from_px),
        vector_image.metadata.height.try_into().map_or(MAX_AU, Au::from_px),
    ).into();

    let scale = layout_context.style_context.device_pixel_ratio();
    let raster_size = Size2D::new(
        base.rect.size.width.scale_by(scale.0).to_px(),
        base.rect.size.height.scale_by(scale.0).to_px(),
    );

    layout_context.image_resolver.rasterize_vector_image(
        vector_image.id, raster_size, tag.node, vector_image.svg_id.clone(),
    )
    .and_then(|image| image.id)
    .map(|image_key| {
        Fragment::Image(ArcRefCell::new(ImageFragment {
            base, clip,
            image_key: Some(image_key),
            showing_broken_image_icon: false,
            url: None,
        }))
    })
    .into_iter()
    .collect()
}
```

**Key observations:**
- The fragment is always `Fragment::Image(ImageFragment)` - SVG becomes a bitmap
- `rasterize_vector_image()` uses `resvg` to rasterize at the current DPI scale
- The `TODO` at line 571: viewBox handling is incorrect
- No fragments are produced if the vector image hasn't loaded yet

#### Step 5: Post-Reflow Processing

After layout completes in [layout_impl.rs](components/layout/layout_impl.rs:1013):
```rust
let pending_svg_elements_for_serialization =
    std::mem::take(&mut *image_resolver.pending_svg_elements_for_serialization.lock());
```

These are returned to script via `ReflowResult` in [lib.rs](components/shared/layout/lib.rs:614):
```rust
pub pending_svg_elements_for_serialization: Vec<UntrustedNodeAddress>,
```

Script processes them in `handle_pending_images_post_reflow()` in [window.rs](components/script/dom/window.rs:3583):
```rust
for node in pending_svg_element_for_serialization.into_iter() {
    let node = unsafe { from_untrusted_node_address(node) };
    let svg = node.downcast::<SVGSVGElement>().unwrap();
    svg.serialize_and_cache_subtree();
    node.dirty(NodeDamage::Other);  // Triggers another reflow!
}
```

**Critical insight:** This means inline SVGs require **at least two reflows**:
1. First reflow: Layout discovers SVG not serialized, queues it
2. Script serializes SVG, dirties node
3. Second reflow: Layout uses cached data URL to load SVG as image

## Key Data Structures

### SVGElementData ([lib.rs](components/shared/layout/lib.rs:152))
```rust
pub struct SVGElementData<'dom> {
    pub source: Option<Result<ServoUrl, ()>>,  // Cached data URL
    pub width: Option<&'dom AttrValue>,         // SVG width attribute
    pub height: Option<&'dom AttrValue>,        // SVG height attribute
    pub svg_id: String,                          // UUID for caching
    pub view_box: Option<&'dom AttrValue>,       // SVG viewBox attribute
}
```

### ReplacedContentKind ([replaced.rs](components/layout/replaced.rs:139))
```rust
pub enum ReplacedContentKind {
    Image(ImageInfo),
    IFrame(IFrameInfo),
    Canvas(CanvasInfo),
    Video(VideoInfo),
    SVGElement(Option<VectorImage>),  // SVG is treated like other replaced content
    Audio,
}
```

### ReplacedContents ([replaced.rs](components/layout/replaced.rs:48))
```rust
pub(crate) struct ReplacedContents {
    pub kind: ReplacedContentKind,
    natural_size: NaturalSizes,
    base_fragment_info: BaseFragmentInfo,
}
```

### ImageFragment ([fragment.rs](components/layout/fragment_tree/fragment.rs:86))
```rust
pub(crate) struct ImageFragment {
    pub base: BaseFragment,
    pub clip: PhysicalRect<Au>,
    pub image_key: Option<ImageKey>,          // WebRender image key
    pub showing_broken_image_icon: bool,
    pub url: Option<ServoUrl>,
}
```

### ImageResolver ([context.rs](components/layout/context.rs:78))
```rust
pub(crate) struct ImageResolver {
    pub origin: ImmutableOrigin,
    pub image_cache: Arc<dyn ImageCache>,
    pub pending_images: Mutex<Vec<PendingImage>>,
    pub pending_rasterization_images: Mutex<Vec<PendingRasterizationImage>>,
    pub pending_svg_elements_for_serialization: Mutex<Vec<UntrustedNodeAddress>>,
    pub animating_images: Arc<RwLock<AnimatingImages>>,
    pub resolved_images_cache: Arc<RwLock<HashMap<ServoUrl, CachedImageOrError>>>,
    pub animation_timeline_value: f64,
}
```

### VectorImage ([shared/net/image_cache.rs:38](components/shared/net/image_cache.rs:38))
```rust
pub struct VectorImage {
    pub id: VectorImageId,
    pub svg_id: Option<String>,
    pub metadata: ImageMetadata,
    pub cors_status: CorsStatus,
}
```

### ReflowResult ([lib.rs](components/shared/layout/lib.rs:603))
```rust
pub struct ReflowResult {
    pub reflow_phases_run: ReflowPhasesRun,
    pub reflow_statistics: ReflowStatistics,
    pub pending_images: Vec<PendingImage>,
    pub pending_rasterization_images: Vec<PendingRasterizationImage>,
    pub pending_svg_elements_for_serialization: Vec<UntrustedNodeAddress>,
    pub iframe_sizes: Option<IFrameSizes>,
}
```

## Layout Element Type Registration

SVG element types are registered in `LayoutElementType` in [lib.rs](components/shared/layout/lib.rs:105):
```rust
pub enum LayoutElementType {
    // ... HTML elements ...
    SVGImageElement,
    SVGSVGElement,
}
```

`SVGSVGElement` and `SVGImageElement` are the only SVG element types recognized by layout.

## Natural Size Calculation for SVG

SVG natural sizing follows the CSS Image spec and SVG spec:

### Width and Height Priority
1. **Explicit attributes**: `width` and `height` attributes on `<svg>` element
2. **ViewBox ratio**: If only one dimension is specified, derive the other from viewBox ratio
3. **Fallback**: CSS default object size (300x150)

### viewBox Parsing
The `SVGElementData::ratio_from_view_box()` in [lib.rs](components/shared/layout/lib.rs:162) parses `viewBox="min-x min-y width height"`:
- Skips `min-x` and `min-y`
- Parses `width` and `height` as unsigned integers
- Returns `width / height` ratio
- Returns `None` for degenerate cases (zero width/height)

## Rasterization Pipeline

When SVG needs to be rendered:

1. **VectorImage exists** in image cache with parsed SVG tree
2. `rasterize_vector_image()` in [context.rs](components/layout/context.rs:218) calls `image_cache.rasterize_vector_image()`
3. Image cache uses `resvg::render()` in [image_cache.rs](components/net/image_cache.rs:1035) to rasterize
4. Returns `RasterImage` with an `ImageKey` for WebRender
5. If rasterization fails (image not ready), adds to `pending_rasterization_images`
6. Fragment created as `Fragment::Image(ImageFragment)` with the `ImageKey`

## Root Cause Analysis of SVG Issues

### 1. CSS Inheritance Failure
**Layout-level cause:** SVG treated as replaced element → serialized to data URL → loaded as image → CSS cascade from parent `<div>` never reaches `<text>` inside SVG.

**Flow:**
```
<div style="fill: green">
    └── <svg> ──serialize──► data:image/svg+xml;base64,... ──load──► ImageFragment
         └── <text>    (CSS inheritance broken here)
```

### 2. Web Fonts Failure
**Layout-level cause:** SVG serialization strips `<style>` element context. Even if `@import` is preserved in the serialized XML, the data URL context doesn't trigger font loading because:
- Font loading uses document base URL
- Data URLs have no document base URL
- Font requests from data URLs may be blocked

### 3. Crisp Transforms Failure
**Layout-level cause:** SVG is rasterized at its natural size, then the resulting bitmap is scaled by CSS transform:
```
SVG (vector) ──► resvg rasterize ──► bitmap at WxH ──► CSS transform scale(4) ──► blurry
```
The vector data is lost during rasterization. The transform applies to the bitmap, not the vector.

## Comparison: Current vs. Proper Implementation

| Aspect | Current (Replaced Element) | Proper (Taffy-like Module) |
|--------|---------------------------|---------------------------|
| **Fragment Type** | `Fragment::Image(ImageFragment)` | SVG-specific fragment types |
| **Rendering** | Rasterized bitmap via resvg | Direct vector display list items |
| **CSS Integration** | Serialized, loses parent styles | Full CSS cascade participation |
| **Web Fonts** | Not supported in data URL context | Shared font system with HTML |
| **Transforms** | Applied to rasterized bitmap | Applied to vector data |
| **Performance** | Serialization + base64 + rasterization on change | Direct tree operations |
| **Reflows** | Minimum 2 reflows for first paint | Single reflow |

## Key Files Reference

### Data Flow Chain
```
script/dom/node/node.rs:2377
    → Node::svg_data()
    → downcast<SVGSVGElement>.data()
    → SVGElementData

layout/dom.rs:378
    → NodeExt::as_svg()
    → self.svg_data()

layout/replaced.rs:185
    → ReplacedContents::for_element()
    → as_svg() → svg_kind_size()

shared/layout/lib.rs:152
    → SVGElementData struct
    → ratio_from_view_box()

layout/replaced.rs:474
    → make_fragments()
    → SVGElement handling (rasterize → ImageFragment)

layout/context.rs:240
    → ImageResolver::queue_svg_element_for_serialization()
    → ImageResolver::rasterize_vector_image()

layout/layout_impl.rs:1016
    → Collect pending_svg_elements_for_serialization

shared/layout/lib.rs:614
    → ReflowResult::pending_svg_elements_for_serialization

script/dom/window.rs:3583
    → handle_pending_images_post_reflow()
    → serialize_and_cache_subtree() + dirty()
```
