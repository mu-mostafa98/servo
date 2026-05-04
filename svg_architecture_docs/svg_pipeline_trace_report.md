# SVG Rendering Pipeline — Full Trace Report

> **Test file:** [svg_tests/simple_svg.html](../svg_tests/simple_svg.html)
> **Total reflow passes:** 4
> **Total trace lines:** 158

```html
<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <circle cx="100" cy="100" r="50" fill="blue" />
</svg>
```

---

## Pass 1 — `Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(DOMChanged | PendingRestyles)`

### Stage 1 — DOM Element Creation

**File:** [components/script/dom/create.rs](../components/script/dom/create.rs)
**Function:** `create_svg_element()`

| Call | Input (`name.local`) | Output |
|------|---------------------|--------|
| 1 | `Atom('svg' type=inline)` | `SVGSVGElement` |
| 2 | `Atom('circle' type=inline)` | `SVGElement` |

The HTML parser encounters the `<svg>` tag in the SVG namespace and dispatches to `create_svg_element()`. The `<svg>` tag becomes `SVGSVGElement`, the `<circle>` child becomes a generic `SVGElement`. Each gets a unique UUID and is added to the DOM tree.

---

### Stage 2.2 — Layout Traversal Entry

**File:** [components/layout/dom_traversal.rs](../components/layout/dom_traversal.rs)
**Function:** `traverse_element()`

**Input:** `ServoLayoutNode` for `<svg>`

The layout traversal walks the DOM tree depth-first:
1. `<html>` → `display: Block`
2. `<head>` → `display: None` (skipped)
3. `<body>` → `display: Block`
4. **`<svg>`** → display = **`GeneratingBox(OutsideInside { outside: Inline, inside: Flow })`**, **`is_svg = true`** → enters `GeneratingBox` arm

**Output:** `Contents::for_element()` called with the SVG element.

---

### Stage 2.3 — Contents Type Detection

**File:** [components/layout/dom_traversal.rs](../components/layout/dom_traversal.rs)
**Function:** `Contents::for_element()`

**Input:** `ServoLayoutNode` — SVG element

The function checks `ReplacedContents::for_element()` for the SVG element. Returns `Contents::Replaced(...)`.

**Output:** `Contents::Replaced(ReplacedContents { kind: SVGElement(None), ... })`

---

### Stage 2.5 — SVG Element Data Construction

**File:** [components/script/dom/svg/svgsvgelement.rs](../components/script/dom/svg/svgsvgelement.rs)
**Function:** `SVGSVGElement::data()`

**Input:** `LayoutDom<SVGSVGElement>` — the SVG element handle

The function reads:
- `uuid` → `"90b40da2-767a-432d-b6ff-56875f1ee205"` (random UUID per element instance)
- `width` attribute → `Some(LengthPercentage("200", Some(Length(Absolute(Px(200.0))))))`
- `height` attribute → `Some(LengthPercentage("200", Some(Length(Absolute(Px(200.0))))))`
- `viewBox` attribute → `Some`

**Critical field:** `cached_serialized_data_url.borrow_for_layout()` → **`None`** (not serialized yet)

**Output:**
```rust
SVGElementData {
    source: None,                              // ← NOT serialized yet
    width: Some(LengthPercentage("200", ...)),
    height: Some(LengthPercentage("200", ...)),
    view_box: Some,
    svg_id: "90b40da2-767a-432d-b6ff-56875f1ee205",
}
```

---

### Stage 2.4 — ReplacedContent Dispatch

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Function:** `ReplacedContents::for_element()`

**Input:** `ServoLayoutNode` + `SVGElementData`

The dispatch chain:
1. `as_image()` → None (not an `<img>`)
2. `as_canvas()` → None
3. `as_iframe()` → None
4. `as_video()` → None
5. **`as_svg()` → `Some(svg_data)`** — SVG detected!

**Output:** SVG detected with `source=None`, `width=Some("200")`, `height=Some("200")`

---

### Stage 2.6 — SVG Natural Size & Source Resolution

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Function:** `svg_kind_size()`

**Input:**
```rust
source = None                                  // not serialized
width_attr = Some(LengthPercentage("200", Some(Length(Absolute(Px(200.0))))))
height_attr = Some(LengthPercentage("200", Some(Length(Absolute(Px(200.0))))))
svg_id = "90b40da2-767a-432d-b6ff-56875f1ee205"
```

