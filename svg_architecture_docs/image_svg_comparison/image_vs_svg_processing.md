# Image vs SVG Processing in Servo

> How SVG is processed through the same image pipeline as standard raster images,
> and the key differences that arise from this design choice.

---

## 1. Raster Image Pipeline (Standard)

A standard `<img src="photo.png">` follows this path through the system:

```
Source: <img src="photo.png">  (HTML source)
         │
         ▼
  ┌──────────────────┐      Thread: HTML Parser
  │ HTML Parser      │
  │ Parses <img> tag → creates HTMLImageElement
  │ URL extracted from src attribute
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Script (Stylo)
  │ Style Resolution │
  │ Stylo resolves CSS for the element
  │ Computes display type, sizing, object-fit
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Layout
  │ Layout Dispatch  │
  │ ReplacedContentKind::Image(ImageInfo)
  │ NaturalSizes from image metadata (header)
  │ Calls ImageResolver::get_cached_image_for_url(url)
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Image Cache (Net)
  │ ImageCache       │
  │ pending_loads.insert()
  │ Network fetch → bytes arrive
  │ Decode: PNG/JPEG/GIF/WEBP decoder
  │ → RasterImage { bytes, metadata, format }
  │ Notify listeners
  │ KeyCache assigns WebRenderImageKey
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Layout
  │ Fragment Tree    │
  │ make_fragments() → ImageFragment
  │ { image_key: Some(Key) }
  │ Stored in box tree as Fragment::Image
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Layout → WR IPC
  │ Display List     │
  │ push_image(key, clip, bounds)
  │ DisplayItem::Image added to display list
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: WebRender (GPU)
  │ WebRender        │
  │ Compositing → GPU shader → Framebuffer → Screen
  └──────────────────┘
```

**Key objects passed between threads:**

| # | Stage Name | Purpose | Input (Type) | Input Example | Output (Type) | Output Example |
|---|-----------|---------|-------------|---------------|---------------|----------------|
| 1 | **DOM Construction** — `HTML Parser → DOM` | Parse HTML source and construct the image DOM element node | HTML token from `html5ever` | `"<img src=\"photo.png\">"` | `HTMLImageElement` DOM node | `HTMLImageElement { local_name: "img", attrs: { "src": "https://example.com/photo.png" } }` |
| 2 | **Style Resolution & Dispatch** — `Style → Layout` | Resolve CSS, compute replaced-element sizing, dispatch to layout | `ComputedValues` (Stylo CSS engine) | `{ display: block, object_fit: Fill, width: Auto, height: Auto }` | `ReplacedContentKind::Image(ImageInfo)` + `NaturalSizes` | `Image(ImageInfo { width: Some(800px), height: Some(600px) })` |
| 3 | **Image Fetch & Decode** — `Layout → ImageCache` | Request network fetch, decode raw bytes into a raster bitmap | `&ServoUrl` (image URL from `src`) | `"https://example.com/photo.png"` | `RasterImage` (decoded bitmap) | `RasterImage { bytes: Arc<Vec<u8>>(1920000), metadata: ImageMetadata { w:800, h:600 }, format: BGRA8, is_opaque: true, id: None }` |
| 4 | **Image Key Assignment** — `ImageCache → Layout` | Assign a WebRender GPU handle for the decoded bitmap, populate the fragment | `WebRenderImageKey` (via `KeyCache::assign_key()`) | `ImageKey(42)` | `ImageFragment { image_key: Some(key) }` | `ImageFragment { image_key: Some(ImageKey(42)), clip: PhysicalRect { x:0, y:0, w:800au, h:600au }, showing_broken_image_icon: false }` |
| 5 | **Fragment Tree Integration** — `Layout → Fragment Tree` | Store the image fragment in the box tree as a `Fragment::Image` for display list building | `ImageFragment` built in `make_fragments()` | `{ image_key: Some(Key(42)), url: Some(url), clip: ... }` | `Fragment::Image(ArcRefCell<ImageFragment>)` | `Fragment::Image(RefCell { image_key: Some(Key(42)) })` stored in box tree |
| 6 | **Display List Emission** — `Fragment Tree → DisplayList` | Convert the image fragment into a `DisplayItem::Image` command for WebRender | `DisplayItem::Image` via `push_image()` | `push_image(ImageKey(42), clip_rect, bounds)` | WebRender GPU command | `WebRenderCmd::DrawImage(Key(42), transform, clip)` |
| 7 | **GPU Compositing** — `DisplayList → WebRender` | Composite the pre-rasterized bitmap onto the screen via the GPU pipeline | GPU-framebuffer (composited quads) | `Framebuffer { pixels: [u8; 1920000] }` | Screen pixels | Display output rendered on monitor |

