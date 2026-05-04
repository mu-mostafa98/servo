# SVG Layout Classes and Methods - Detailed Documentation

## Overview
This document provides a comprehensive breakdown of all classes and methods in Servo's layout system that handle SVG elements. SVG is treated as a **replaced element** alongside images, iframes, canvases, and videos.

## File Structure
```
components/shared/layout/       (shared between script and layout threads)
├── lib.rs                      # SVGElementData struct, ReflowResult, LayoutElementType
├── layout_node.rs              # LayoutNode trait with svg_data()
├── layout_element.rs           # LayoutElement trait

components/layout/              (layout thread)
├── replaced.rs                 # ReplacedContents, SVG handling logic
├── context.rs                  # ImageResolver, SVG serialization queue
├── layout_impl.rs              # Reflow orchestration, pending SVGs collection
├── dom.rs                      # NodeExt trait, as_svg()
├── fragment_tree/
│   ├── fragment_tree.rs        # FragmentTree top-level
│   └── fragment.rs             # Fragment enum, ImageFragment struct
├── display_list/mod.rs         # Display list generation for SVG
└── style_ext.rs                # Style extensions (clip-path)

components/script/layout_dom/   (script thread)
├── servo_layout_node.rs        # Script's LayoutNode impl
```

---

## File: shared/layout/lib.rs

### Struct: `SVGElementData<'dom>`

**Purpose:** Carries SVG element data from the DOM (script thread) to the layout system. Contains the serialized SVG data URL and dimension information.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Option<Result<ServoUrl, ()>>` | The SVG's XML source as a base64-encoded `data:` URL. `None` = not serialized yet, `Err(())` = serialization failed |
| `width` | `Option<&'dom AttrValue>` | SVG `width` attribute value |
| `height` | `Option<&'dom AttrValue>` | SVG `height` attribute value |
| `svg_id` | `String` | UUID string for caching rasterized images |
| `view_box` | `Option<&'dom AttrValue>` | SVG `viewBox` attribute value (`"min-x min-y width height"`) |

### Methods

#### `impl SVGElementData`

**`pub fn ratio_from_view_box(&self) -> Option<f32>`**
- **Purpose:** Parses the `viewBox` attribute to extract the aspect ratio (width/height)
- **Inputs:** None (self)
- **Output:** `Some(width / height)` if valid, `None` if viewBox is missing or invalid
- **Parsing logic:**
  1. Skip `min-x` (first integer)
  2. Skip `min-y` (second integer)
  3. Parse `width` as unsigned integer, return `None` if zero
  4. Parse `height` as unsigned integer, return `None` if zero
  5. Skip trailing whitespace
  6. Ensure no extra content after the four values
  7. Return `width as f32 / height as f32`
- **Used by:** `ReplacedContents::svg_kind_size()` to determine natural aspect ratio

### Enum: `LayoutElementType`

**Purpose:** Identifies the type of a layout element. SVG types are registered here.

**SVG-related variants:**
- `SVGImageElement` - `<image>` element in SVG namespace
- `SVGSVGElement` - `<svg>` root element

### Struct: `ReflowResult`

**Purpose:** Information derived from a layout pass returned to the script thread.

**SVG-related field:**
- `pending_svg_elements_for_serialization: Vec<UntrustedNodeAddress>` - List of SVGSVGElement nodes that need to be serialized (serialization must happen on the script thread)

---

## File: shared/layout/layout_node.rs

### Trait: `LayoutNode<'dom>`

**Purpose:** Trait exposing DOM nodes to the layout system. SVG-relevant method:

**Method:**
**`fn svg_data(&self) -> Option<SVGElementData<'dom>>`**
- **Purpose:** Returns SVG element data if this node is an SVG element
- **Output:** `Some(SVGElementData)` if this node is an SVGSVGElement, `None` otherwise
- **Implementation:** Delegates to script-side implementation

---

