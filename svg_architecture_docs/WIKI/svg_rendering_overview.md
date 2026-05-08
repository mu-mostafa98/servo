# SVG Rendering Pipeline — Overview

> A high-level walkthrough of how Servo renders inline SVG elements (`<svg>`). The pipeline spans 4 rendering update passes across 3 threads (Script, Layout, Image Cache), involving 6 stages and 2 async bridges.
>
> **Test case:** `<svg width="200" height="200" viewBox="0 0 200 200"><circle cx="100" cy="100" r="50" fill="blue"/></svg>`

---

## Architecture Overview

```
Threads:   Script Thread          Layout Thread        Image Cache Thread
              │                       │                        │
    PASS 1    │  [Stage 1 → 3]        │                        │
              │  DOM + Serialization  │  Layout Traversal      │
              │                       │                        │
    PASS 2    │                       │  [Stage 4]             │  [Async Load]
              │                       │  Cache Wait (Pending)  │  VectorImage
              ├────── Bridge 2→3 ─────┼────────────────────────┤
    PASS 3    │                       │  [Stage 5]             │  [Rasterize]
              │                       │  Vector Hit + Request  │
              ├────── Bridge 3→4 ─────┼────────────────────────┤
    PASS 4    │                       │  [Stage 6]             │
              │                       │  Fragment::Image → GPU |
```

The entire pipeline is driven by 4 sequential `update_the_rendering()` → `reflow()` cycles. Each pass re-enters `svg_kind_size()`, which branches based on the current state of `source` and the image cache.

---

## Stage 1 — DOM Construction & Style Computation (Pre-Pass)

**Pass:** Pre-Pass (initial page load)
**Thread:** Script Thread → Layout Thread
**Purpose:** Build the SVG DOM tree, parse attributes, compute CSS styles.

**What's ready:** Raw HTML bytes from the parser.

| Input | Source |
|-------|--------|
| HTML document | Network / file load |
| CSS stylesheets | Document / UA defaults |

**Call hierarchy:**
```
HTML Parser → create_element(ns!(svg), "svg")
  → SVGElement::new()
    → SVGSVGElement::new_inherited()
      → SVGSVGElement {
            uuid: Uuid::new_v4(),
            cached_serialized_data_url: DomRefCell::new(None),
            width: AttrValue::LengthPercentage("200", Some(Px(200.0))),
            height: AttrValue::LengthPercentage("200", Some(Px(200.0))),
            view_box: AttrValue::String("0 0 200 200"),
        }
```

**Important Data Structures:**

- **`SVGSVGElement`** — The DOM node type representing `<svg>`. Holds parsed attributes (`width`, `height`, `viewBox`) and a serialization cache cell. Lives on the Script Thread; layout accesses it via unsafe `LayoutDom` borrow.
- **`cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl>>>`** — A mutable DOM cell that stores the serialized data URL once produced. Initially `None`. Updated in Stage 3.
- **`ElementData.styles: Arc<ComputedValues>`** — The computed CSS values (produced by Stylo). Contains resolved `display`, `width`, `height` used by layout for sizing.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| `create_element()` | `script/dom/create.rs` | Routes `<svg>` to the SVG element constructor by namespace |
| `SVGSVGElement::new()` | `script/dom/svg/svgsvgelement.rs` | Allocates the SVG element with `Uuid::new_v4()` |
| `parse_plain_attribute()` | `script/dom/svg/svgsvgelement.rs` | Parses `width` / `height` into CSS `LengthPercentage` values |
| `restyle_and_build_trees()` | `layout/layout_impl.rs` | Parallel style computation via Stylo (third-party CSS engine) |

**Output for next stage:**
- `SVGSVGElement` with `uuid`, `width: 200px`, `height: 200px`, `viewBox: "0 0 200 200"`
- `cached_serialized_data_url: None`
- Computed CSS: `display: inline`, explicit `width`/`height`

---

## Stage 2 — Layout Traversal + Queue Serialization (Pass 1)

**Pass:** Pass 1 (rendering update 1)
**Thread:** Layout Thread
**Purpose:** Walk the DOM tree, identify SVG as replaced content, compute natural dimensions, and queue the element for serialization.

**What's ready:** Styled DOM tree from Stage 1. `source=None` (no data URL yet).

| Input | Source |
|-------|--------|
| `cached_serialized_data_url: None` | Stage 1 — not serialized yet |
| `uuid`, `width`, `height`, `viewBox` | Stage 1 — parsed attributes |