---

## 2. SVG Image Pipeline (Current Architecture)

An inline `<svg>` element follows a path that diverges and re-converges:

```
Source: <svg> ... </svg>  (inline HTML source)
         │
         ▼
  ┌──────────────────┐      Thread: HTML Parser
  │ HTML Parser      │
  │ Parses <svg> tag → creates SVGSVGElement
  │ SVG subtree built in DOM tree
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Script (Stylo)
  │ Style Resolution │
  │ Stylo resolves CSS for the element
  │ Computes display type, sizing
  └──────┬───────────┘
         │
         ▼
  ╔══════════════════╗      Thread: Layout → Script → Layout
  ║ DIVERGENT PATH   ║     ─── Two-Reflow Serialization ───
  ╚══════════════════╝
         │
         ▼
  ┌──────────────────┐      Thread: Layout (1st pass)
  │ First Layout     │
  │ SVGElementData { source: None }
  │ No cached data URL → queue serialization
  │ queue_svg_element_for_serialization()
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Script DOM
  │ Serialize Subtree│ ←── EXTRA STAGE (SVG-only)
  │ serialize_and_cache_subtree()
  │ process_use_elements() → resolve <use>
  │ XML serialize DOM → Base64 → data URL
  │ cached_serialized_data_url = Some(url)
  │ Trigger dirty layout
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Layout (2nd pass)
  │ Second Layout    │
  │ SVGElementData { source: Some(data_url) }
  │ ReplacedContentKind::SVGElement(vector_image)
  │ Calls ImageResolver::get_cached_image_for_url(url)
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Image Cache (Net)
  │ ImageCache       │
  │ pending_loads.insert()
  │ data URL → instant base64 decode
  │ usvg::Tree::from_data(bytes) → VectorImageData
  │ Store in store.vector_images[PendingImageId]
  │ Return Image::Vector(VectorImage)
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Thread Pool (async)
  │ CPU Rasterize    │ ←── EXTRA STAGE (SVG-only)
  │ tiny_skia::Pixmap::new(w,h)
  │ resvg::render(&tree, &transform, &mut pixmap)
  │ pixmap.take() → raw RGBA bytes
  │ → RasterImage { bytes, metadata }
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Image Cache → Layout
  │ Complete Load    │
  │ load_image_with_keycache() → WebRenderImageKey
  │ complete_load_svg() → store rasterized result
  │ Notify listeners → ImageFragment updated
  └──────┬───────────┘
         │
         ▼
  ╔══════════════════╗
  ║ RECONVERGE       ║     ─── Same path from here ───
  ╚══════════════════╝
         │
         ▼
  ┌──────────────────┐      Thread: Layout
  │ Fragment Tree    │
  │ make_fragments() → ImageFragment
  │ { image_key: Some(key) }
  │ Stored in box tree as Fragment::Image
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: Layout → WR IPC
  │ Display List     │
  │ push_image(key, clip, bounds)
  │ DisplayItem::Image added to display list
  └──────┬───────────┘
         │
         ▼
  ┌──────────────────┐      Thread: WebRender (GPU)
  │ WebRender        │
  │ Compositing → GPU shader → Framebuffer → Screen
  └──────────────────┘
```