**Internal steps:**

| Step | Action | Result |
|------|--------|--------|
| 1 | Get parent style (body) | Parent style acquired |
| 2 | Compute w/h from attributes | `width=Some(200.0 px)`, `height=Some(200.0 px)` |
| 3 | Compute natural_size | `(Some(200px), Some(200px), Some(1.0))` |
| 4 | **Branch on `source`** | **`source=None` → QUEUE FOR SERIALIZATION** |
| 5 | `queue_svg_element_for_serialization(node)` | Node added to pending serialization list |

**Output:**
```rust
(ReplacedContentKind::SVGElement(None), NaturalSizes {
    width: Some(Au(12000)),    // 200px × 60 Au/px
    height: Some(Au(12000)),
    ratio: Some(1.0),
})
```

**Key insight:** `vector_image = NONE` because the SVG hasn't been serialized to a data URL yet. The serialization is *queued* for the script thread to process after layout completes.

---

### Stage 3 — Queue SVG for Serialization

**File:** [components/layout/context.rs](../components/layout/context.rs)
**Function:** `queue_svg_element_for_serialization()`

**Input:** `ServoLayoutNode` → `OpaqueNode(17776695034752)`

The node is pushed to `pending_svg_elements_for_serialization` list. This list is read by `handle_pending_images_post_reflow()` on the script thread after the layout pass completes.

**Output:** Node queued for serialization. Stage 3 handler in [window.rs](../components/script/dom/window.rs) will process it post-reflow.

---

### Stage 8 — Fragment Construction (Empty)

**File:** [components/layout/replaced.rs](../components/layout/replaced.rs)
**Function:** `make_fragments()`

**Input:** `size=200px×200px`, `kind=SVGElement(None)`

```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else {
        return vec![];  // ← TAKEN: vector_image is None
    };
```

**Output:** `vec![]` — **empty fragment**. No image data available yet, so no fragment is produced. The SVG takes up no visual space in this pass.

---

### Stage 3 (post-reflow) — Script handles pending serialization

**File:** [components/script/dom/window.rs](../components/script/dom/window.rs)
**Function:** `handle_pending_images_post_reflow()`

After the layout pass completes, the script thread processes pending SVG elements. For each queued node:
1. Calls `serialize_and_cache_subtree()` — **triggers Stage 4**
2. Sets `dirty` flag on the node — **triggers next reflow**

**Output:** SVG node is dirty → triggers Pass 2.

---

## Pass 2 — `Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(PendingRestyles)`

### Stage 4 — SVG Serialization

**File:** [components/script/dom/svg/svgsvgelement.rs](../components/script/dom/svg/svgsvgelement.rs)
**Function:** `serialize_and_cache_subtree()`

**Input:** `SVGSVGElement` (self)

**Internal steps:**

| Step | Action | Result |
|------|--------|--------|
| 1 | Process `<use>` elements | 0 cloned nodes (no `<use>` in test) |
| 2 | `xml_serialize()` subtree | Success |
| 3 | XML source length | **231 bytes** |
| 4 | Base64 encode | Standard base64 |
| 5 | Data URL length | **334 characters** |
| 6 | Parse as ServoUrl | Success |

**Serialized XML (decoded from base64):**
```xml
<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <circle xmlns="http://www.w3.org/2000/svg" cx="100" cy="100" r="50" fill="blue"></circle>
</svg>
```

**Data URL cached:**
```
data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAgMjAwIDIwMCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICAgICAgICA8Y2lyY2xlIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgY3g9IjEwMCIgY3k9IjEwMCIgcj0iNTAiIGZpbGw9ImJsdWUiPjwvY2lyY2xlPgogICAgPC9zdmc+
```

**Output:** `cached_serialized_data_url` set to `Some(Ok(data_url))`.

---

### Stage 3 (post-reflow) — Dirty Flag

After serialization, `handle_pending_images_post_reflow()` sets the `dirty` flag on the SVG node.

**Output:** `NodeDamage::Other` → triggers **Pass 3**.

---

## Pass 3 — `Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(PendingRestyles)`

### Stage 2.5 (repeat) — SVG Element Data

**Input:** `LayoutDom<SVGSVGElement>`

**Output:** `source = Some(Ok(data_url))` — the data URL from Stage 4 is now available!