**Important Data Structures:**

- **`SVGElementData`** — A snapshot of SVG DOM data, constructed for the layout thread via unsafe `LayoutDom` borrow. Contains `svg_id` (uuid string), `source` (the data URL or None), and parsed `width`/`height`/`view_box`.
- **`ReplacedContentKind::SVGElement`** — An enum variant classifying the SVG as replaced content. Holds `vector_image: Option<ReplacedVectorImage>` (the loaded SVG image, initially None) and `has_viewbox: bool` (whether a `viewBox` attribute was present).
- **`NaturalSizes`** — Computed natural dimensions from CSS/attributes: `width: Au`, `height: Au`, `aspect_ratio: f32`. Au (AppUnit) = 1/60 of a CSS pixel. So 200px = Au(12000).
- **`pending_svg_elements_for_serialization: Vec<UntrustedNodeAddress>`** — A `Mutex`-protected list of SVG node addresses queued for post-reflow serialization. Populated by `queue_svg_element_for_serialization()`.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| `traverse_element()` | `layout/dom_traversal.rs` | Recursive DOM traversal; reads computed `display` per element |
| `Contents::for_element()` | `layout/dom_traversal.rs` | Classifies element as replaced or non-replaced |
| `SVGElementData::data()` | `script/dom/svg/svgsvgelement.rs` | Reads DOM node fields via unsafe `LayoutDom` borrow |
| **`svg_kind_size()`** | **`layout/replaced.rs`** | **Central branching function** — computes natural sizes; branches on `source` state |
| `queue_svg_element_for_serialization()` | `layout/context.rs` | Enqueues SVG node address for post-reflow serialization |

**Key decision in `svg_kind_size()`:**
```rust
// source=None → queue for serialization, no image produced
source: None => {
    queue_svg_element_for_serialization(node);
    vector_image = None  // no image source available yet
}
```