## File: layout/dom.rs

### Trait: `NodeExt<'dom>` (extension methods for ServoLayoutNode)

**Method:**
**`fn as_svg(&self) -> Option<SVGElementData<'dom>>`**
- **Purpose:** Extracts SVGElementData from a layout node for replaced element handling
- **Output:** `Some(SVGElementData)` if the node is an SVGSVGElement
- **Implementation:** `self.svg_data()` (delegates to LayoutNode trait)
- **Called by:** `ReplacedContents::for_element()` to detect SVG elements

---

## File: script/layout_dom/servo_layout_node.rs

### `impl LayoutNode for ServoLayoutNode` (script-side implementation)

**Method:**
**`fn svg_data(&self) -> Option<SVGElementData<'dom>>`**
- **Purpose:** Implements the LayoutNode trait for the script thread
- **Implementation:** `self.node.svg_data()` (delegates to DOM Node)

---

## File: script/dom/node/node.rs

### `impl Node` (DOM node implementation)

**Method:**
**`pub(crate) fn svg_data(self) -> Option<SVGElementData<'dom>>`**
- **Purpose:** Extracts SVGElementData by downcasting this node to SVGSVGElement
- **Output:** `Some(SVGElementData)` if node is an SVGSVGElement
- **Implementation:** `self.downcast::<SVGSVGElement>().map(|svg| svg.data())`
- **Key insight:** Only `SVGSVGElement` (the root `<svg>`) can provide SVG data - individual SVG child elements like `<rect>`, `<circle>`, `<text>` are NOT handled separately

---

## File: layout/replaced.rs

### Struct: `ReplacedContents`

**Purpose:** Represents a replaced element's content (images, iframes, canvases, videos, SVG). Handles size calculation, fragment generation, and aspect ratio.

**Fields:**
- `kind: ReplacedContentKind` - Type of replaced content
- `natural_size: NaturalSizes` - Natural dimensions (width, height, ratio)
- `base_fragment_info: BaseFragmentInfo` - Fragment identification info

### Struct: `NaturalSizes`

**Purpose:** Natural dimensions of a replaced element.

**Fields:**
- `width: Option<Au>` - Natural width in app units
- `height: Option<Au>` - Natural height in app units
- `ratio: Option<CSSFloat>` - Aspect ratio (width/height)

**Methods:**
- `from_width_and_height(width: f32, height: f32) -> Self` - Creates from explicit dimensions, computes ratio
- `from_natural_size_in_dots(size: PhysicalSize<f64>) -> Self` - Creates from image pixel dimensions
- `empty() -> Self` - Creates with no dimensions (for iframes, audio)

### Enum: `ReplacedContentKind`

**Purpose:** Discriminated union of replaced content types.

**Variants:**
- `Image(ImageInfo)` - Raster images (PNG, JPEG, etc.)
- `IFrame(IFrameInfo)` - Inline frames
- `Canvas(CanvasInfo)` - HTML canvas elements
- `Video(VideoInfo)` - Video elements
- **`SVGElement(Option<VectorImage>)`** - SVG elements (vector image or None if not loaded)
- `Audio` - Audio elements

### Enum: `ReplacedContentKind` Methods

#### `impl ReplacedContents`

**`pub fn for_element(node: ServoLayoutNode<'_>, context: &LayoutContext) -> Option<Self>`**
- **Purpose:** Creates ReplacedContents for a given DOM node
- **Inputs:**
  - `node`: Layout node to analyze
  - `context`: Layout context with image resolver
- **Output:** `Some(ReplacedContents)` if the node is a replaced element, `None` otherwise
- **Flow:**
  1. Check for data-attribute-based objects
  2. Check if image element → `ReplacedContentKind::Image`
  3. Check if canvas → `ReplacedContentKind::Canvas`
  4. Check if iframe → `ReplacedContentKind::IFrame`
  5. Check if video → `ReplacedContentKind::Video`
  6. **Check if SVG → calls `svg_kind_size()`**
  7. Check if audio → `ReplacedContentKind::Audio`
  8. Fallback to `content` CSS property