All other fields unchanged:
- `width` = `Some(LengthPercentage("200", ...))`
- `height` = `Some(LengthPercentage("200", ...))`
- `view_box` = `Some`
- `svg_id` = `"9435b93e-ea8a-4323-996d-b048ab24a3ab"` (new UUID, element was re-created)

---

### Stage 2.6 (repeat) — Source Resolution

**Input:**
```rust
source = Some(Ok("data:image/svg+xml;base64,..."))
width_attr = Some(LengthPercentage("200", ...))
height_attr = Some(LengthPercentage("200", ...))
svg_id = "9435b93e-ea8a-4323-996d-b048ab24a3ab"
```

**Internal steps:**

| Step | Action | Result |
|------|--------|--------|
| 1 | Parent style | Acquired |
| 2 | Compute w/h | `width=Some(200.0 px)`, `height=Some(200.0 px)` |
| 3 | natural_size | `(Some(200px), Some(200px), Some(1.0))` |
| 4 | **Branch on source** | **`source=Some(Ok/Err)` → resolve URL** |
| 5 | `get_cached_image_for_url()` | **`ERR/NOT_CACHED`** — image not loaded yet |

**Why the image is not cached:**
The data URL was just created in Pass 2. The image cache hasn't had time to:
1. Create a load request for the data URL
2. Parse the SVG with usvg
3. Store the resulting `VectorImage`

This happens asynchronously. The load request was initiated when `get_or_request_image_or_meta()` was first called, but the response hasn't arrived yet.

**Output:**
```rust
(ReplacedContentKind::SVGElement(None), NaturalSizes {
    width: Some(Au(12000)),
    height: Some(Au(12000)),
    ratio: Some(1.0),
})
```

Still `vector_image = NONE` — the image cache lookup failed.

---

### Stage 8 (repeat) — Empty Fragment Again

**File:** [replaced.rs](../components/layout/replaced.rs)
**Function:** `make_fragments()`

**Input:** `size=200px×200px`, `kind=SVGElement(None)`

**Output:** `vec![]` — still empty. No image rasterization available.

---

## Pass 4 — `Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(PendingRestyles)`

### Stage 5 — Async Image Load Complete

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `complete_load()`

**Input:** `key=PendingImageId(1)`, `load_result=LoadedVectorImage`

This fires when the image cache finishes parsing the SVG data URL:

| Step | Action | Result |
|------|--------|--------|
| 1 | Remove from pending loads | Load found |
| 2 | Insert into `vector_images` | Key `PendingImageId(1)` → `usvg::Tree` |
| 3 | Get natural dimensions | **200×200** from parsed SVG |
| 4 | Build `VectorImage` metadata | `ImageMetadata { width: 200, height: 200 }` |

**Output:** `ImageResponse::Loaded(Image::Vector(VectorImage { id: PendingImageId(1), metadata: 200×200 }), url)`

The `usvg::Tree` is now stored in `self.vector_images` keyed by `PendingImageId(1)`.

---

### Stage 2.5 (repeat) — SVG Element Data

**Output:** `source = Some(Ok(data_url))` — same as Pass 3.

---

### Stage 2.6 (repeat) — Image Cache Hit

**Input:** `source = Some(Ok(data_url))`, `svg_id = "9435b93e-ea8a-4323-996d-b048ab24a3ab"`

| Step | Action | Result |
|------|--------|--------|
| 1-3 | Same as previous passes | Same results |
| 4 | Branch on source | `source=Some(Ok/Err)` → resolve URL |
| 5 | `get_cached_image_for_url()` | **`"OK"`** — cached! |
| 6 | Match on Image | **`Image::Vector(VectorImage)`** |

**The cached image data:**
```rust
VectorImage {
    id: PendingImageId(1),
    svg_id: None,                          // will be tagged with our UUID
    metadata: ImageMetadata {
        width: 200,
        height: 200,
    },
    cors_status: ...,
}
```

After tagging: `svg_id = Some("9435b93e-ea8a-4323-996d-b048ab24a3ab")`

**Output:**
```rust
(ReplacedContentKind::SVGElement(Some(VectorImage {
    id: PendingImageId(1),
    svg_id: Some("9435b93e-ea8a-4323-996d-b048ab24a3ab"),
    metadata: ImageMetadata { width: 200, height: 200 },
})), NaturalSizes {
    width: Some(Au(12000)),
    height: Some(Au(12000)),
    ratio: Some(1.0),
})
```

