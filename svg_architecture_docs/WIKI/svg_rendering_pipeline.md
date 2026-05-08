# SVG Rendering Pipeline — Organization by Passes

> A complete walkthrough of the SVG rendering pipeline organized by the actual rendering update passes. Each pass represents one full `update_the_rendering()` → `reflow()` cycle. Bridges describe work that happens between passes (post-reflow hooks or async callbacks).
>
> **Test case:**
> ```html
> <svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
>   <circle cx="100" cy="100" r="50" fill="blue"/>
> </svg>
> ```
>
> All concrete values are derived from this test case.

---

## Table of Contents

1. [Pre-Pass — DOM Construction & Style Computation](#pre-pass--dom-construction--style-computation)
2. [Pass 1 — Layout: source=None → Queue Serialization + Post-Reflow Serialization](#pass-1--layout-sourcenone--queue-serialization--post-reflow-serialization)
3. [Pass 2 — Layout: Image Cache Wait](#pass-2--layout-image-cache-wait)
4. [Pass 2 → Pass 3 Bridge — VectorImage Loading Complete](#pass-2--pass-3-bridge--vectorimage-loading-complete)
5. [Pass 3 — Layout: VectorImage Available + Rasterization](#pass-3--layout-vectorimage-available--rasterization)
6. [Pass 3 → Pass 4 Bridge — Rasterization Complete](#pass-3--pass-4-bridge--rasterization-complete)
7. [Pass 4 — Layout: Fragment::Image → Display List](#pass-4--layout-fragmentimage--display-list)
8. [Reflow Trigger Summary](#8-reflow-trigger-summary)
9. [Key Values Table](#9-key-values-table)

---

## Pre-Pass — DOM Construction & Style Computation

This is the initial setup that happens before any SVG-specific reflow occurs. The DOM tree is built, attributes are parsed, and CSS styles are computed.

### Stage 1.1–1.6: HTML Parser → DOM Construction (Script Thread)

**Purpose:** Build the SVG DOM tree with a unique `uuid`, parse and store `width`/`height`/`viewBox` attributes.

**Key functions:**

| Function | File | Purpose |
|----------|------|---------|
| `create_element()` | `components/script/dom/create.rs` | Dispatches element creation by namespace; routes `ns!(svg)` to SVG path |
| `SVGSVGElement::new()` | `components/script/dom/svg/svgsvgelement.rs` | Allocates `SVGSVGElement` with `Uuid::new_v4()` |
| `parse_plain_attribute()` | `components/script/dom/svg/svgsvgelement.rs` | Parses `width`/`height` into `LengthPercentage` via CSS parser |
| `InsertBefore()` | `components/script/dom/node.rs` | Inserts `<svg>` into the document DOM tree |

**Concrete values after construction:**

```rust
SVGSVGElement {
    uuid: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    cached_serialized_data_url: DomRefCell::new(None),
}

// width="200" → parsed by CSS parser:
AttrValue::LengthPercentage(
    "200".to_owned(),
    Some(LengthPercentage::Length(LengthValue::Px(200.0)))
)

// height="200" → same:
AttrValue::LengthPercentage(
    "200".to_owned(),
    Some(LengthPercentage::Length(LengthValue::Px(200.0)))
)

// viewBox="0 0 200 200" → stored as string:
AttrValue::String("0 0 200 200".to_owned())
```

### Stage 1.7: Style Computation (Layout Thread)

**Purpose:** Compute CSS properties via Stylo (parallel CSS engine) — resolves `display`, `width`, `height` for each element.

**Key functions:**

| Function | File | Purpose |
|----------|------|---------|
| `handle_reflow()` | `components/layout/layout_impl.rs` | Entry point for layout processing |
| `restyle_and_build_trees()` | `components/layout/layout_impl.rs` | Iterates dirty elements, resolves CSS cascade |
| `ServoDangerousStyleElement::match_element()` | `components/layout/style_ext.rs` | Matches CSS selectors → produces `ComputedValues` |

**Concrete values:**

```rust
// For <svg> element:
ElementData.styles = Arc<ComputedValues> {
    display: Display::Inline,
    width: LengthPercentageOrAuto::LengthPercentage(
        LengthPercentage::Length(LengthValue::Px(200.0))
    ),
    height: LengthPercentageOrAuto::LengthPercentage(
        LengthPercentage::Length(LengthValue::Px(200.0))
    ),
}

// For <circle> child:
ElementData.styles = Arc<ComputedValues> {
    display: Display::Inline,
    // No explicit width/height → defaults
}
```

> **Note:** Styles are computed once and cached (`Arc::clone()` on subsequent passes). The style cache persists until a mutation invalidates it.

---

## Pass 1 — Layout: source=None → Queue Serialization + Post-Reflow Serialization

**Rendering Update:** 1
**Trigger:** Normal HTML spec "update the rendering" step (DOM is ready)
**Thread:** Layout Thread
**`svg_kind_size()` behavior:** `source=None` → queues SVG for serialization, produces no image fragment

### Stage 2: Layout Traversal + Replaced Content

**Purpose:** Walk the DOM tree, classify each element's display type. For SVG elements, dispatch as replaced content and compute natural dimensions.

#### 2.1: traverse_element() — DOM Traversal

**Purpose:** Classify display behavior; route SVG into the replaced content pipeline.

**Key functions:**

| Function | Purpose |
|----------|---------|
| `traverse_element()` | Top-level traversal; reads computed `display` property per element |
| `Display::from()` | Converts computed display → `None` / `Contents` / `GeneratingBox` |
| `Contents::for_element()` | Checks if element is replaced; for SVG returns `ReplacedContentKind::SVGElement` |
| `handle_element()` | Generates layout boxes from traversal output |

**Operations:**
1. `traverse_element()` reads computed `display: inline` for `<svg>`
2. `Contents::for_element()` identifies SVG as replaced content
3. Children recurse; `<circle>` gets a regular element box

**Concrete values:**

```rust
ReplacedContentKind::SVGElement {
    vector_image: None,      // no image source yet
    has_viewbox: true,       // viewBox="0 0 200 200" present
}

Display::from(computed_display) → GeneratingBox::Inline
```

#### 2.2: svg_kind_size() — P1 (First Layout)

**Purpose:** Compute natural dimensions from SVG attributes. Since `source=None` (no cached data URL yet), queue the SVG element for serialization in the script thread.

**Key functions:**

| Function | File | Purpose |
|----------|------|---------|
| `svg_kind_size()` | `components/layout/replaced.rs:231` | Central sizing function; branches on `source` field |
| `SVGElementData::data()` | `components/script/dom/svg/svgsvgelement.rs:182` | Constructs data struct from DOM node via `LayoutDom` borrow |
| `queue_svg_element_for_serialization()` | `components/layout/context.rs:240` | Enqueues SVG's node address for post-reflow serialization |

**Operations:**
1. `SVGElementData::data()` reads `uuid`, `width`, `height`, `viewBox`, `source` from the DOM node via unsafe `LayoutDom` borrow
2. `source = None` — no serialization has happened yet
3. Computes natural sizes from width/height attributes
4. Calls `queue_svg_element_for_serialization()` — stores node address in `pending_svg_elements_for_serialization` (a `Mutex<Vec<UntrustedNodeAddress>>`)
5. Returns `ReplacedContentKind::SVGElement { vector_image: None, has_viewbox }`

**Concrete values:**

```rust
// SVGElementData constructed from DOM:
SVGElementData {
    svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    source: None,  // ← no data URL yet — this is the key flag
    width:  Some(AttrValue::LengthPercentage("200", Some(Px(200.0)))),
    height: Some(AttrValue::LengthPercentage("200", Some(Px(200.0)))),
    view_box: Some(AttrValue::String("0 0 200 200")),
}

// Natural sizes:
NaturalSizes {
    width:  Au(12000),   // 200px × 60 Au/px
    height: Au(12000),   // 200px × 60 Au/px
    ratio:  1.0,
}

// Returned to fragment builder:
ReplacedContentKind::SVGElement {
    vector_image: None,
    has_viewbox: true,
}
// → No Fragment::Image produced this pass
```

### Post-Reflow Serialization (within the same Window::reflow() call)

**Location:** Within `Window::reflow()`, after layout returns — not a separate rendering update
**Thread:** Script Thread
**`svg_kind_size()`:** Does NOT run here (this is post-reflow processing, not a layout pass)

#### Stage 3: SVG Serialization

Serialization of the SVG DOM subtree into a cached data URL. Runs as a post-reflow hook on the Script Thread after layout returns.

**Key functions:**

| Function | File | Line | Purpose |
|----------|------|------|---------|
| `handle_pending_images_post_reflow()` | `window.rs` | 3521 | Post-reflow hook; drains pending list, dispatches serialization |
| `serialize_and_cache_subtree()` | `svgsvgelement.rs` | 81 | Top-level serialization orchestrator |
| `process_use_elements()` | `svgsvgelement.rs` | 115 | Clones `<use>`-referenced subtrees before serialization |
| `xml_serialize()` | `xml5ever` (Node impl) | — | Walks DOM subtree → XML string |
| `base64::encode()` | `base64` crate | — | Encodes XML bytes to base64 |
| `cleanup_cloned_nodes()` | `svgsvgelement.rs` | 163 | Removes temporary `<use>` clones after serialization |

**Sub-stages:**

**3.1 — Dispatch:** `handle_pending_images_post_reflow()` drains `pending_svg_elements_for_serialization` and calls `serialize_and_cache_subtree()` for each queued SVG node.

```
layout.borrow_mut().reflow(reflow)    ← svg_kind_size() runs here, queues SVG
    ↓
handle_pending_images_post_reflow()    ← post-reflow hook
    ├── pending_svg_elements_for_serialization → iterate
    └── serialize_and_cache_subtree()  ← 3.2
```

**3.2 — Serialize & Cache:** `serialize_and_cache_subtree()` produces the data URL.

1. `process_use_elements()` — scans for `<use>` (none in test case, no-op)
2. `xml_serialize(TraversalScope::IncludeNode)` — produces XML string
3. `base64::engine::general_purpose::STANDARD.encode()` — base64-encodes the XML
4. `ServoUrl::parse()` — wraps as `data:image/svg+xml;base64,...`
5. Stores in `cached_serialized_data_url = Some(Ok(url))`
6. `cleanup_cloned_nodes()` — removes temporary `<use>` clones

**3.3 — Dirty → Schedule:** `node.dirty(NodeDamage::Other)` flags next rendering update.

**Concrete values:**

```rust
// XML serialization output (~231 bytes):
let xml_source = "\
<svg xmlns=\"http://www.w3.org/2000/svg\" \
     width=\"200\" height=\"200\" \
     viewBox=\"0 0 200 200\">\
  <circle cx=\"100\" cy=\"100\" r=\"50\" fill=\"blue\"/>\
</svg>";

// Base64 encoded (~334 chars):
let base64 = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmci\
IHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAg\
MjAwIDIwMCI+PGNpcmNsZSBjeD0iMTAwIiBjeT0iMTAwIiByPSI1\
MCIgZmlsbD0iYmx1ZSIvPjwvc3ZnPg==";

// Cached on SVGSVGElement:
*self.cached_serialized_data_url.borrow_mut() = Some(Ok(
    ServoUrl::parse("data:image/svg+xml;base64,...").unwrap()
));

// Dirty flag → triggers next rendering update:
node.dirty(NodeDamage::Other);
```

### Pass 1 Summary (layout + post-reflow)

| Attribute | Value |
|-----------|-------|
| **Trigger** | Normal rendering update (HTML spec step) |
| **`svg_kind_size()`** | Runs: `source=None` → queues SVG for serialization |
| **Post-reflow hook** | `handle_pending_images_post_reflow()` → serializes, stores data URL |
| **Fragment produced?** | ❌ No — `vector_image=None` |
| **Key output** | `cached_serialized_data_url = Some(Ok(data:...))` |
| **Next trigger** | `dirty(NodeDamage::Other)` → `needs_rendering_update() = true` → schedule Pass 2 |

---

## Pass 2 — Layout: Image Cache Wait

**Rendering Update:** 2
**Trigger:** `dirty(NodeDamage::Other)` set post-reflow → timer fires → `update_the_rendering()` re-enters
**Thread:** Layout Thread + Image Cache Thread (async)
**`svg_kind_size()` behavior:** `source=Some(url)` → cache returns `Pending` → no VectorImage yet

### Stage 4: Cache Wait

**Purpose:** Re-enter layout with the data URL available. Request the image from the image cache; it hasn't loaded yet → cache miss → triggers async load.

#### 4.1: svg_kind_size() — P2 (Cache Wait)

**Purpose:** Attempt to retrieve the image from cache using the data URL. Returns `Pending` → `vector_image = None` → no fragment produced.

**Key functions:**

| Function | File | Line | Purpose |
|----------|------|------|---------|
| `svg_kind_size()` | `replaced.rs` | 231 | Called again; `source=Some(url)` branches to cache lookup |
| `get_cached_image_for_url()` | `layout/context.rs` | 181 | Queries image cache by URL |
| `SVGElementData::data()` | `svgsvgelement.rs` | 182 | Reads `cached_serialized_data_url = Some(Ok(url))` |

**Operations:**
1. Style restyled due to `NodeDamage::Other` — no actual style changes (just reconstruction)
2. `SVGElementData::data()` reads `source = Some(Ok(url))` — the data URL from Stage 3 serialization
3. `svg_kind_size()` calls `get_cached_image_for_url(data_url)`
4. Image cache hasn't loaded the URL yet → returns `Err(Pending(PendingImageId(1)))`
5. `svg_source = Some(url)` but `cached_image` is `Err` → `vector_image = None`
6. Returns `ReplacedContentKind::SVGElement { vector_image: None, ... }`

**Concrete values:**

```rust
// SVGElementData now has source=Some(url):
SVGElementData {
    svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    source: Some(Ok(ServoUrl("data:image/svg+xml;base64,..."))),
    // width, height, view_box unchanged
}

// get_cached_image_for_url() → Pending:
Err(ImageCacheErr::Pending(PendingImageId(1)))

// svg_kind_size returns:
ReplacedContentKind::SVGElement {
    vector_image: None,
    has_viewbox: true,
}
// → No Fragment::Image — make_fragments returns vec![]
```

#### 4.2: Async VectorImage Load (Image Cache Thread)

**Purpose:** Fetch the data URL, decode the SVG XML into a `usvg::Tree`, store as `VectorImage`.

**Key functions:**

| Function | Location | Purpose |
|----------|----------|---------|
| `service_thread()` | Image cache | Processes image load requests on background thread |
| `complete_load()` | Image cache | Handles loaded data; for SVG stores `usvg::Tree` as VectorImage |

**Operations:**
1. Image cache thread fetches the `data:image/svg+xml;base64,...` URL
2. Decodes base64 → XML bytes
3. `usvg::Tree::from_xml()` parses XML into SVG tree
4. Stores as `LoadedVectorImage` in `vector_images` map keyed by `PendingImageId(1)`
5. Stores `svg_id → PendingImageId` mapping
6. Fires notification → pipeline schedules next rendering update

**Concrete values:**

```rust
// Image cache stores:
vector_images: HashMap<PendingImageId, LoadedVectorImage> = {
    PendingImageId(1) => LoadedVectorImage {
        svg_tree: usvg::Tree { /* parsed <svg> + <circle> */ },
        metadata: ImageMetadata { width: 200, height: 200 },
        svg_id: Some("b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()),
    }
}

// Lookup mapping:
svg_to_image_id: HashMap<String, PendingImageId> = {
    "b3c8d2f4-..." => PendingImageId(1),
}
```

### Pass 2 Summary

| Attribute | Value |
|-----------|-------|
| **Trigger** | `NodeDamage::Other` from serialization bridge |
| **Style recomputed?** | Yes — triggered by dirty flag (but values are same, cached) |
| **`svg_kind_size()`** | Runs: `source=Some(url)` → cache returns `Pending(PendingImageId(1))` |
| **Fragment produced?** | ❌ No — `vector_image=None` |
| **Async work started** | Image cache loads data URL → `usvg::Tree` → VectorImage |

---

## Pass 2 → Pass 3 Bridge — VectorImage Loading Complete

**Trigger:** Image cache `complete_load()` fires notification after VectorImage is stored

**Flow:**

```
Image Cache Thread                    Script Thread
    │                                      │
    ├─ complete_load()                     │
    │   VectorImage stored                 │
    │   (usvg::Tree parsed)                │
    │                                      │
    ├─ fire notification ───────────────►  │
    │   pending_layout_image_              │
    │   notification(response)             │
    │                                      │
    │                                      ├─ needs_rendering_update() → true
    │                                      ├─ schedule timer
    │                                      ├─ ...
    │                                      ├─ update_the_rendering()
    │                                      │   → reflow() → Pass 3
```

**Key mechanism:** `register_image_cache_listener()` (at `window.rs:3540`) sets up a callback that calls `pending_layout_image_notification()`. This flags the document as needing a rendering update, which schedules the next reflow.

---

## Pass 3 — Layout: Vector Cache Hit + Rasterization Request

**Rendering Update:** 3
**Trigger:** Image cache notification → `needs_rendering_update()` → `update_the_rendering()` re-enters
**Threads:** Layout Thread + Image Cache Thread
**`svg_kind_size()` behavior:** `source=Some(url)` → `DataAvailable` → `vector_image=Some` → `make_fragments()` calls `rasterize_vector_image()`

### Stage 5: Vector Cache Hit + Rasterization Request

**Purpose:** The image cache now has the VectorImage loaded. `svg_kind_size()` gets `DataAvailable` → `vector_image = Some`. Then `make_fragments()` calls `rasterize_vector_image()` → cache miss → async rasterization started.

**Key functions:**

| Function | File | Purpose |
|----------|------|---------|
| `svg_kind_size()` | `replaced.rs` | `source=Some(url)` → `get_cached_image_for_url()` → `Ok(DataAvailable(...))` |
| `get_cached_image_for_url()` | `layout/context.rs` | Returns `DataAvailable` with the `Arc<VectorImage>` |
| `make_fragments()` | `components/layout/replaced.rs` | Constructs `Fragment::Image` from `SVGElement(Some(vector_image))` |
| `rasterize_vector_image()` | Image cache | Checks rasterized cache; on miss, starts async rasterization; returns `None` |

**Sub-stages:**

**5.1 — Vector Cache Hit:** `svg_kind_size()` reads `source=Some(url)` → cache returns `DataAvailable(VectorImage{...})` → `vector_image = Some(ReplacedVectorImage)`.

```rust
// get_cached_image_for_url() returns DataAvailable:
Ok(Arc::new(ImageResource::DataAvailable(VectorImage {
    id: PendingImageId(1),
    metadata: ImageMetadata { width: 200, height: 200 },
    svg_tree: usvg::Tree { /* parsed SVG tree */ },
    svg_id: Some("b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()),
})))

// vector_image is Some:
ReplacedVectorImage { id: PendingImageId(1), metadata: { 200, 200 }, svg_id: "b3c8d2f4-..." }

// svg_kind_size returns:
ReplacedContentKind::SVGElement { vector_image: Some(...), has_viewbox: true }
```

**5.2 — Rasterization Request:** `make_fragments()` receives `SVGElement(Some(vector_image))`. Sets `base.rect` from metadata (`Au(12000) × Au(12000)`). Calls `rasterize_vector_image(PendingImageId(1), (200,200))` → **cache miss**.

1. Image cache checks rasterized cache → miss (not rasterized yet)
2. Spawns thread pool task: `resvg::render()` → `tiny_skia::Pixmap(200×200)` → `RasterImage`
3. Returns `None` to layout (async work in progress)
4. Post-reflow: registers a rasterization completion listener

```rust
// rasterize_vector_image() returns None (async miss):
let result: Option<RasterImage> = None;

// → No image_key → no Fragment::Image yet
```

### Pass 3 Summary

| Attribute | Value |
|-----------|-------|
| **Trigger** | VectorImage loaded notification |
| **Stage 5** | Vector Cache Hit (DataAvailable → `vector_image=Some`) + Rasterization Request (cache miss) |
| **`rasterize_vector_image()`** | **Miss** → returns `None` (async work in progress) |
| **Fragment produced?** | ❌ Not yet — rasterization in progress |
| **Async work started** | `resvg::render()` → `tiny_skia::Pixmap` → `RasterImage` stored in keycache |

---

## Pass 3 → Pass 4 Bridge — Rasterization Complete

**Trigger:** `add_rasterization_complete_listener()` callback fires after `RasterImage` is stored in keycache

**Flow:**

```
Image Cache Thread                    Script Thread
    │                                      │
    ├─ rasterization complete              │
    │   RasterImage stored in keycache     │
    │   ImageKey(IdNamespace(1), 91)       │
    │                                      │
    ├─ callback ────────────────────────►  │
    │   image_cache_sender.send(response)  │
    │                                      │
    │                                      ├─ needs_rendering_update() → true
    │                                      ├─ schedule timer
    │                                      ├─ update_the_rendering()
    │                                      │   → reflow() → Pass 4
```

**Key mechanism:** `window.rs:3566-3573` registers a `Box::new(move |response| { let _ = image_cache_sender.send(response); })` callback. When the image cache finishes rasterizing, it sends a response that triggers the next rendering update.

---

## Pass 4 — Layout: Raster Cache Hit + Fragment::Image

**Rendering Update:** 4
**Trigger:** Rasterization completion notification → `needs_rendering_update()` → `update_the_rendering()`
**Thread:** Layout Thread
**`svg_kind_size()` behavior:** Same as Pass 3 — `source=Some(url)` → `vector_image=Some`
**`make_fragments()` behavior:** `rasterize_vector_image()` → **cache HIT** → `RasterImage` with `ImageKey` → `Fragment::Image`

### Stage 6: Raster Cache Hit + Fragment::Image

**Purpose:** `svg_kind_size()` produces the same result as Pass 3 (`vector_image=Some`). But this time `rasterize_vector_image()` **hits** the rasterized cache → `RasterImage` with `ImageKey` returned → `Fragment::Image` constructed → display list complete.

**Key functions:**

| Function | File | Purpose |
|----------|------|---------|
| `svg_kind_size()` | `replaced.rs` | Same as Pass 3 — `DataAvailable` → `vector_image=Some` |
| `make_fragments()` | `replaced.rs` | Constructs `Fragment::Image` from `SVGElement(Some(vector_image))` |
| `rasterize_vector_image()` | Image cache | Second call → **cache HIT** → returns `Some(RasterImage { id: Some(ImageKey(1, 91)) })` |
| `push_image()` | Display list builder | Adds image to WebRender display list |

**Operations:**
1. `make_fragments()` receives `ReplacedContentKind::SVGElement(Some(vector_image))`
2. Sets `base.rect` from metadata (`Au(12000) × Au(12000)`)
3. Calls `rasterize_vector_image(PendingImageId(1), (200,200))`
4. Image cache checks rasterized cache → **HIT** → returns `Some(RasterImage { id: Some(ImageKey(1, 91)) })`
5. Extracts `ImageKey(IdNamespace(1), 91)` from `RasterImage.id`
6. Constructs `Fragment::Image { image_key: Some(ImageKey(1, 91)), base.rect: 200×200, ... }`
7. Fragment tree → display list → `push_image(ImageKey(1, 91), rect)` → WebRender

**Concrete values:**

```rust
// rasterize_vector_image() — second call → CACHE HIT:
Some(RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc::new([/* 160,000 RGBA bytes */]),
    id: Some(ImageKey(IdNamespace(1), 91)),  // ← ImageKey assigned!
})

// Extracted image_key → Fragment::Image:
Fragment::Image {
    base: BaseFragment {
        rect: PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) },
        clip: PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) },
        style: Arc::clone(&computed_values),
    },
    image_key: Some(ImageKey(IdNamespace(1), 91)),
    image_rendering: ImageRendering::Auto,
    showing_broken_image_icon: false,
}

// Display list → WebRender → GPU renders a 200×200 blue circle:
display_list.push_image(
    ClipId::root(),
    LayoutRect::new(LayoutPoint { x: 0.0, y: 0.0 }, LayoutSize { width: 200.0, height: 200.0 }),
    ImageKey(IdNamespace(1), 91),
    ImageRendering::Auto,
);
```

### Pass 4 Summary

| Attribute | Value |
|-----------|-------|
| **Trigger** | Rasterization complete notification |
| **Stage 6** | Raster Cache Hit + Fragment::Image constructed |
| **Fragment produced?** | ✅ Yes — `Fragment::Image { image_key: Some(ImageKey(1, 91)), rect: 200×200 }` |
| **Pipeline state** | ✅ Complete — blue circle rendered on screen |

---

## 8. Reflow Trigger Summary

All passes go through the **same entry point**: `ScriptThread::update_the_rendering()` → `Document::update_the_rendering()` → `Window::reflow(UpdateTheRendering)`. The behavior differs based on the state of `cached_serialized_data_url` and the image cache.

| Transition | Trigger | Mechanism | Code Location |
|-----------|---------|-----------|---------------|
| **Initial → Pre-pass** | HTML spec "update the rendering" | rAF timer | `script_thread.rs:1121` |
| **Pre-pass → Pass 1** | DOM ready, styles computed | Normal rendering update | Same entry point |
| **Pass 1 (incl. post-reflow)** | Layout returns → post-reflow serialization → `dirty(NodeDamage::Other)` | Post-reflow hook: `handle_pending_images_post_reflow()` serializes, stores data URL, sets dirty | `window.rs:3521-3591` |
| **Pass 1 → Pass 2** | `dirty(NodeDamage::Other)` → timer fires | `needs_rendering_update()` → schedule timer → re-entry | `script_thread.rs:1302` |
| **Pass 2 → Bridge 2** | Image cache `Pending` → listener registered | `register_image_cache_listener()` sets up callback | `window.rs:3540-3545` |
| **Bridge 2 → Pass 3** | `complete_load()` → VectorImage stored | `pending_layout_image_notification()` flags update needed | Image cache callback |
| **Pass 3 → Bridge 3** | `rasterize_vector_image()` miss → listener registered | `add_rasterization_complete_listener()` sets up callback | `window.rs:3566-3573` |
| **Bridge 3 → Pass 4** | Rasterization complete → `RasterImage` in keycache | `image_cache_sender.send(response)` → schedule update | Image cache callback |

### Full Pass Sequence Diagram

```
Initial DOM Ready
    │
    ├─ update_the_rendering()
    │   └─ reflow() ──────────── PASS 1 (includes post-reflow)
    │       ├─ svg_kind_size(source=None) → queue
    │       └─ handle_pending_images_post_reflow()
    │           ├─ serialize_and_cache_subtree()
    │           └─ dirty(NodeDamage::Other)
    │
    ├─ [timer fires] ─────────── PASS 2
    │   └─ reflow()
    │       ├─ svg_kind_size(source=Some, cache=Pending)
    │       └─ [image cache: VectorImage loaded]
    │
    ├─ [cache notification] ──── PASS 3 (Stage 5)
    │   └─ reflow()
    │       ├─ svg_kind_size(source=Some, cache=DataAvailable)
    │       │   → vector_image=Some
    │       └─ make_fragments()
    │           └─ rasterize_vector_image() → MISS → async
    │
    ├─ [rasterization done] ──── PASS 4 (Stage 6)
    │   └─ reflow()
    │       ├─ svg_kind_size(source=Some, vector_image=Some)
    │       └─ make_fragments()
    │           └─ rasterize_vector_image() → HIT → ImageKey
    │               → Fragment::Image → Display List → GPU
    │
    ▼
Pipeline Stable
```

---

## 9. Key Values Table

### Pre-Pass

| Value | Status | Concrete |
|-------|--------|----------|
| `uuid` | ✅ | `"b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b"` |
| `cached_serialized_data_url` | ✅ `None` | Not serialized yet |
| `width` | ✅ `LengthPercentage` | `Length(Px(200.0))` |
| `height` | ✅ `LengthPercentage` | `Length(Px(200.0))` |
| `viewBox` | ✅ | `"0 0 200 200"` |
| `ElementData.styles` | ✅ | `display: inline`, `width: 200px` |

### Pass 1

| Value | Status | Concrete |
|-------|--------|----------|
| `source` | ✅ `None` → `Some(Ok(url))` | Post-reflow: data URL cached |
| `vector_image` | ✅ `None` | Not available |
| `NaturalSizes` | ✅ | `Au(12000)`, `Au(12000)`, `ratio: 1.0` |
| `ReplacedContentKind` | ✅ | `SVGElement { vector_image: None, has_viewbox: true }` |
| `xml_serialize()` output | ✅ | `"<svg ...><circle .../></svg>"` (~231 bytes) |
| `base64` encoded | ✅ | `"PHN2ZyB4bWxucz0i..."` (~334 chars) |
| `cached_serialized_data_url` | ✅ `None → Some(Ok(...))` | `ServoUrl("data:image/svg+xml;base64,...")` |
| `NodeDamage` | ✅ | `NodeDamage::Other` set |
| `Fragment::Image` | ❌ | Not produced |

### Pass 2

| Value | Status | Concrete |
|-------|--------|----------|
| `source` | ✅ `Some(Ok(url))` | Data URL available |
| `cached_image` | ❌ `Err(Pending)` | `PendingImageId(1)` — not loaded yet |
| `vector_image` | ❌ `None` | Still unavailable |
| `Fragment::Image` | ❌ | Not produced |
| `PendingImageId` | ✅ | `PendingImageId(1)` assigned |

### Pass 2 → Pass 3 Bridge

| Value | Status | Concrete |
|-------|--------|----------|
| `LoadedVectorImage.svg_tree` | ✅ | `usvg::Tree` with parsed `<svg>` + `<circle>` |
| `LoadedVectorImage.metadata` | ✅ | `ImageMetadata { width: 200, height: 200 }` |
| `LoadedVectorImage.svg_id` | ✅ | `"b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b"` |
| `svg_to_image_id` mapping | ✅ | `"b3c8d2f4-..." → PendingImageId(1)` |

### Pass 3

| Value | Status | Concrete |
|-------|--------|----------|
| `source` | ✅ `Some(Ok(url))` | Same data URL |
| `cached_image` | ✅ `DataAvailable` | `Ok(Arc::new(DataAvailable(VectorImage{...})))` |
| `vector_image` | ✅ `Some` | `ReplacedVectorImage { id: PendingImageId(1), metadata: 200×200 }` |
| `rasterize_vector_image()` | ❌ Miss — async | Returns `None` (rasterization in progress) |
| `Fragment::Image` | ❌ | Not yet (no image_key) |

### Pass 3 → Pass 4 Bridge

| Value | Status | Concrete |
|-------|--------|----------|
| `tiny_skia::Pixmap` | ✅ | `200 × 200 × 4 = 160,000 bytes RGBA` |
| `resvg::render()` | ✅ | Renders SVG tree → pixmap |
| `RasterImage` | ✅ | `{ metadata: 200×200, format: RGBA8, bytes: 160KB, id: Some(ImageKey(1, 91)) }` |
| `keycache` | ✅ Stored | Key: `(PendingImageId(1), (200,200))` |

### Pass 4

| Value | Status | Concrete |
|-------|--------|----------|
| `source` | ✅ `Some(Ok(url))` | Same data URL |
| `vector_image` | ✅ `Some` | Same as Pass 3 |
| `rasterize_vector_image()` | ✅ **HIT** | `Some(RasterImage { id: Some(ImageKey(1, 91)) })` |
| `base.rect` | ✅ | `PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) }` |
| `image_key` | ✅ | `ImageKey(IdNamespace(1), 91)` |
| `Fragment::Image` | ✅ | `{ image_key: Some(ImageKey(1, 91)), rect: 200×200 }` |
| **Display List** | ✅ | **Blue circle rendered on screen** |