- **SVG handling:** `node.as_svg()` triggers SVGElementData extraction

**`fn svg_kind_size(svg_data: SVGElementData, context: &LayoutContext, node: ServoLayoutNode<'_>) -> (ReplacedContentKind, NaturalSizes)`**
- **Purpose:** Computes the replaced content kind and natural sizes for an SVG element
- **Inputs:**
  - `svg_data`: SVG element data from DOM
  - `context`: Layout context
  - `node`: The SVG layout node
- **Output:** Tuple of (ReplacedContentKind::SVGElement, NaturalSizes)
- **Process:**
  1. Creates a style context to compute width/height attribute values
  2. Parses `width` attribute as `LengthPercentage`, computes to length
  3. Parses `height` attribute as `LengthPercentage`, computes to length
  4. Determines aspect ratio:
     - If both width/height are explicit: `width / height`
     - Otherwise: `ratio_from_view_box()`
  5. Creates `NaturalSizes` with computed dimensions
  6. Handles SVG source:
     - `None` (not serialized): queues element via `queue_svg_element_for_serialization()`
     - `Some(Err(()))`: previous serialization failed, skip
     - `Some(Ok(url))`: looks up cached image via `get_cached_image_for_url()`
  7. If cached image found as `Image::Vector`, assigns `svg_id` to `VectorImage`
  8. Returns `(ReplacedContentKind::SVGElement(vector_image), natural_size)`

**`fn make_fragments(&self, layout_context: &LayoutContext, style: &Arc<ComputedValues>, size: PhysicalSize<Au>) -> Vec<Fragment>`**
- **Purpose:** Generates fragment(s) for this replaced element
- **Inputs:**
  - `layout_context`: Layout context (for DPI scaling, rasterization)
  - `style`: Computed style for the element
  - `size`: Physical size constraint
- **Output:** Vector of Fragment objects
- **SVG-specific logic (ReplacedContentKind::SVGElement):**
  1. If `vector_image` is `None` (not loaded yet), returns empty vec
  2. Sets fragment rect from `vector_image.metadata` (width/height)
  3. **TODO at line 571:** "This is incorrect if the SVG has a viewBox."
  4. Scales by device pixel ratio for rasterization size
  5. Calls `rasterize_vector_image()` to get a `RasterImage`
  6. Creates `Fragment::Image(ImageFragment)` with the resulting `ImageKey`
  7. **Critical:** SVG is always rendered as a raster bitmap, never as vector content

**`fn calculate_fragment_rect(&self, style: &Arc<ComputedValues>, size: PhysicalSize<Au>) -> (PhysicalSize<Au>, PhysicalRect<Au>)`**
- **Purpose:** Calculates the object-fit adjusted fragment rectangle
- **Inputs:**
  - `style`: Computed style (for object-fit, object-position)
  - `size`: Available size
- **Output:** Tuple of (object-fit size, positioned rectangle)
- **Applies:** `object-fit` (fill, contain, cover, none, scale-down) and `object-position`

**`fn content_size(&self, axis: Direction, preferred_aspect_ratio: Option<AspectRatio>, get_size_in_opposite_axis: &dyn Fn() -> SizeConstraint, get_fallback_size: &dyn Fn() -> Au) -> Au`**
- **Purpose:** Computes content size in a given axis using aspect ratio if available

**`fn preferred_aspect_ratio(&self, style: &ComputedValues, padding_border_sums: &LogicalVec2<Au>) -> Option<AspectRatio>`**
- **Purpose:** Returns the preferred aspect ratio, combining natural ratio with CSS `aspect-ratio` property