**Key objects passed between threads:**

| # | Stage Name | Purpose | Input (Type) | Input Example | Output (Type) | Output Example |
|---|-----------|---------|-------------|---------------|---------------|----------------|
| 1 | **DOM Construction** — `HTML Parser → DOM` | Parse HTML source and construct the SVG DOM element node | HTML token from `html5ever` | `"<svg viewBox=\"0 0 100 100\">...</svg>"` | `SVGSVGElement` DOM node | `SVGSVGElement { local_name: "svg", attrs: { viewBox: "0 0 100 100" }, children: [circle, rect, ...] }` |
| 2 | **Style Resolution & Dispatch** — `Style → Layout` | Compute CSS styles, dispatch as replaced SVG element (no source yet, first pass) | `ComputedValues` (Stylo CSS engine) | `{ display: block, width: 100px, height: 100px }` | `ReplacedContentKind::SVGElement(Option<VectorImage>)` + `NaturalSizes` | `SVGElement(None)` — no source yet (1st pass) |
| 3 ⚠ | **DOM Serialization Request** — `Layout (1st) → Script` | SVG-only: detect missing source data, queue the SVG subtree for XML serialization | `SVGElementData { source: None }` serialization request | `SVGElementData { source: None, width: 100, height: 100, view_box: Some(ViewBox(0,0,100,100)), svg_id: "uuid-123" }` | `cached_serialized_data_url` (XML → base64 → data URL) | `"data:image/svg+xml;base64,PHN2ZyB2aWV3Qm94PSIwIDAgMTAwIDEwMCI+PC9zdmc+"` |
| 4 ⚠ | **Serialized Source Injection** — `Script → Layout (2nd)` | SVG-only: deliver the serialized data URL back to layout, trigger the second reflow pass | `cached_serialized_data_url` → `SVGElementData { source: Some(url) }` | `SVGElementData { source: Some("data:image/svg+xml;base64,..."), width: 100, height: 100, view_box: Some(ViewBox(0,0,100,100)) }` | `ReplacedContentKind::SVGElement(Some(VectorImage))` | `SVGElement(Some(VectorImage { data: VectorImageData { svg_tree: Arc<usvg::Tree>, cors_status: Some(Insecure) } }))` |
| 5 | **SVG Tree Parsing** — `Layout → ImageCache` | Send data URL to cache, decode base64, parse SVG XML into a `usvg::Tree` | `&ServoUrl` (data URL from `SVGElementData.source`) | `"data:image/svg+xml;base64,PHN2Zy..."` | `VectorImage` / `VectorImageData` (parsed SVG tree) | `VectorImageData { svg_tree: Arc<usvg::Tree { defs, layers, size }>, cors_status: CorsStatus::Insecure }` |
| 6 ⚠ | **CPU Rasterization** — `ImageCache → ThreadPool` | SVG-only: rasterize the parsed SVG tree into a pixel bitmap via `resvg::render()` on the thread pool | `Arc<usvg::Tree>` + rasterization request | `rasterize_vector_image(id=7, Size(800,600), tree.clone(), svg_id="uuid-123")` | `RasterImage` (CPU-rasterized bitmap via `resvg::render()`) | `RasterImage { bytes: Arc<Vec<u8>>(1920000), metadata: { w:800, h:600 }, format: BGRA8, is_opaque: false, id: None }` |
| 7 ⚠ | **Image Key Delivery** — `ImageCache → Layout` | SVG-only: assign a WebRender GPU handle to the rasterized bitmap, notify listeners | `RasterImage` → `load_image_with_keycache()` | `RasterImage { id: None, bytes: [...], metadata: { w:800, h:600 } }` | `WebRenderImageKey` (via `KeyCache`) | `ImageKey(42)` stored in `ImageFragment.image_key = Some(ImageKey(42))` |
| 8 | **Fragment Tree Integration** — `Layout → Fragment Tree` | Store the image fragment in the box tree as `Fragment::Image` for display list building | `ImageFragment` built in `make_fragments()` | `{ image_key: Some(Key(42)), clip: Rect, url: Some(data_url), showing_broken_image_icon: false }` | `Fragment::Image(ArcRefCell<ImageFragment>)` | `Fragment::Image(RefCell { image_key: Some(Key(42)) })` stored in box tree |
| 9 | **Display List Emission** — `Fragment Tree → DisplayList` | Convert the image fragment into a `DisplayItem::Image` command for WebRender | `DisplayItem::Image` via `push_image()` | `push_image(ImageKey(42), clip_rect, bounds)` | WebRender GPU command | `WebRenderCmd::DrawImage(Key(42), transform, clip)` |
| 10 | **GPU Compositing** — `DisplayList → WebRender` | Composite the pre-rasterized bitmap onto the screen via the GPU pipeline | GPU-framebuffer (composited quads) | `Framebuffer { pixels: [u8; 1920000] }` | Screen pixels | Display output rendered on monitor |