**`vector_image = SOME`** — first time we have actual image data!

---

### Stage 8 — Fragment Construction (With Image Data)

**File:** [replaced.rs](../components/layout/replaced.rs)
**Function:** `make_fragments()`

**Input:** `size=200px×200px`, `kind=SVGElement(Some(VectorImage { ... }))`

```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else {
        return vec![];  // ← NOT taken this time!
    };
    // vector_image.is_some = true
    // metadata = 200×200
```

**Fragment building:**
1. `base.rect` set to `200px × 200px` from vector_image metadata
2. `raster_size` computed = `200 × 200` (at 1.0 device pixel ratio)
3. `rasterize_vector_image()` called with `PendingImageId(1)`, `200×200`, `svg_id=Some("9435b93e...")`

**Output:**
```rust
Fragment::Image(ArcRefCell::new(ImageFragment {
    base: BaseFragment { rect: 200px × 200px, ... },
    clip: clip rect,
    image_key: None,         // will be set after rasterization
    showing_broken_image_icon: false,
    url: None,
}))
```

---

### Stage 6 — Vector Image Rasterization

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `rasterize_vector_image()`

**Input:**
```rust
image_id = PendingImageId(1)
requested_size = 200 × 200
svg_id = Some("9435b93e-ea8a-4323-996d-b048ab24a3ab")
```

| Step | Action | Result |
|------|--------|--------|
| 1 | Lock store, get vector_image | Found: `usvg tree size=200×200` |
| 2 | Check rasterized cache | Miss (not cached yet) |
| 3 | Update `svg_id_image_id_map` | Maps our UUID → `PendingImageId(1)` |
| 4 | Update `image_id_size_map` | Maps `PendingImageId(1)` → `[200×200]` |
| 5 | **Spawn thread pool task** | Async rasterization |

**Thread pool task (async):**
```rust
natural_size = 200 × 200
tinyskia_requested_size = 200 × 200    // clamped to MAX_SVG_PIXMAP_DIMENSION
transform = scale(1.0, 1.0)           // 200/200 = 1.0
pixmap = tiny_skia::Pixmap::new(200, 200)
resvg::render(&svg_tree, transform, &mut pixmap)
bytes = pixmap.take()                  // 160000 bytes (200 × 200 × 4)
```

**Rasterized result:**
```rust
RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc::new([160000 bytes of RGBA pixel data]),
    id: None,              // set when WebRender key is assigned
    is_opaque: false,
}
```

**Output:** `None` initially (async), then calls `load_image_with_keycache(PendingKey::Svg(...))`.

---

### Stage 7 — WebRender Image Key Assignment

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `set_key_and_finish_load()`

**Input:**
```rust
pending_image = PendingKey::Svg((PendingImageId(1), RasterImage{200×200, 160000 bytes}, 200×200))
image_key = ImageKey(IdNamespace(1), 90)
```

| Step | Action | Detail |
|------|--------|--------|
| 1 | Match pending variant | `Svg` branch taken |
| 2 | `set_webrender_image_key()` | Assigns `ImageKey(IdNamespace(1), 90)` to the rasterized image |
| 3 | `complete_load_svg()` | Notifies listeners |

---

### Stage 5 (second part) — complete_load_svg

**File:** [components/net/image_cache.rs](../components/net/image_cache.rs)
**Function:** `complete_load_svg()`

**Input:**
```rust
rasterized_image: RasterImage(200×200, 160000 bytes)
pending_image_id: PendingImageId(1)
requested_size: 200×200
```

| Step | Action | Result |
|------|--------|--------|
| 1 | Look up listeners | Found **1 listener** |
| 2 | Store rasterized result | `task.result = Some(rasterized_image)` |
| 3 | Notify pipeline | `VectorImageRasterizationComplete { pipeline_id: (1,1), image_id: PendingImageId(1), requested_size: 200×200 }` |

**Output:** Pipeline `(1,1)` notified that rasterization is complete. This triggers another reflow (potentially Pass 5), but in our 4-pass trace, the rasterized image was already available for the **same pass**.

---

### Stage 6 (second call) — Cached Raster Hit

When called again in the same pass:

```rust
rasterize_vector_image() → CACHED result, returning early
```