**`fn fallback_inline_size(&self, writing_mode: WritingMode) -> Au`**
- **Purpose:** Returns the fallback inline size (300px default for horizontal, 150px for vertical)
- **Spec:** CSS Images Module Level 3, default object size

**`fn fallback_block_size(&self, writing_mode: WritingMode) -> Au`**
- **Purpose:** Returns the fallback block size (150px default for horizontal, 300px for vertical)

**`fn logical_natural_sizes(&self, writing_mode: WritingMode) -> LogicalVec2<Option<Au>>`**
- **Purpose:** Returns natural sizes in logical coordinates (inline/block)

**`fn layout(&self, layout_context: &LayoutContext, containing_block_for_children: &ContainingBlock, preferred_aspect_ratio: Option<AspectRatio>, base: &LayoutBoxBase, lazy_block_size: &LazySize) -> IndependentFormattingContextLayoutResult`**
- **Purpose:** Lays out the replaced element
- **Output:** Layout result with fragments

### Trait Implementation: `ComputeInlineContentSizes for ReplacedContents`

**`fn compute_inline_content_sizes(&self, _: &LayoutContext, constraint_space: &ConstraintSpace) -> InlineContentSizesResult`**
- **Purpose:** Computes the inline content sizes for the replaced element
- **Depends on:** Aspect ratio and block constraints

---

## File: layout/context.rs

### Struct: `LayoutContext<'a>`

**Purpose:** Context passed through the layout process, containing shared state.

**SVG-relevant fields:**
- `image_resolver: Arc<ImageResolver>` - Resolves images including SVG rasterization
- `style_context: SharedStyleContext<'a>` - Style system context (for DPI ratio)

### Struct: `ImageResolver`

**Purpose:** Resolves images during box and fragment tree construction. Manages SVG serialization queue, image caching, and vector image rasterization.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `origin` | `ImmutableOrigin` | Document origin for image fetch requests |
| `image_cache` | `Arc<dyn ImageCache>` | Reference to script thread image cache |
| `pending_images` | `Mutex<Vec<PendingImage>>` | In-progress image loads to share with script |
| `pending_rasterization_images` | `Mutex<Vec<PendingRasterizationImage>>` | Vector images needing rasterization |
| `pending_svg_elements_for_serialization` | `Mutex<Vec<UntrustedNodeAddress>>` | SVG elements not yet serialized |
| `animating_images` | `Arc<RwLock<AnimatingImages>>` | Map of nodes with animating images |
| `resolved_images_cache` | `Arc<RwLock<HashMap<ServoUrl, CachedImageOrError>>>` | Cache of resolved image results |
| `animation_timeline_value` | `f64` | Current animation timeline value |

### Methods

#### `impl ImageResolver`

**`pub(crate) fn get_or_request_image_or_meta(&self, node: OpaqueNode, url: ServoUrl, destination: LayoutImageDestination) -> LayoutImageCacheResult`**
- **Purpose:** Checks image cache for image data or metadata, starts loading if needed
- **Inputs:**
  - `node`: The DOM node requesting the image
  - `url`: Image URL
  - `destination`: Where in layout the image is needed
- **Output:** `LayoutImageCacheResult::DataAvailable`, `Pending`, or `LoadError`
- **Behavior:** If image is not cached, creates `PendingImage` and adds to `pending_images` list

**`pub(crate) fn handle_animated_image(&self, node: OpaqueNode, image: Arc<RasterImage>)`**
- **Purpose:** Tracks or removes a node's animated image state
- **Inputs:**
  - `node`: DOM node
  - `image`: Raster image that may be animated
- **Behavior:** If image has multiple frames, adds to animating_images map; otherwise removes it

**`pub(crate) fn get_cached_image_for_url(&self, node: OpaqueNode, url: ServoUrl, destination: LayoutImageDestination) -> Result<CachedImage, ResolveImageError>`**
- **Purpose:** Gets a fully loaded cached image for a URL
- **Inputs:**
  - `node`: DOM node
  - `url`: Image URL (the SVG data URL)
  - `destination`: Layout destination