---

## 3. Stage-by-Stage Comparison

| Stage | Raster Image | SVG (Current) | Same? |
|-------|-------------|---------------|-------|
| **HTML Parser** | Creates `HTMLImageElement` | Creates `SVGSVGElement` | Different DOM node |
| **Style Resolution** | Stylo: display, sizing, object-fit | Stylo: same properties | **Same engine** |
| **Layout dispatch** | `ReplacedContentKind::Image` | `ReplacedContentKind::SVGElement` | Different variant |
| **Natural size** | From image metadata (header) | From width/height/viewBox | Different source |
| **DOM serialization** | None (URL-based) | XML serialize → base64 → data URL | **SVG extra** |
| **Cache lookup** | `ImageResolver::get_cached_image_for_url()` | Same function | **Same** |
| **Cache storage** | `store.completed_loads` (RasterImage) | `store.vector_images` (VectorImageData) | Different store |
| **Decode/Parse** | PNG/JPEG/GIF/WEBP decoder | `usvg::Tree::from_data()` | Different library |
| **Rasterization** | Part of decode (pixel bytes) | `resvg::render()` thread pool | **SVG extra** |
| **Fragment tree** | `Fragment::Image(ImageFragment)` | Same variant | **Same** |
| **Display item** | `push_image(key)` | Same call | **Same** |
| **GPU rendering** | WebRender compositing | Same | **Same** |

---

## 4. Key Shared Objects

These types are used by BOTH raster image and SVG paths. They form the shared backbone
that makes the "SVG as Image" design possible.

### DOM Node Creation (both go through the HTML parser)

| Aspect | Raster (`<img>`) | SVG (`<svg>`) |
|--------|-------------------|---------------|
| Parser | `html5ever::tree_builder` | Same parser |
| Element creation | `create_element("img")` in `create.rs` | `create_svg_element("svg")` dispatch |
| DOM interface | `HTMLImageElement` | `SVGSVGElement` (extends `SVGGraphicsElement` → `SVGElement` → `Element`) |

### RasterImage (`components/pixels/lib.rs:299-309`)
```rust
pub struct RasterImage {
    pub metadata: ImageMetadata,   // width, height
    pub format: PixelFormat,       // BGRA8 or RGBA8
    pub id: Option<ImageKey>,      // assigned via KeyCache
    pub bytes: Arc<Vec<u8>>,       // raw pixel data (RGBA)
    pub frames: Vec<ImageFrame>,   // animated frame support
    pub cors_status: CorsStatus,
    pub is_opaque: bool,
}
```
**Used by SVG:** After `resvg::render()` produces pixel bytes, they are wrapped in exactly this struct.
RasterImage is the universal currency for bitmap data in Servo's rendering pipeline.