The rasterized image from the thread pool task was stored in `rasterized_vector_images` cache. The second call returns the cached result immediately without spawning another task.

---

### Stage 9 — Display List Construction

**File:** [components/layout/display_list/mod.rs](../components/layout/display_list/mod.rs)
**Fragment handler:** `Fragment::Image`

**Input:**
```rust
Fragment::Image {
    rect: Rect(200px×200px at (0px, 0px)),
    image_key: Some(ImageKey(IdNamespace(1), 90)),
}
```

**Processing:**
1. Visibility check → `Visible`
2. Compute image rendering → `auto`
3. Translate rect to containing block coordinates
4. Push image to WebRender display list:
   ```rust
   builder.wr().push_image(
       &common, rect, image_rendering,
       wr::AlphaType::PremultipliedAlpha,
       ImageKey(IdNamespace(1), 90),   // ← the SVG raster
       wr::ColorF::WHITE,
   );
   ```

**Output:** WebRender display list item for the SVG raster image. The GPU will render a 200×200 blue circle.

---

## Summary Table

| Pass | Reflow Reason | Source State | Image Cache | vector_image | fragment | Stage 9? |
|------|---------------|-------------|-------------|-------------|----------|----------|
| **1** | DOMChanged \| PendingRestyles | `None` | N/A | `None` | Empty `vec![]` | No |
| **2** | PendingRestyles | Serializing... | N/A | N/A (serialization) | N/A | No |
| **3** | PendingRestyles | `Some(Ok(url))` | `ERR/NOT_CACHED` | `None` | Empty `vec![]` | No |
| **4** | PendingRestyles | `Some(Ok(url))` | `"OK"` → VectorImage | `Some(...)` | `Fragment::Image` | Yes |

---

## Key Function Call Flow (Pass 4 Complete)

```
create_svg_element(name.local="svg")          → SVGSVGElement         [STAGE 1]
create_svg_element(name.local="circle")       → SVGElement            [STAGE 1]
    ↓
traverse_element(svg_node)                    → display=Inline        [STAGE 2.2]
Contents::for_element(svg_node)               → Replaced(SVGElement)  [STAGE 2.3]
SVGSVGElement::data()                         → SVGElementData        [STAGE 2.5]
    ↓
ReplacedContents::for_element(svg_node)       → SVG DETECTED          [STAGE 2.4]
svg_kind_size(source=None/Ok, w=200, h=200)   → SVGElement(None/Some) [STAGE 2.6]
    ├─ source=None        → queue_svg_element_for_serialization()     [STAGE 3]
    └─ source=Some(Ok)    → get_cached_image_for_url()
                            ├─ ERR/NOT_CACHED → SVGElement(None)
                            └─ "OK" → VectorImage → SVGElement(Some)
    ↓
make_fragments(size=200×200, kind=SVGElement) → Vec<Fragment>        [STAGE 8]
    ├─ vector_image=None  → vec![] (empty)
    └─ vector_image=Some  → rasterize_vector_image()
                            → Fragment::Image { image_key }
    ↓
serialize_and_cache_subtree()                 → XML→base64→data:url   [STAGE 4]
complete_load(key=1, VectorImage)             → usvg parsed 200×200  [STAGE 5]
rasterize_vector_image(id=1, 200×200)         → RasterImage 160KB    [STAGE 6]
set_key_and_finish_load(Svg, ImageKey(1,90))  → WR texture ready     [STAGE 7]
complete_load_svg(id=1, 200×200, 1 listener)  → Pipeline notified    [STAGE 5]
    ↓
DisplayList Fragment::Image(ImageKey(1,90))   → WR display item      [STAGE 9]
```

## How to Reproduce

```powershell
# Build
./mach build

# Run with full SVG tracing
./target/debug/servoshell.exe --exit "file:///D:/Projects/servo/svg_tests/simple_svg.html" -Z relayout-event 2>&1 | Select-String "\[SVG_TRACE"

# Count reflow passes
./target/debug/servoshell.exe --exit "file:///D:/Projects/servo/svg_tests/simple_svg.html" -Z relayout-event 2>&1 | Select-String "Reflow"

# Filter to specific stages
./target/debug/servoshell.exe --exit "file:///D:/Projects/servo/svg_tests/simple_svg.html" -Z relayout-event 2>&1 | Select-String "\[SVG_TRACE_STAGE_[46]"
```