- **Output:** `Ok(CachedImage)` or `Err(ResolveImageError)`
- **Behavior:** Checks `resolved_images_cache` first, then queries image cache
- **Called by:** `svg_kind_size()` for SVG data URL resolution

**`pub(crate) fn rasterize_vector_image(&self, image_id: PendingImageId, size: DeviceIntSize, node: OpaqueNode, svg_id: Option<String>) -> Option<RasterImage>`**
- **Purpose:** Rasterizes a vector image to a specific pixel size
- **Inputs:**
  - `image_id`: ID of the vector image in cache
  - `size`: Target rasterization size in device pixels
  - `node`: DOM node (for pending tracking)
  - `svg_id`: Optional SVG identifier for caching
- **Output:** `Some(RasterImage)` if available, `None` if pending
- **Behavior:**
  - Calls `image_cache.rasterize_vector_image()` which uses `resvg::render()`
  - If rasterization fails (not ready), adds to `pending_rasterization_images`
- **Called by:** `make_fragments()` for SVG element fragments

**`pub(crate) fn queue_svg_element_for_serialization(&self, element: ServoLayoutNode<'_>)`**
- **Purpose:** Adds an SVG element to the pending serialization queue
- **Inputs:** `element`: Layout node of an SVGSVGElement
- **Behavior:** Pushes node address to `pending_svg_elements_for_serialization`
- **Called by:** `svg_kind_size()` when SVG source is `None` (not serialized)

**`pub(crate) fn resolve_image<'a>(&self, node: Option<OpaqueNode>, image: &'a Image) -> Result<ResolvedImage<'a>, ResolveImageError>`**
- **Purpose:** Resolves a CSS image value to a concrete image or gradient
- **Inputs:**
  - `node`: Optional DOM node
  - `image`: CSS Image value (URL, gradient, image-set, etc.)
- **Output:** `Ok(ResolvedImage)` or `Err(ResolveImageError)`

---

## File: layout/layout_impl.rs

### Reflow Processing (SVG-relevant section)

**Purpose:** Orchestrates the full reflow process including SVG element handling.

**SVG-relevant code (lines 979-1026):**
1. Creates `ImageResolver` with empty `pending_svg_elements_for_serialization`
2. Runs restyle, build trees, calculate overflow, build stacking context tree, build display list
3. **After layout phases complete:**
   ```rust
   let pending_svg_elements_for_serialization =
       std::mem::take(&mut *image_resolver.pending_svg_elements_for_serialization.lock());
   ```
4. Includes pending SVGs in `ReflowResult` returned to script thread

---

## File: layout/fragment_tree/fragment.rs

### Enum: `Fragment`

**SVG-relevant variant:**
- `Image(ArcRefCell<ImageFragment>)` - SVG always produces Image fragments

### Struct: `ImageFragment`

**Purpose:** Represents an image (including rasterized SVG) in the fragment tree.

**Fields:**
- `base: BaseFragment` - Base fragment data (rect, style, node info)
- `clip: PhysicalRect<Au>` - Clipping rectangle
- `image_key: Option<ImageKey>` - WebRender image key for rendering
- `showing_broken_image_icon: bool` - Whether to show broken image placeholder
- `url: Option<ServoUrl>` - Source URL (None for SVG since it uses data URL)

---

## File: layout/display_list/mod.rs

### Display List SVG Handling

**Purpose:** Generates WebRender display list items from fragments.

**SVG-relevant code (search for `svg_id`):**
- When building display list items for images, checks for `svg_id` on VectorImage
- Uses `svg_id` to evict and re-rasterize when SVG content changes

---

## File: layout/style_ext.rs

### Style Extensions (SVG-relevant)

**Purpose:** Provides extension methods on computed values for layout-specific needs.