**Third-party:** Stylo (parallel CSS engine, Servo fork of Mozilla's style system)

**Output for next stage:**
- `ReplacedContentKind::SVGElement { vector_image: None, has_viewbox: true }`
- SVG node address stored in `pending_svg_elements_for_serialization`
- No `Fragment::Image` produced this pass

---

## Stage 3 — SVG Serialization (Pass 1, Post-Reflow)

**Pass:** Pass 1 post-reflow hook (same `Window::reflow()` call, no separate rendering update)
**Thread:** Script Thread
**Purpose:** Serialize the SVG DOM subtree to XML, base64-encode it, and cache as a data URL on the element.

**What's ready:** Pending SVG element addresses from Stage 2.

| Input | Source |
|-------|--------|
| `pending_svg_elements_for_serialization` | Stage 2 — queue populated |
| SVG DOM subtree (`<svg>` + `<circle>`) | Stage 1 — built and styled |

**Important Data Structures:**

- **`ServoUrl`** — Servo's wrapper around a parsed URL. For SVG serialization it holds the `data:image/svg+xml;base64,...` URI that encodes the entire SVG subtree as an inline resource.
- **`NodeDamage::Other`** — A DOM node dirty flag indicating that the node needs re-layout (but style is unchanged). Setting this flag causes `needs_rendering_update()` to return `true`, scheduling the next rendering pass.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| `handle_pending_images_post_reflow()` | `script/window.rs` | Post-reflow hook; drains pending list, dispatches serialization |
| **`serialize_and_cache_subtree()`** | **`script/dom/svg/svgsvgelement.rs`** | **Top-level serialization orchestrator** |
| `process_use_elements()` | `script/dom/svg/svgsvgelement.rs` | Clones `<use>`-referenced subtrees before serialization |
| `cleanup_cloned_nodes()` | `script/dom/svg/svgsvgelement.rs` | Removes temporary `<use>` clones after serialization |
| `xml_serialize()` | `xml5ever` (third-party) | Walks DOM subtree → produces XML string (bytes) |
| `base64::engine::general_purpose::STANDARD.encode()` | `base64` crate (third-party) | Encodes XML bytes → base64 (chars) |
| `ServoUrl::parse()` | `servo_url` crate | Wraps base64 as `data:image/svg+xml;base64,...` |

**Sequence:**
```
layout.borrow_mut().reflow(reflow)
    ↓
handle_pending_images_post_reflow()
    └── serialize_and_cache_subtree()
        ├── process_use_elements()
        ├── xml_serialize()           → XML string
        ├── base64::encode()          → base64
        ├── ServoUrl::parse()         → data URL
        └── cached_serialized_data_url = Some(Ok(url))
    ↓
dirty(NodeDamage::Other) → schedules Pass 2
```

**Third-party:** xml5ever (XML serialization), base64 crate (encoding)

**Output for next stage:**
- `cached_serialized_data_url: Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))`
- `source: None → Some(Ok(url))`
- `NodeDamage::Other` set → triggers Pass 2

---

## Stage 4 — Cache Wait (Pass 2)

**Pass:** Pass 2 (rendering update 2)
**Threads:** Layout Thread + Image Cache Thread (async)
**Purpose:** Re-enter layout with the data URL available. Query the image cache — the VectorImage hasn't loaded yet → cache returns `Pending` → triggers async fetch and parse.

**What's ready:** Data URL from Stage 3. `source=Some(url)` now available.

| Input | Source |
|-------|--------|
| `cached_serialized_data_url: Some(Ok(url))` | Stage 3 — data URL cached |
| `source: Some(Ok(ServoUrl("data:...")))` | Stage 3 — now available for cache lookup |

**Important Data Structures:**

- **`PendingImageId(u64)`** — A handle representing an in-flight image load. Assigned by the image cache when the data URL is first queried but not yet loaded. Used as a key to track the pending load across passes.
- **`ImageCacheErr::Pending(PendingImageId)`** — The cache response indicating the image is being loaded asynchronously. The layout thread receives this and knows no `VectorImage` is available yet.
- **`LoadedVectorImage`** — The loaded SVG data stored in the image cache. Contains a parsed `usvg::Tree`, `ImageMetadata { width, height }`, and the original `svg_id` for mapping back to the DOM element.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| **`svg_kind_size()`** | **`layout/replaced.rs`** | **Called again** — `source=Some(url)` branches to cache lookup |
| `SVGElementData::data()` | `script/dom/svg/svgsvgelement.rs` | Reads `cached_serialized_data_url = Some(Ok(url))` |
| **`get_cached_image_for_url()`** | **`layout/context.rs`** | **Queries image cache by URL** — returns `Pending(PendingImageId(1))` |
| `service_thread()` | `net/image_cache.rs` | Background thread: fetches data URL, decodes base64 |
| `usvg::Tree::from_xml()` | `usvg` crate (third-party) | Parses SVG XML into a tree data structure |
| `complete_load()` | `net/image_cache.rs` | Stores `LoadedVectorImage` in cache by `PendingImageId` |

**Key decision in `svg_kind_size()`:**
```rust
// source=Some(url) → cache lookup
source: Some(Ok(url)) => {
    match get_cached_image_for_url(url) {
        Err(Pending(id)) => { vector_image = None }  // ← this case
        Ok(DataAvailable(image)) => { vector_image = Some(image) }
    }
}
```

**Cache response:** `Err(ImageCacheErr::Pending(PendingImageId(1)))`

**Third-party:** usvg (SVG parser), base64 (decoding)

**Output for next stage:**
- `vector_image: None` (still unavailable)
- Image cache now has: `vector_images[{PendingImageId(1) → LoadedVectorImage}]` (after async load)
- Notification fires → schedules Pass 3

---

### Bridge 2→3 — VectorImage Load Complete

**What happens:** Image cache thread finishes loading the data URL, decodes base64, parses XML into `usvg::Tree`, and stores as `LoadedVectorImage`. Fires a notification to the script thread.

```
Image Cache → pending_layout_image_notification() → needs_rendering_update() → schedule Pass 3
```

**Stored in cache:**
```rust
vector_images: {
    PendingImageId(1) → LoadedVectorImage {
        svg_tree: usvg::Tree,           // parsed SVG
        metadata: { width: 200, height: 200 },
        svg_id: "b3c8d2f4-..."
    }
}
```

---

## Stage 5 — Vector Cache Hit + Rasterization Request (Pass 3)

**Pass:** Pass 3 (rendering update 3)
**Threads:** Layout Thread + Image Cache Thread
**Purpose:** Re-enter layout. This time `get_cached_image_for_url()` returns `DataAvailable` with the loaded `VectorImage`. Then request rasterization — the raster cache is empty → cache miss → async rasterization starts.

**What's ready:** VectorImage loaded in cache. `source=Some(url)` → cache hit on the data URL.

| Input | Source |
|-------|--------|
| `VectorImage` stored by `PendingImageId(1)` | Bridge 2→3 — async load complete |
| `cached_serialized_data_url: Some(Ok(url))` | Stage 3 — unchanged |

**Important Data Structures:**

- **`VectorImage`** (cache-side) — The loaded SVG stored in the image cache. Contains `id: PendingImageId`, `metadata: ImageMetadata`, `svg_tree: usvg::Tree` (the parsed SVG), and `svg_id` for DOM mapping.
- **`ReplacedVectorImage`** (layout-side) — A lightweight reference to the loaded vector image, carried in `ReplacedContentKind::SVGElement.vector_image`. Contains `id: PendingImageId` and `metadata: ImageMetadata`. Layout uses `id` to request rasterization.
- **`RasterImage`** — The rasterized pixel buffer produced by `resvg::render()`. Contains `metadata: ImageMetadata`, `format: PixelFormat::RGBA8`, `bytes: Arc<[u8]>` (raw RGBA pixels), and `id: Option<ImageKey>` (set after GPU texture upload).
- **`ImageKey(IdNamespace, u32)`** — WebRender's handle for a GPU-stored texture. Assigned after rasterization completes. Used in display list commands to reference the image.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| **`svg_kind_size()`** | **`layout/replaced.rs`** | `source=Some(url)` → `get_cached_image_for_url()` → **`DataAvailable`** now |
| `get_cached_image_for_url()` | `layout/context.rs` | Returns `Ok(Arc::new(DataAvailable(VectorImage{...})))` |
| **`make_fragments()`** | **`layout/replaced.rs`** | **Constructs fragments** from `SVGElement(Some(vector_image))` |
| **`rasterize_vector_image()`** | **`layout/context.rs`** → `net/image_cache.rs` | **Checks rasterized cache** — miss → spawns async task |
| `resvg::render()` | `resvg` crate (third-party) | Renders SVG tree into `tiny_skia::Pixmap` pixel buffer |
| `tiny_skia::Pixmap::new()` | `tiny_skia` crate (third-party) | Allocates 200×200 RGBA pixel buffer |

**Sequence:**
```
svg_kind_size():
  get_cached_image_for_url() → Ok(DataAvailable(VectorImage))
  → vector_image = Some(ReplacedVectorImage)

make_fragments():
  rasterize_vector_image(PendingImageId(1), (200,200))
  → Cache MISS → returns None (async rasterization started)
  → No Fragment::Image yet
```

**Cache response:** `None` (cache miss — rasterization dispatched to thread pool)

**Third-party:** tiny_skia (software rasterizer), resvg (SVG-to-pixmap renderer)

**Output for next stage:**
- `PendingRasterizationImage { id: PendingImageId(1), size: (200,200) }` registered
- Thread pool running: `tiny_skia::Pixmap(200,200)` → `resvg::render()` → `RasterImage`
- No `Fragment::Image` yet

---

### Bridge 3→4 — Rasterization Complete

**What happens:** Thread pool finishes rasterizing the SVG. `RasterImage` (with `ImageKey(1, 91)`) stored in rasterized keycache. Notification fires to schedule Pass 4.

```
Image Cache → image_cache_sender.send(response) → needs_rendering_update() → schedule Pass 4
```

**Stored in cache:**
```rust
rasterized_vector_images: {
    (PendingImageId(1), (200,200)) → RasterImage {
        metadata: 200×200, format: RGBA8,
        bytes: Arc<[160,000 RGBA bytes]>,
        id: Some(ImageKey(IdNamespace(1), 91))
    }
}
```

---

## Stage 6 — Raster Cache Hit + Fragment::Image → Display List (Pass 4)

**Pass:** Pass 4 (rendering update 4) — final pass
**Thread:** Layout Thread
**Purpose:** Re-enter layout. `svg_kind_size()` produces the same `vector_image=Some` result. But now `rasterize_vector_image()` **hits** the raster cache → `RasterImage` with `ImageKey` → `Fragment::Image` constructed → display list pushed to WebRender → GPU renders.

**What's ready:** `RasterImage` in keycache with `ImageKey(1, 91)`.

| Input | Source |
|-------|--------|
| `RasterImage` in `rasterized_vector_images` keycache | Bridge 3→4 — async rasterization complete |
| `vector_image: Some(ReplacedVectorImage)` | Stage 5 — unchanged |

**Important Data Structures:**

- **`Fragment::Image`** (a.k.a. `ImageFragment`) — A layout fragment that represents a rasterized image to be painted. Contains `image_key: Option<ImageKey>` (reference to GPU texture), `base: BaseFragment` (position, clip, style), and flags like `showing_broken_image_icon`. This is the output that the display list builder consumes.
- **`DisplayList`** — An ordered list of rendering commands sent to WebRender. Each entry is a `DisplayItem` (like `push_image()`). WebRender processes this list to issue GPU draw calls. The final display list for our SVG contains a single `push_image()` command with the `ImageKey`.

**Key functions:**

| Function | Location | Role |
|----------|----------|------|
| **`svg_kind_size()`** | **`layout/replaced.rs`** | Same result as Stage 5 — `DataAvailable` → `vector_image=Some` |
| **`make_fragments()`** | **`layout/replaced.rs`** | Calls `rasterize_vector_image()` — this time **cache HIT** |
| **`rasterize_vector_image()`** | **`net/image_cache.rs`** | **Cache HIT** → returns `Some(RasterImage { id: Some(ImageKey(1,91)) })` |
| **`Fragment::Image` construction** | `layout/replaced.rs` | Wraps `ImageKey` + rect into `ImageFragment` |
| `push_image()` | Display list builder / WebRender | Adds image command to the display list |

**Sequence:**
```
svg_kind_size() → same: vector_image=Some

make_fragments():
  rasterize_vector_image(PendingImageId(1), (200,200))
  → Cache HIT! Returns Some(RasterImage)
  → Extract ImageKey(IdNamespace(1), 91)

Fragment::Image {
    image_key: Some(ImageKey(IdNamespace(1), 91)),
    rect: (Au(0), Au(0), Au(12000), Au(12000)),
    showing_broken_image_icon: false,
}

display_list.push_image(
    ClipId::root(),
    LayoutRect(0, 0, 200, 200),
    ImageKey(IdNamespace(1), 91),
    ImageRendering::Auto,
)

→ WebRender → GPU: 200×200 blue circle rendered at (0,0)
```

**Key units:** 1 CSS px = 60 Au → 200 px = Au(12000)

**Third-party:** WebRender (GPU rendering engine)

**Output:** ✅ Blue circle rendered on screen. Pipeline complete.

---

## Quick Reference — All Stages

| Stage | Pass | Thread(s) | Key Function | Produces Fragment? |
|-------|------|-----------|--------------|--------------------|
| 1 — DOM Construction | Pre | Script | `create_element()`, `SVGSVGElement::new()` | ❌ |
| 2 — Layout Traversal | Pass 1 | Layout | `svg_kind_size()`, `queue_svg_element_for_serialization()` | ❌ |
| 3 — SVG Serialization | Pass 1* | Script | `serialize_and_cache_subtree()`, `xml_serialize()` + `base64::encode()` | ❌ |
| 4 — Cache Wait | Pass 2 | Layout + Image Cache | `get_cached_image_for_url()` → `Pending`, `usvg::Tree::from_xml()` | ❌ |
| 5 — Vector Hit + Rasterize Request | Pass 3 | Layout + Image Cache | `rasterize_vector_image()` (miss), `resvg::render()`, `tiny_skia::Pixmap` | ❌ |
| 6 — Fragment::Image → GPU | Pass 4 | Layout | `rasterize_vector_image()` (HIT), `push_image()` | ✅ |

\* Stage 3 runs as a post-reflow hook in the same `Window::reflow()` call — no separate rendering update.

### Two Async Bridges

| Bridge | From | To | What Completes |
|--------|------|----|----------------|
| Bridge 2→3 | Pass 2 | Pass 3 | VectorImage load (data URL fetch + usvg parse) |
| Bridge 3→4 | Pass 3 | Pass 4 | Rasterization (tiny_skia pixmap + resvg render) |

### Third-Party Dependencies Used

| Crate | Used In Stage(s) | Role |
|-------|-----------------|------|
| Stylo | 1 | Parallel CSS engine (style computation) |
| xml5ever | 3 | XML serialization of DOM subtree |
| base64 | 3 | Base64 encode/decode |
| usvg | 4, 5 | SVG XML parser → tree data structure |
| tiny_skia | 5, 6 | Software pixel buffer (200×200 RGBA) |
| resvg | 5, 6 | SVG tree → pixmap rasterization |
| WebRender | 6 | GPU rendering via display list commands |