### ImageFragment (`components/layout/fragment_tree/fragment.rs:86-92`)
```rust
pub struct ImageFragment {
    pub base: BaseFragment,
    pub clip: PhysicalRect<Au>,
    pub image_key: Option<ImageKey>,
    pub showing_broken_image_icon: bool,
    pub url: Option<ServoUrl>,
}
```
**Used by SVG:** SVG produces this exact fragment type — `Fragment::Image(ArcRefCell<ImageFragment>)`.
This is the single point of convergence in the box tree.

### Fragment::Image (`components/layout/fragment_tree/fragment.rs`)
```rust
pub enum Fragment {
    Box(ArcRefCell<BoxFragment>),
    Text(ArcRefCell<TextFragment>),
    Image(ArcRefCell<ImageFragment>),  // ← Both raster AND SVG use this
    IFrame(ArcRefCell<IFrameFragment>),
    // ...
}
```
**Used by SVG:** The exact same enum variant — no SVG-specific fragment exists.
The box tree cannot distinguish between a raster image and an SVG.

### DisplayItem::Image (`components/layout/display_list/`)
```rust
// Fragment::Image dispatch → display_list.push_image(image_key, clip_rect, bounds)
// Creates DisplayItem::Image in the built display list
```
**Used by SVG:** The display list item is identical. WebRender receives
a pre-rasterized bitmap and cannot tell it was originally SVG.

---

## 5. Visual Flow: Shared → Divergent → Shared

The overall pipeline structure follows a "shared → divergent → shared" pattern:

```
                   ┌──────────────────────┐
                   │  1. HTML Parser       │  ← SHARED
                   │  Creates DOM element  │
                   └──────────┬───────────┘
                              │
                   ┌──────────▼───────────┐
                   │  2. Style Resolution  │  ← SHARED
                   │  Stylo computes CSS   │
                   └──────────┬───────────┘
                              │
                    ┌─────────┴──────────┐
                    ▼                    ▼
          ┌─────────────────┐  ┌─────────────────────┐
          │ RASTER PATH     │  │ SVG PATH            │  ← DIVERGENT
          │ 3. Layout       │  │ 3. First Layout     │
          │ 4. ImageCache   │  │ 4. Serialize DOM    │
          │ 5. RasterImage  │  │ 5. Second Layout    │
          │                 │  │ 6. ImageCache parse │
          │                 │  │ 7. resvg::render    │
          └────────┬────────┘  └──────────┬──────────┘
                    └──────────┬──────────┘
                              │
                   ┌──────────▼───────────┐
                   │  6/8. Fragment Tree   │  ← RECONVERGED
                   │  ImageFragment        │    SHARED
                   └──────────┬───────────┘
                              │
                   ┌──────────▼───────────┐
                   │  7/9. Display List    │  ← SHARED
                   │  push_image(key)      │
                   └──────────┬───────────┘
                              │
                   ┌──────────▼───────────┐
                   │  8/10. WebRender      │  ← SHARED
                   │  GPU → Screen         │
                   └──────────────────────┘
```

---

## 6. Summary: The "SVG as Image" Design

The current architecture deliberately treats SVG as a **specialized image format** within the existing image pipeline.
Both paths converge at `RasterImage` — after that, the pipeline is identical:

```
                      ┌─────────────────────┐
                      │  Any source produces │
                      │    RasterImage +     │
                      │   Fragment::Image    │
                      │   + push_image(key)  │
                      └──────┬──────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
         Raster Image    SVG (inline)   SVG (<img>)
         PNG/JPEG/GIF   serialize DOM   fetch URL
              │         usvg parse      fetch_usvg parse
              │         resvg render    resvg render
              └──────────────┬──────────────┘
                             ▼
                    WebRender compositing
                         GPU → Screen
```

**The three consequences of this design:**

1. **CSS inheritance breaks** at the data URL boundary — the serialized SVG is a separate document context
2. **Web fonts don't load** — the data URL context has no access to the parent document's fonts
3. **CSS transforms are blurry** — the bitmap is rasterized at a fixed resolution, then transformed