**SVG-relevant sections:**
- Clip-path handling: `if self.get_svg().clip_path != ClipPath::None` (line 770)
- Will-change bits: SVG-specific will-change handling (line 864)
- SVG paint server elements and renderable elements handling (line 538 comment)

---

## Complete Data Flow Diagram

```
Layout Node (ServoLayoutNode)
  │
  ├── NodeExt::as_svg()                          [dom.rs:378]
  │     └── LayoutNode::svg_data()                [layout_node.rs:197]
  │           └── Node::svg_data()                [node.rs:2377]
  │                 └── SVGSVGElement::data()      [svgsvgelement.rs:172]
  │                       └── SVGElementData       [shared/layout/lib.rs:152]
  │
  ├── ReplacedContents::for_element()             [replaced.rs:149]
  │     └── svg_kind_size(svg_data)               [replaced.rs:221]
  │           ├── Parse width/height from attributes
  │           ├── Compute natural_size
  │           ├── If source is None:
  │           │     └── queue_svg_element_for_serialization()
  │           │           └── ImageResolver.pending_svg_elements_for_serialization
  │           │                 [context.rs:240]
  │           └── If source is Ok(url):
  │                 └── get_cached_image_for_url()
  │                       └── ImageResolver.resolved_images_cache
  │                             [context.rs:181]
  │
  ├── ReplacedContents::layout()                  [replaced.rs:688]
  │     └── make_fragments()                      [replaced.rs:474]
  │           └── ReplacedContentKind::SVGElement  [replaced.rs:566]
  │                 ├── rasterize_vector_image()   [context.rs:218]
  │                 │     └── image_cache.rasterize_vector_image()
  │                 │           └── resvg::render()  [net/image_cache.rs:1035]
  │                 └── Fragment::Image(ImageFragment)
  │
  └── Post-Reflow:                                 [layout_impl.rs:1016]
        └── pending_svg_elements_for_serialization
              └── ReflowResult                       [shared/layout/lib.rs:603]
                    └── → Script thread
                          └── serialize_and_cache_subtree()
                                └── → Dirty node → Second reflow!
```

## Key Patterns and Observations

### 1. SVG as Replaced Element
SVG is treated identically to images in the replaced element system. The same `ImageFragment` type is used for both raster images and SVG.

### 2. Two-Reflow Requirement
Every inline SVG requires at least two layout passes:
1. First pass discovers SVG needs serialization
2. Script serializes and dirties node
3. Second pass loads and renders the serialized data URL

### 3. Rasterization at Fixed Size
SVG is rasterized at the size determined by layout, using `resvg::render()`. This means:
- The vector data is rasterized to a pixel grid
- CSS transforms on the SVG element scale the bitmap, not the vector
- Device pixel ratio affects rasterization quality

### 4. viewBox Issue
The `TODO` at line 571 in [replaced.rs](components/layout/replaced.rs:571) indicates that viewBox handling is incorrect. Currently, the SVG's rendered size comes from `vector_image.metadata` (which may not match the CSS-specified size).

### 5. Limited SVG Element Support
Only `SVGSVGElement` (root `<svg>`) and `SVGImageElement` are registered in `LayoutElementType`. Individual SVG child elements (rect, circle, text, path) are not recognized by the layout system - they are only present in the serialized XML.

### 6. No SVG-Specific Fragment Type
There is no `Fragment::Svg` variant. All SVG content produces `Fragment::Image` after rasterization. This is the fundamental architectural limitation.

### 7. Image Cache Dependency
SVG rendering depends entirely on the image cache system:
- Serialized SVG → data URL → image cache lookup
- Image cache returns `VectorImage` (parsed SVG tree)
- VectorImage must be rasterized to get `ImageKey` for WebRender

### 8. CSS Object-Fit Applies to SVG
Since SVG is treated as an image, CSS properties like `object-fit` and `object-position` apply to SVG content, controlling how the rasterized SVG fits within its CSS box.
