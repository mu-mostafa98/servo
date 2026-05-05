# SVG Rendering Pipeline — Stage Overview with Class Hierarchy

> A comprehensive breakdown of the SVG rendering pipeline across all 8 stages, 4 passes, and 3 threads. This document extends the [stage_pipeline_overview.md](stage_pipeline_overview.md) with the complete class hierarchy, field-level details, inter-class relationships, and thread ownership model.
>
> **Test case:**
> ```html
> <svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
>   <circle cx="100" cy="100" r="50" fill="blue"/>
> </svg>
> ```
>
> All concrete values shown in each section are derived from this test case.

---

## Table of Contents

1. [Pipeline Architecture](#1-pipeline-architecture)
2. [Stage 1 — DOM Construction & Style Computation](#2-stage-1--dom-construction--style-computation)
3. [Stage 2 — Layout Traversal & Replaced Content](#3-stage-2--layout-traversal--replaced-content)
4. [Stage 3 — Serialization Dispatch](#4-stage-3--serialization-dispatch)
5. [Stage 4 — Subtree Serialization](#5-stage-4--subtree-serialization)
6. [Stage 5 — Cache Wait](#6-stage-5--cache-wait)
7. [Stage 6 — Vector Rasterization](#7-stage-6--vector-rasterization)
8. [Stage 7 — Cache Response](#8-stage-7--cache-response)
9. [Stage 8 — Fragment Construction & Display List](#9-stage-8--fragment-construction--display-list)
10. [The Four-Pass Retry Loop](#10-the-four-pass-retry-loop)
11. [Key Values Through the Pipeline](#11-key-values-through-the-pipeline)
12. [Pass Reference](#12-pass-reference)
13. [Class Hierarchy & Details](#13-class-hierarchy--details)
14. [Thread & Ownership Model](#14-thread--ownership-model)
15. [Cache Invalidation Reference](#15-cache-invalidation-reference)

---

## 1. Pipeline Architecture

### Thread Swimlanes

The pipeline spans three threads, each executing specific stages:

| Thread | Stages | Responsibilities |
|--------|--------|-----------------|
| **Script Thread** | Stage 1.1–1.6, Stage 3, Stage 4 | HTML parsing, DOM construction, SVG subtree serialization |
| **Layout Thread** | Stage 1.7, Stage 2, Stage 5, Stage 7, Stage 8 | Style computation (Stylo), DOM traversal, fragment construction, display list |
| **Image Cache Thread** | Stage 5 (async), Stage 6 | Data URL fetching, `usvg` parsing, `resvg` rasterization |

### Passes Overview

The pipeline requires **4 passes** (reflow cycles) before an SVG element becomes visible on screen:

| Pass | What Happens | Result |
|------|-------------|--------|
| **P1** (Pass 1) | First layout, `source=None` → queue for serialization | No fragment produced |
| **P2** (Pass 2) | Script thread serializes SVG → base64 data URL | `cached_serialized_data_url = Some(Ok(url))` |
| **P3** (Pass 3) | Re-layout with data URL; image cache miss → retry | Still no fragment |
| **P4** (Pass 4) | VectorImage cached → rasterize → `Fragment::Image` | SVG visible on screen |

### Data Flow Summary

```
HTML bytes ──Stage 1──> SVG DOM Element ──Stage 1.7──> Styled ComputedValues
    │                                                         │
    │                                                  Stage 2 (P1)
    │                                                         │
    │                                                  source=None
    │                                                         │
    │                                                  queue_svg_element()
    │                                                         │
    └──────────────Stage 3 (dispatch) ──Stage 4 (P2)──────────┘
                                                         │
                                                  data: URL cached
                                                         │
                                                  Stage 5 (P3) ──> image cache miss
                                                         │
                                                  Stage 7 (P4) ──> VectorImage hit
                                                         │
                                                  Stage 6 (P4) ──> RasterImage
                                                         │
                                                  Stage 8 (P4) ──> Fragment::Image
                                                         │
                                                  Display List ──> GPU
```

---

## 2. Stage 1 — DOM Construction & Style Computation

**Pass:** Pre-Pass (before P1)
**Threads:** Script (1.1–1.6) → Layout (1.7)
**Key files:**
- [components/script/dom/create.rs](../../components/script/dom/create.rs)
- [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
- [components/script/dom/svg/svgelement.rs](../../components/script/dom/svg/svgelement.rs)
- [components/layout/layout_impl.rs](../../components/layout/layout_impl.rs)

### Sub-stages 1.1–1.6: HTML Parser → DOM Construction (Script Thread)

**Purpose:** Build the SVG DOM tree with a unique `uuid`; parse and store `width`/`height` attributes.

**Key functions:**

| Function | Role |
|----------|------|
| `create_element()` (in `create.rs`) | Dispatches element creation by namespace; routes `ns!(svg)` to `create_svg_element()` |
| `SVGSVGElement::new()` | Allocates the `SVGSVGElement` DOM object with a fresh `Uuid::new_v4()` |
| `parse_plain_attribute()` | Parses `width`/`height` into `LengthPercentage` values via CSS parser |
| `InsertBefore()` | Inserts the `<svg>` element into the document tree under `<body>` |

**Operations in detail:**
1. `html5ever` tokenizer produces a start-tag token for `<svg>` in the SVG namespace:
   ```rust
   QualName {
       ns: ns!(svg),  // "http://www.w3.org/2000/svg"
       local: local_name!("svg"),
       prefix: None,
   }
   ```
2. Tree sink calls `create_element_for_token()` → `create_element()` → namespace dispatch
3. For `ns!(svg)`, calls `create_svg_element()` → `SVGSVGElement::new_inherited()`
4. `new_inherited()` generates `uuid: Uuid::new_v4().to_string()` and sets `cached_serialized_data_url: None`
5. `parse_plain_attribute()` processes `width`/`height` → CSS parser → `LengthPercentage` values
6. `InsertBefore()` adds the element to the DOM tree

**Concrete values for our test case:**

After construction, the `SVGSVGElement` DOM node holds:

```rust
SVGSVGElement {
    svggraphicselement: SVGGraphicsElement { /* ... */ },
    uuid: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",       // Uuid::new_v4()
    cached_serialized_data_url: DomRefCell::new(None),   // not serialized yet
}
```

The parsed attributes stored on the element (via `AttrValue`):

```rust
// width="200" → Attribute in list:
AttrValue::LengthPercentage(
    "200".to_owned(),                                    // raw string
    Some(LengthPercentage::Length(LengthValue::Px(200.0)))  // parsed value
)

// height="200" → same struct:
AttrValue::LengthPercentage(
    "200".to_owned(),
    Some(LengthPercentage::Length(LengthValue::Px(200.0)))
)

// viewBox="0 0 200 200" → stored as plain string:
AttrValue::String("0 0 200 200".to_owned())

// xmlns="http://www.w3.org/2000/svg" → stored as plain string:
AttrValue::String("http://www.w3.org/2000/svg".to_owned())
```

### Sub-stage 1.7: Style Computation (Layout Thread)

**Purpose:** Compute CSS properties (`display`, `width`, `height`) for each DOM element via Stylo (Servo's CSS engine).

**Key functions:**

| Function | Role |
|----------|------|
| `handle_reflow()` | Entry point for layout; triggers style recomputation on dirty subtree |
| `restyle_and_build_trees()` | Iterates dirty elements, resolves CSS cascade |
| `traverse_dom()` | Walks the DOM tree, calls style resolution per element |
| `ServoDangerousStyleElement::match_element()` | Matches CSS selectors against element, produces `ComputedValues` |

**Important notes:**
- Style computation uses **Stylo** (parallel CSS engine, shared with Firefox)
- `ComputedValues` are **cached** in `ElementData.styles` — full recompute only on first layout or after mutations
- On P3/P4 re-entry, style is **not recomputed** (it's cloned via `Arc::clone()`)

**Concrete values for our test case:**

After style computation, each DOM element has its `ElementData.styles` populated. For the `<svg>` root:

```rust
// Stored in element's ElementData
styles: Arc<ComputedValues> {
    // From CSS cascade + presentational hints (width="200" → width: 200px)
    display: Display::Inline,
    width:  LengthPercentageOrAuto::LengthPercentage(
        LengthPercentage::Length(LengthValue::Px(200.0))
    ),
    height: LengthPercentageOrAuto::LengthPercentage(
        LengthPercentage::Length(LengthValue::Px(200.0))
    ),
    // ... all other CSS properties use their initial values
}

// For the <circle> child element:
styles: Arc<ComputedValues> {
    display: Display::Inline,
    // No explicit width/height on <circle> → defaults
}
```

> **Key insight:** Style is computed **once** and then cached for all subsequent passes (P3, P4). The `Arc::clone()` is a pointer bump, not a recomputation.

---

## 3. Stage 2 — Layout Traversal & Replaced Content

**Pass:** P1 (Pass 1)
**Thread:** Layout Thread
**Key files:**
- [components/layout/dom_traversal.rs](../../components/layout/dom_traversal.rs)
- [components/layout/replaced.rs](../../components/layout/replaced.rs)
- [components/layout/style_ext.rs](../../components/layout/style_ext.rs)

### Purpose

Classify each element's display type; for SVG elements, dispatch as **Replaced** content and compute natural dimensions. This is where the pipeline discovers that `<svg>` is special and needs different handling from regular HTML elements.

### 2.1: traverse_element() — DOM Traversal

**Purpose:** Walk the DOM tree and classify each element's display behavior; route SVG elements into the replaced content pipeline.

**Key functions:**

| Function | Role |
|----------|------|
| `traverse_element()` | Top-level traversal; reads computed `display` property |
| `Display::from()` | Converts computed `display` → `None` / `Contents` / `GeneratingBox` |
| `Contents::for_element()` | Checks if element is a replaced element; for SVG, returns `ReplacedContentKind::SVGElement` |
| `handle_element()` | Generates layout boxes from traversal output |

**Operations in detail:**
1. `traverse_element()` reads `ElementData.styles.get_display()` → `display: inline`
2. For SVG elements, `Contents::for_element()` identifies it as a replaced element
3. Returns `ReplacedContentKind::SVGElement { vector_image: None, has_viewbox: true }`
4. Children are recursed; `handle_element()` generates layout boxes

**Concrete values for our test case:**

```rust
// Output of Contents::for_element() for our <svg> element:
ReplacedContentKind::SVGElement {
    vector_image: None,      // no image source available yet
    has_viewbox: true,       // viewBox="0 0 200 200" is present
}

// The computed display classification:
Display::from(computed_display) → GeneratingBox::Inline  // SVG defaults to inline
```

### 2.2: svg_kind_size() — P1 (First Layout)

**Purpose:** Compute natural dimensions from SVG attributes and queue the element for P2 serialization, since the image source is not yet available.

**Key functions:**

| Function | Role |
|----------|------|
| `svg_kind_size()` | Central sizing function; reads `source` field to determine next action |
| `SVGElementData::data()` | Constructs data struct from SVG element (reads uuid, width, height, viewBox, source) |
| `queue_svg_element_for_serialization()` | Enqueues the SVG element's uuid for later serialization in the script thread |

**Operations in detail:**
1. `SVGElementData::data()` reads: `uuid`, `width`/`height` (as parsed `LengthPercentage`), `viewBox`, and — critically — `source`
2. In P1, `source = None` because no serialization has happened yet
3. Parses width/height → computes aspect ratio → produces `NaturalSizes`
4. Since `source = None`, calls `queue_svg_element_for_serialization()` to trigger serialization in script thread
5. Returns `Replaced(SVGElement(None))` — no `VectorImage` available yet

**Concrete values for our test case:**

```rust
// SVGElementData constructed in data():
SVGElementData {
    svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",  // same uuid
    source: None,                                       // no data URL yet
    width:  Some(AttrValue::LengthPercentage(
        "200".to_owned(),
        Some(LengthPercentage::Length(LengthValue::Px(200.0)))
    )),
    height: Some(AttrValue::LengthPercentage(
        "200".to_owned(),
        Some(LengthPercentage::Length(LengthValue::Px(200.0)))
    )),
    view_box: Some(AttrValue::String("0 0 200 200".to_owned())),
}

// Natural sizes computed from width/height (200px → Au(12000)):
NaturalSizes {
    width:  Au(12000),   // 200 CSS pixels × 60 Au/px
    height: Au(12000),   // 200 CSS pixels × 60 Au/px
    ratio:  1.0,         // square aspect ratio (200/200)
}

// Since source=None, the svg_kind_size returns:
Replaced(SVGElement(None))   // vector_image = None → no Fragment::Image
```

### Bridge: P1 → P2

After layout completes, the post-reflow hook picks up the queued SVG element UUIDs and dispatches them to the script thread for serialization:

```
Layout Thread                    Script Thread
    │                                  │
    ├─ queue_svg_element() ──────────► │
    │   (stores "b3c8d2f4-...")       │
    │                                  │
    └─ post-reflow hook ─────────────► │
       (handle_pending_images())       │
                                       ▼
                                Stage 4: Serialization
```

---

## 4. Stage 3 — Serialization Dispatch

**Pass:** P1 → P2 Bridge
**Thread:** Script Thread (dispatched from Layout)
**Key files:**
- [components/script/dom/window.rs](../../components/script/dom/window.rs)

### Purpose

Bridge the gap between layout pass and script: pick up SVG elements that were queued for serialization during P1 layout, and invoke their serialization on the script thread.

**Key functions:**

| Function | Role |
|----------|------|
| `handle_pending_svg_elements_for_serialization()` | Iterates the pending queue and calls `serialize_and_cache_subtree()` for each pending SVG |

**Operations in detail:**
1. Layout thread stores a list of SVG element UUIDs that need serialization
2. After P1 layout completes, script's `handle_pending_svg_elements_for_serialization()` processes the list
3. For each pending SVG, calls `self.upcast::<SVGSVGElement>().serialize_and_cache_subtree()`

**Concrete values for our test case:**

```rust
// Layout thread stores this in the pending queue:
pending_svg_uuids: Vec<String> = vec![
    "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()
]

// Script receives the pending list and iterates, calling:
svg_element.serialize_and_cache_subtree()
// where svg_element is the SVGSVGElement with uuid = "b3c8d2f4-..."
```

---

## 5. Stage 4 — Subtree Serialization

**Pass:** P2 (Pass 2)
**Thread:** Script Thread
**Key files:**
- [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)

### Purpose

Serialize the entire SVG subtree into an XML string, encode it as base64, and wrap it in a `data:image/svg+xml;base64,...` URL. This data URL is then cached on the `SVGSVGElement` for subsequent layout passes to consume.

**Key functions:**

| Function | Role |
|----------|------|
| `serialize_and_cache_subtree()` | Top-level serialization orchestrator |
| `process_use_elements()` | Handles `<use>` elements by cloning referenced subtrees before serialization |
| `xml_serialize()` | Generates XML string from the DOM subtree |
| `base64::engine::general_purpose::STANDARD.encode()` | Encodes XML to base64 |
| `ServoUrl::parse()` | Parses the `data:` URL string into a `ServoUrl` |
| `cleanup_cloned_nodes()` | Removes `<use>`-cloned nodes after serialization |

**Operations in detail:**
1. **`process_use_elements()`**: Scans for `<use>` elements (none in our test case) — no-op
2. **`xml_serialize()`**: Walks the SVG subtree and produces an XML string
3. **`base64::encode()`**: Encodes the XML bytes into base64
4. **`ServoUrl::parse()`**: Wraps as `data:image/svg+xml;base64,...` and parses into a valid URL
5. **`cleanup_cloned_nodes()`**: Removes temporary `<use>` clones (none in our case)
6. Sets `cached_serialized_data_url = Some(Ok(url))` on the `SVGSVGElement`
7. Calls `node.dirty(NodeDamage::Other)` to trigger another reflow (P3)

**Concrete values for our test case:**

```rust
// xml_serialize() output (<svg> root + <circle> child):
let xml_source: String = "\
<svg xmlns=\"http://www.w3.org/2000/svg\" \
     width=\"200\" height=\"200\" \
     viewBox=\"0 0 200 200\">\
  <circle cx=\"100\" cy=\"100\" r=\"50\" fill=\"blue\"/>\
</svg>".to_owned();
// Approximately 231 bytes of XML

// base64 encoding of the XML bytes:
let base64_encoded: String = "\
PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmci\
IHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAg\
MjAwIDIwMCI+PGNpcmNsZSBjeD0iMTAwIiBjeT0iMTAwIiByPSI1\
MCIgZmlsbD0iYmx1ZSIvPjwvc3ZnPg==".to_owned();
// Approximately 334 base64 characters

// Complete data URL:
let data_url_str: String = "\
data:image/svg+xml;base64,\
PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmci\
IHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAg\
MjAwIDIwMCI+PGNpcmNsZSBjeD0iMTAwIiBjeT0iMTAwIiByPSI1\
MCIgZmlsbD0iYmx1ZSIvPjwvc3ZnPg==".to_owned();

// Parsed into ServoUrl and stored:
*self.cached_serialized_data_url.borrow_mut() = Some(Ok(
    ServoUrl::parse(&data_url_str).unwrap()
));

// After serialization, the SVGSVGElement's state:
SVGSVGElement {
    uuid: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    cached_serialized_data_url: DomRefCell::new(
        Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))
    ),
    // ... other fields unchanged
}

// Dirty flag triggers reflow (P3):
node.dirty(NodeDamage::Other);  // → PendingRestyles → reflow
```

---

## 6. Stage 5 — Cache Wait

**Pass:** P3 (Pass 3)
**Thread:** Layout Thread + Image Cache Thread (async)

### Purpose

Re-enter layout now that the data URL exists. Request the image from the image cache using the data URL. The image cache has not yet loaded the data URL, so this pass results in a cache miss — triggering an async load on the image cache thread.

### 5.1: svg_kind_size() — P3 (Cache Wait Re-entry)

**Purpose:** Attempt to retrieve the image from the cache using the data URL; on cache miss, enqueue an async load and prepare for retry.

**Key functions:**

| Function | Role |
|----------|------|
| `svg_kind_size()` | Calls `get_cached_image_for_url()` with the data URL |
| `get_cached_image_for_url()` | Queries the image cache; returns `Err(Pending)` if not yet loaded |

**Operations in detail:**
1. Layout re-enters `svg_kind_size()` triggered by the dirty flag from Stage 4
2. `SVGElementData::data()` reads `cached_serialized_data_url = Some(Ok(url))`
3. `source = Some(url)` — now passes to `get_cached_image_for_url()`
4. Image cache returns `Err(Pending)` — the data URL hasn't been fetched yet
5. `vector_image = None` — no image available
6. Returns `Replaced(SVGElement(None))` → `make_fragments()` returns empty `vec![]`
7. The layout triggers an async load request to the image cache thread

**Concrete values for our test case:**

```rust
// SVGElementData::data() returns source=Some(url) now:
SVGElementData {
    svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    source: Some(Ok(ServoUrl("data:image/svg+xml;base64,..."))),  // ← now available!
    // width, height, view_box unchanged from P1
}

// get_cached_image_for_url() returns Err(Pending):
// The image cache has not fetched/parsed the data URL yet.
let cached_image: Result<Arc<ImageResource>, ImageCacheErr> = Err(
    ImageCacheErr::Pending(PendingImageId(1))
);

// Because cached_image is Err, vector_image is None:
let vector_image: Option<VectorImage> = None;

// So svg_kind_size returns:
Replaced(SVGElement(None))
// → make_fragments returns vec![] → no display item
```

### 5.2: Async Image Cache Load

**Purpose:** Fetch the data URL, decode the SVG XML into a `usvg::Tree`, and store it as a `VectorImage` in the image cache.

**Key functions:**

| Function | Role |
|----------|------|
| `service_thread()` | Background thread that processes image load requests |
| `complete_load()` | Handles the loaded result; for vector images, stores the `usvg::Tree` |

**Operations in detail:**
1. Image cache thread fetches the `data:image/svg+xml;base64,...` URL
2. Decodes the base64 content → XML bytes
3. Parses with `usvg` → `usvg::Tree` with natural dimensions 200×200
4. Stores as `VectorImage` in the `vector_images` map
5. `svg_id → image_id` mapping stored for later lookup
6. Notifies the pipeline → triggers another reflow (P4)

**Concrete values for our test case:**

```rust
// Image cache assigns a PendingImageId:
let image_id: PendingImageId = PendingImageId(1);  // first SVG image

// The data URL is fetched and base64 decoded back to XML:
// (same XML as in Stage 4, ~231 bytes)

// usvg parses the XML into a tree:
let svg_tree: usvg::Tree = usvg::Tree::from_xml(&xml_bytes, &opts).unwrap();
// svg_tree contains:
//   - <svg viewBox="0 0 200 200" width="200" height="200">
//   -   <circle cx="100" cy="100" r="50" fill="blue"/>
//   - Natural dimensions: 200×200

// Stored in image cache's vector_images map:
vector_images: HashMap<PendingImageId, LoadedVectorImage> = {
    PendingImageId(1) => LoadedVectorImage {
        svg_tree: usvg::Tree { /* parsed SVG tree */ },
        metadata: ImageMetadata { width: 200, height: 200 },
        svg_id: Some("b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()),
    }
}

// svg_id → image_id mapping:
svg_to_image_id: HashMap<String, PendingImageId> = {
    "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b" => PendingImageId(1),
}
```

---

## 7. Stage 6 — Vector Rasterization

**Pass:** P4 (Pass 4)
**Thread:** Image Cache Thread

### Purpose

Take the stored `usvg::Tree` and rasterize it into a pixel buffer using `resvg` and `tiny_skia`. The resulting `RasterImage` is cached by key for future re-use.

**Key functions:**

| Function | Role |
|----------|------|
| `rasterize_vector_image()` | Entry point: locks the store, looks up `usvg::Tree`, checks rasterized cache |
| `resvg::render()` | Renders the `usvg::Tree` to a `tiny_skia::Pixmap` |
| `load_image_with_keycache()` | Stores the rendered pixels with a WebRender-compatible cache key |

**Operations in detail:**
1. Called when layout requests rasterization for `(PendingImageId(1), size=(200,200))`
2. Locks the store, looks up `usvg::Tree` by `PendingImageId(1)`
3. Checks the rasterized cache — on first call, **cache miss**
4. Spawns a thread pool task for async rasterization
5. On completion: renders SVG to pixmap, builds `RasterImage`, caches it
6. On re-entry (after notification): cache **hit** → returns `Some(RasterImage)` with `ImageKey`

**Concrete values for our test case:**

**First call (cache miss — async):**

```rust
// Called from layout:
rasterize_vector_image(id: PendingImageId(1), size: (200, 200));

// Inside: looks up usvg::Tree by PendingImageId(1):
let svg_tree: &usvg::Tree = &vector_images[&PendingImageId(1)].svg_tree;

// Checks rasterized cache → miss for key (PendingImageId(1), (200,200)):
cache: HashMap<(PendingImageId, (u32, u32)), RasterImage> = { /* empty */ }

// Spawns thread pool task:
//   Creates tiny_skia Pixmap:
let mut pixmap = tiny_skia::Pixmap::new(200, 200).unwrap();
//   pixmap: 200 × 200 × 4 bytes = 160,000 bytes (RGBA)

//   resvg renders the SVG tree onto the pixmap:
resvg::render(&svg_tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

//   Builds RasterImage:
RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc<[u8]>( /* 160,000 bytes of RGBA pixel data */ ),
    id: None,  // ImageKey not yet assigned
}

//   load_image_with_keycache() stores to key cache, assigns WR key:
//   → RasterImage.id = Some(ImageKey(IdNamespace(1), 91))

// Returns None to layout (result not ready yet):
Return: None  // layout retries on next reflow
```

**Re-entry (cache hit — synchronous):**

```rust
rasterize_vector_image(id: PendingImageId(1), size: (200, 200));

// Checks rasterized cache → HIT for (PendingImageId(1), (200,200)):
let cached = RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc<[u8]>(/* 160,000 bytes */),
    id: Some(ImageKey(IdNamespace(1), 91)),  // WebRender image key assigned!
}

// Returns immediately:
Return: Some(RasterImage { /* with ImageKey(1, 91) */ })
```

---

## 8. Stage 7 — Cache Response

**Pass:** P4 (Pass 4)
**Thread:** Layout Thread

### Purpose

Re-enter layout from the image cache notification. This time `get_cached_image_for_url()` returns `DataAvailable` — the `VectorImage` is ready. `svg_kind_size()` now returns `Replaced(SVGElement(Some(VectorImage)))`.

**Key functions:**

| Function | Role |
|----------|------|
| `svg_kind_size()` | Calls `get_cached_image_for_url()` again; this time returns available data |
| `get_cached_image_for_url()` | Returns `DataAvailable` with the `VectorImage` |

**Operations in detail:**
1. Image cache notification triggers another reflow
2. Layout enters `svg_kind_size()` for the third time
3. `source = Some(url)` → `get_cached_image_for_url()` → now returns **available**
4. `VectorImage` is retrieved with `svg_id` mapping intact
5. `vector_image = Some(VectorImage { id: PendingImageId(1), metadata: 200×200, svg_id })`

**Concrete values for our test case:**

```rust
// get_cached_image_for_url() now returns OK:
let cached_image: Result<Arc<ImageResource>, ImageCacheErr> = Ok(
    Arc::new(ImageResource::DataAvailable(
        VectorImage {
            id: PendingImageId(1),
            metadata: ImageMetadata { width: 200, height: 200 },
            svg_tree: usvg::Tree { /* ... */ },
            svg_id: Some("b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()),
        }
    ))
);

// Because cached_image is Ok, vector_image is Some:
let vector_image: Option<ReplacedVectorImage> = Some(
    ReplacedVectorImage {
        id: PendingImageId(1),
        metadata: ImageMetadata { width: 200, height: 200 },
        svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned(),
    }
);

// So svg_kind_size returns:
Replaced(SVGElement(Some(vector_image)))
// → make_fragments now has an image to work with!
```

---

## 9. Stage 8 — Fragment Construction & Display List

**Pass:** P4 (Pass 4)
**Thread:** Layout Thread
**Key files:**
- [components/layout/replaced.rs](../../components/layout/replaced.rs)

### Purpose

Build a concrete `Fragment::Image` with the image key and dimensions for the display list. This is the final layout step — after this, the SVG is rendered on screen.

**Key functions:**

| Function | Role |
|----------|------|
| `make_fragments()` | Constructs `Fragment` entries from `ReplacedContentKind::SVGElement` |
| `rasterize_vector_image()` | Requests rasterization from image cache (may be cached or trigger async) |
| `push_image()` | Adds the image to the WebRender display list |

**Operations in detail:**
1. `make_fragments()` receives `ReplacedContentKind::SVGElement(Some(vector_image))`
2. Checks `vector_image.is_some() = true` → proceeds to fragment construction
3. Sets `base.rect` from `vector_image.metadata` → `200 × 200`
4. Calls `rasterize_vector_image(PendingImageId(1), (200,200))` → retrieves `RasterImage` with `ImageKey`
5. Extracts `ImageKey(IdNamespace(1), 91)` from the rasterized result
6. Constructs `Fragment::Image` with the image key and rect
7. Fragment enters the fragment tree → converted to display list item
8. `push_image(ImageKey(1, 91), rect)` → WebRender receives the display command
9. GPU renders the blue circle pixels at the specified position and size

**Concrete values for our test case:**

```rust
// make_fragments() is called with vector_image=Some:
let kind = ReplacedContentKind::SVGElement(Some(vector_image));

// Fragment rect computed from metadata + viewBox:
let base.rect: PhysicalRect<Au> = PhysicalRect {
    origin: PhysicalPoint { x: Au(0), y: Au(0) },    // positioned at origin
    size:   PhysicalSize  { width: Au(12000), height: Au(12000) },  // 200×200
};

// rasterize_vector_image() returns RasterImage with key:
// (second call — cache hit, synchronous)
RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc::new([/* 160,000 RGBA bytes of blue circle */]),
    id: Some(ImageKey(IdNamespace(1), 91)),
}

// The ImageKey is pulled from RasterImage.id:
let image_key: Option<WebRenderImageKey> = Some(ImageKey(IdNamespace(1), 91));

// Final Fragment::Image:
Fragment::Image {
    base: BaseFragment {
        info: BaseFragmentInfo { /* ... */ },
        style: Arc::clone(&computed_values),
        rect: PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) },
        clip: PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) },
        // ...
    },
    image_key: Some(ImageKey(IdNamespace(1), 91)),
    image_rendering: ImageRendering::Auto,
    showing_broken_image_icon: false,
}

// Display list command → WebRender:
display_list.push_image(
    ClipId::root(),
    LayoutRect::new(
        LayoutPoint { x: 0.0, y: 0.0 },
        LayoutSize  { width: 200.0, height: 200.0 },
    ),
    ImageKey(IdNamespace(1), 91),
    ImageRendering::Auto,
);
// → GPU renders a 200×200 blue circle
```

---

## 10. The Four-Pass Retry Loop

The SVG rendering pipeline relies on a **multi-pass retry loop** because data dependencies span threads. Each pass checks whether its prerequisite data is ready; if not, it triggers the work needed and re-schedules.

### Why 4 Passes?

```
Pass 1: DOM exists, but no image source → queue serialization
              ↓ (post-reflow)
Pass 2: Script serializes SVG → data URL → dirty flag → reflow
              ↓
Pass 3: Data URL exists, but image not cached yet → load → reflow
              ↓
Pass 4: VectorImage cached → rasterize → Fragment::Image → DONE
```

### Central Role of `svg_kind_size()`

The function `svg_kind_size()` is the critical branching point. It runs in **P1, P3, and P4** and behaves differently each time based on the state of two values:

| Pass | `source` | `cached_image` | `vector_image` | Outcome |
|------|----------|----------------|----------------|---------|
| **P1** | `None` | — | `None` | → Queue serialization |
| **P3** | `Some(Ok(data:url))` | `Err(Pending)` | `None` | → Load image, retry |
| **P4** | `Some(Ok(data:url))` | `DataAvailable(VectorImage{200×200})` | `Some(VectorImage)` | → Rasterize + Fragment |

### Concrete Value Progression Through Passes

```
PASS 1                                  PASS 2
svg_kind_size()                         serialize_and_cache_subtree()
┌──────────────────────┐                ┌──────────────────────────────┐
│ source = None         │                │ XML   = "<svg ...>...</svg>"│
│ vector_image = None   │  → queue →    │ base64 = "PHN2ZyB4bWxuc..." │
│ NaturalSizes(200×200) │                │ URL   = data:image/svg+...  │
└──────────────────────┘                └──────────────────────────────┘
                                                  │ cached_serialized
                                                  ▼ _data_url = Some(Ok)
PASS 3                                  PASS 4
svg_kind_size()                         svg_kind_size()
┌──────────────────────┐                ┌──────────────────────────────┐
│ source = Some(Ok)    │                │ source = Some(Ok)            │
│ cached = Err(Pending)│  → load →     │ cached = DataAvailable       │
│ vector_image = None  │               │ vector_image = Some(Vector)  │
└──────────────────────┘               └──────────┬───────────────────┘
                                                   │
                                                   ▼
                                        rasterize → Fragment::Image(200×200)
                                        → Display List → GPU shows blue circle
```

### Retry Triggers

Each pass is triggered by a specific event that sets a dirty flag and requests reflow:

| Trigger | Source | Destination |
|---------|--------|-------------|
| P1: `queue_svg_element_for_serialization()` | Layout → post-reflow hook | Script |
| P2: `node.dirty(NodeDamage::Other)` | Script | Layout |
| P3: Image cache `complete_load()` for `PendingImageId(1)` | Image Cache | Layout |
| P4: Rasterized image stored in keycache with `ImageKey(1, 91)` | Image Cache | Layout |

---

## 11. Key Values Through the Pipeline

A complete table showing which values exist (✅) or do not exist (❌) at each pass, with concrete values from our test case.

### Pre-Pass (Script) — After Stage 1.1–1.6

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `uuid` | ✅ | `"b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b"` |
| `cached_serialized_data_url` | ✅ `None` | `DomRefCell::new(None)` |
| `width` | ✅ `LengthPercentage` | `LengthPercentage::Length(Px(200.0))` |
| `height` | ✅ `LengthPercentage` | `LengthPercentage::Length(Px(200.0))` |
| `viewBox` | ✅ | `"0 0 200 200"` |

### Pre-Pass (Layout) — After Stage 1.7

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `ElementData.styles` | ✅ `Arc<ComputedValues>` | `display: inline`, `width: 200px`, `height: 200px` |
| Style cache | ✅ Cached until mutation | No recomputation on P3/P4 |

### P1 (Pass 1) — After Stage 2

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `source` | ✅ `None` | `None` — not serialized yet |
| `vector_image` | ✅ `None` | `None` — unavailable |
| `NaturalSizes` | ✅ | `{ width: Au(12000), height: Au(12000), ratio: 1.0 }` |
| `ReplacedContentKind` | ✅ | `SVGElement { vector_image: None, has_viewbox: true }` |
| `Fragment::Image` | ❌ Not produced | `make_fragments()` returns `vec![]` |

### P2 (Pass 2) — After Stage 4

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `cached_serialized_data_url` | ✅ `Some(Ok(...))` | `Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))` |
| `xml_serialize()` output | ✅ | `"<svg xmlns=...><circle cx=100 .../></svg>"` (~231 bytes) |
| base64 output | ✅ | `"PHN2ZyB4bWxucz0i..."` (~334 chars) |
| Dirty flag | ✅ | `NodeDamage::Other` → triggers P3 reflow |

### P3 (Pass 3) — After Stage 5

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `source` | ✅ `Some(Ok(url))` | `Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))` |
| `cached_image` | ❌ `Err(Pending)` | `Err(Pending(PendingImageId(1)))` — not yet loaded |
| `vector_image` | ❌ `None` | `None` — still unavailable |
| `Fragment::Image` | ❌ Not produced | `make_fragments()` returns `vec![]` → retry |

### P4 (Pass 4) — After Stage 7 (svg_kind_size)

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `source` | ✅ `Some(Ok(url))` | Same data URL |
| `cached_image` | ✅ `DataAvailable` | `Ok(Arc::new(DataAvailable(VectorImage{id:PendingImageId(1), 200×200})))` |
| `vector_image` | ✅ `Some(VectorImage)` | `Some(VectorImage { id: PendingImageId(1), metadata: 200×200, svg_id })` |

### P4 (Pass 4) — After Stage 8 (make_fragments)

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `vector_image` | ✅ `Some` | Same as above |
| `base.rect` | ✅ | `PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) }` |
| `image_key` | ✅ `Some(ImageKey)` | `ImageKey(IdNamespace(1), 91)` |
| `Fragment::Image` | ✅ Constructed | `Fragment::Image { image_key: Some(ImageKey(1, 91)), rect: 200×200, ... }` |

### P4 (Pass 4) — Rasterization (Image Cache)

| Value | Status | Concrete Value |
|-------|--------|----------------|
| `Pixmap` | ✅ | `tiny_skia::Pixmap(200×200) = 160,000 bytes RGBA` |
| `RasterImage` | ✅ | `{ metadata: 200×200, format: RGBA8, bytes: 160000B, id: Some(ImageKey(1, 91)) }` |
| `keycache` | ✅ Stored | Cached at key `((PendingImageId(1), (200,200)), RasterImage)` |
| Re-entry | ✅ Cache hit | `ImageKey` returned immediately — no re-rasterization |

---

## 12. Pass Reference

### Pass 1 (P1)

| Attribute | Detail |
|-----------|--------|
| **Trigger** | `PendingRestyles` from DOM creation |
| **Thread** | Layout |
| **Stages** | Stage 2 (traverse_element, svg_kind_size) |
| **Key functions** | `traverse_element()`, `Contents::for_element()`, `svg_kind_size()`, `queue_svg_element_for_serialization()` |
| **Concrete input** | `source=None`, `uuid="b3c8d2f4-..."`, `NaturalSizes(12000, 12000, 1.0)` |
| **Concrete output** | `Replaced(SVGElement(None))` → serialization queued, no fragment |

### Pass 2 (P2)

| Attribute | Detail |
|-----------|--------|
| **Trigger** | Post-reflow hook picks up pending SVG UUID `"b3c8d2f4-..."` |
| **Thread** | Script |
| **Stages** | Stage 4 (serialize_and_cache_subtree) |
| **Key functions** | `serialize_and_cache_subtree()`, `xml_serialize()`, `base64::encode()` |
| **Concrete input** | SVGSVGElement with `cached_serialized_data_url: None` |
| **Concrete output** | `cached_serialized_data_url = Some(Ok(data:image/svg+xml;base64,...))` → dirty flag → P3 reflow |

### Pass 3 (P3)

| Attribute | Detail |
|-----------|--------|
| **Trigger** | `NodeDamage::Other` from P2 serialization |
| **Thread** | Layout + Image Cache (async) |
| **Stages** | Stage 5 (svg_kind_size re-entry, image cache load) |
| **Key functions** | `svg_kind_size()`, `get_cached_image_for_url()`, `complete_load()` |
| **Concrete input** | `source=Some(Ok(data:url))`, `cached_image=Err(Pending)` |
| **Concrete output** | `cached_image=Err(Pending(PendingImageId(1)))` → async load started, no fragment |

### Pass 4 (P4)

| Attribute | Detail |
|-----------|--------|
| **Trigger** | Image cache notification (VectorImage loaded for `PendingImageId(1)`) |
| **Thread** | Layout + Image Cache |
| **Stages** | Stage 6 (rasterize), Stage 7 (cache response), Stage 8 (make_fragments) |
| **Key functions** | `svg_kind_size()`, `rasterize_vector_image()`, `make_fragments()`, `push_image()` |
| **Concrete input** | `source=Some(Ok(data:url))`, `cached_image=DataAvailable` |
| **Concrete output** | `vector_image=Some(VectorImage)` → `RasterImage{200×200, RGBA8}` → `Fragment::Image{ImageKey(1,91)}` → blue circle on screen ✅ |

---

## 13. Class Hierarchy & Details

This section documents every important class, struct, and enum involved in the SVG rendering pipeline, organized by thread and layer. Each entry includes the inheritance chain, field descriptions, and the stages where the class is active.

### 13.1 Inheritance Tree

The full DOM inheritance chain for `SVGSVGElement`:

```
Node (components/script/dom/node)
  └── Element (components/script/dom/element)
        └── SVGElement (components/script/dom/svg/svgelement.rs)
              └── SVGGraphicsElement (components/script/dom/svg/svggraphicselement.rs)
                    └── SVGSVGElement (components/script/dom/svg/svgsvgelement.rs)
```

Layout and image cache types are standalone structs/enums — they do not form an inheritance tree.

### 13.2 DOM Classes (Script Thread)

These classes live on the **script thread** and are managed by Servo's SpiderMonkey GC (with `#[dom_struct]`).

---

#### `Node`

**File:** [components/script/dom/node.rs](../../components/script/dom/node.rs)
**Role:** Base class for all DOM nodes. Provides tree operations (parent/child traversal, insertion, removal).

**Key fields relevant to SVG:**

| Field | Type | Description |
|-------|------|-------------|
| `owner_doc` | `Dom<Document>` | The document this node belongs to |
| `parent_node` | `DomRefCell<Option<Dom<Node>>>` | Parent in the DOM tree |
| `child_list` | `DomRefCell<Vec<Dom<Node>>>` | Children in the DOM tree |
| `node_data` | `DomRefCell<NodeData>` | Type-specific data and flags |
| `damage` | `DomRefCell<NodeDamage>` | Layout damage flags (drives reflow) |

**Stages:** All stages (foundation of the DOM tree)

---

#### `Element`

**File:** [components/script/dom/element.rs](../../components/script/dom/element.rs)
**Role:** Base class for all HTML and SVG elements.

**Key fields relevant to SVG:**

| Field | Type | Description |
|-------|------|-------------|
| `node` | `Node` | Inherited node base |
| `local_name` | `LocalName` | Element tag name (e.g. `"svg"`, `"circle"`) |
| `namespace` | `Namespace` | Element namespace (e.g. `ns!(svg)`) |
| `attrs` | `DomRefCell<Vec<Dom<Attr>>>` | List of attributes |
| `attr_values` | `DomRefCell<HashMap<LocalName, AttrValue>>` | Parsed attribute values indexed by name |

**Stages:** All stages (attribute access, mutation handling)

---

#### `SVGElement`

**File:** [components/script/dom/svg/svgelement.rs](../../components/script/dom/svg/svgelement.rs)
**Role:** Base class for all SVG-specific elements. Inherits from `Element`.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `element` | `Element` | Inherited element base |
| `style_decl` | `MutNullableDom<CSSStyleDeclaration>` | Cached style declaration object |

**Stages:** Stage 1 (construction), handled generically elsewhere

---

#### `SVGGraphicsElement`

**File:** [components/script/dom/svg/svggraphicselement.rs](../../components/script/dom/svg/svggraphicselement.rs)
**Role:** Base class for SVG elements that can be rendered (graphics). Inherits from `SVGElement`.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `svgelement` | `SVGElement` | Inherited SVG element base |

**Stages:** Stage 1 (construction)

---

#### `SVGSVGElement`

**File:** [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
**Role:** The `<svg>` root element. Holds the serialized subtree cache that drives the pipeline.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `svggraphicselement` | `SVGGraphicsElement` | Inherited graphics element base |
| `uuid` | `String` | Unique identifier (`Uuid::new_v4()`) used as cache key for rasterization |
| `cached_serialized_data_url` | `DomRefCell<Option<Result<ServoUrl, ()>>>` | Cached data URL of the serialized SVG subtree. `None` = not serialized, `Some(Ok(url))` = ready, `Some(Err(()))` = serialization failed |

**Stages:** Stage 1 (construction), Stage 2 (data access via `LayoutDom`), Stage 4 (serialization), Stage 5+ (read-only via cached URL)

**Concrete value after Stage 4:**
```rust
SVGSVGElement {
    uuid: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b",
    cached_serialized_data_url: DomRefCell::new(
        Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))
    ),
}
```

---

#### `AttrValue` (enum)

**File:** [components/script/dom/attr.rs](../../components/script/dom/attr.rs)
**Role:** Parsed attribute values. Used to store SVG attributes in a type-safe way.

**Variants used in SVG:**

| Variant | SVG Usage | Example Value |
|---------|-----------|---------------|
| `String(String)` | Plain attributes (viewBox, xmlns) | `"0 0 200 200"` |
| `LengthPercentage(String, Option<LengthPercentage>)` | width, height | `("200", Some(LengthPercentage::Length(Px(200.0))))` |
| `UInt(String, u32)` | Integer attributes | `("1", 1)` for `column-count` |

**Stages:** Stage 1 (parsed during DOM construction), Stage 2 (read via `get_attr_for_layout`)

---

#### `NodeDamage` (bitflags)

**File:** [components/script/dom/node.rs](../../components/script/dom/node.rs)
**Role:** Tracks what kind of layout damage a node has experienced. Determines what needs re-computation.

**Relevant variants:**

| Variant | Meaning | Used By |
|---------|---------|---------|
| `NodeDamage::Other` | Generic damage — triggers full reflow | Stage 4 (after serialization) |
| `NodeDamage::Style` | CSS style changed | Stage 1 |
| `NodeDamage::Node` | Node structure changed (insert/remove) | Stage 1 |

**Stages:** Stage 4 (set to `Other` after serialization)

---

### 13.3 Layout Classes (Layout Thread)

These classes live on the **layout thread** and are read-only borrows from the script DOM (via `LayoutDom`) or local layout types.

---

#### `SVGElementData<'dom>`

**File:** [components/shared/layout/lib.rs](../../components/shared/layout/lib.rs)
**Role:** Carries SVG element data from the DOM (script thread) to the layout system. This is the **central data structure** that determines what happens in `svg_kind_size()`.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `svg_id` | `String` | UUID string from `SVGSVGElement.uuid` — used for cache key matching |
| `source` | `Option<Result<ServoUrl, ()>>` | The serialized data URL. `None` = P1, `Some(Ok(url))` = P3/P4, `Some(Err(()))` = serialization failed |
| `width` | `Option<&'dom AttrValue>` | Width attribute value (as `LengthPercentage`) |
| `height` | `Option<&'dom AttrValue>` | Height attribute value (as `LengthPercentage`) |
| `view_box` | `Option<&'dom AttrValue>` | viewBox attribute value (string `"min-x min-y w h"`) |

**Stages:** Stage 2, Stage 5, Stage 7 (read in `svg_kind_size()` each pass)

**Value progression through passes:**

```rust
// P1: source=None, no data URL yet
SVGElementData { svg_id: "b3c8d2f4-...", source: None, width: Some(...), height: Some(...), view_box: Some("0 0 200 200") }

// P3/P4: source=Some(Ok(url)), data URL available
SVGElementData { svg_id: "b3c8d2f4-...", source: Some(Ok(data_url)), width: Some(...), height: Some(...), view_box: Some("0 0 200 200") }
```

---

#### `NaturalSizes`

**File:** [components/layout/replaced.rs](../../components/layout/replaced.rs)
**Role:** Stores the natural (intrinsic) dimensions of a replaced element.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `width` | `Au` | Natural width in app units (1px = 60 Au) |
| `height` | `Au` | Natural height in app units |
| `ratio` | `f32` | Aspect ratio (width/height) |

**Concrete value for our test case:**
```rust
NaturalSizes { width: Au(12000), height: Au(12000), ratio: 1.0 }
```

**Stages:** Stage 2 (computed in `svg_kind_size` P1)

---

#### `ReplacedContentKind` (enum)

**File:** [components/layout/replaced.rs](../../components/layout/replaced.rs)
**Role:** Classifies what kind of replaced content an element produces. SVG uses the `SVGElement` variant.

**Relevant variant:**

```rust
enum ReplacedContentKind {
    SVGElement {
        vector_image: Option<ReplacedVectorImage>,
        has_viewbox: bool,
    },
    Image(/* ... */),
    // ... other variants for iframe, video, canvas, etc.
}
```

**Variant fields:**

| Field | Type | Description |
|-------|------|-------------|
| `vector_image` | `Option<ReplacedVectorImage>` | `None` in P1/P3, `Some(VectorImage)` in P4 |
| `has_viewbox` | `bool` | Whether the SVG element has a `viewBox` attribute |

**Stages:** Stage 2 (created in `Contents::for_element`), Stage 5 (re-created P3), Stage 7 (re-created P4), Stage 8 (consumed by `make_fragments`)

**Value progression:**
```rust
// P1: 
SVGElement { vector_image: None, has_viewbox: true }

// P3:
SVGElement { vector_image: None, has_viewbox: true }

// P4:
SVGElement { vector_image: Some(vector_image), has_viewbox: true }
```

---

#### `ReplacedVectorImage`

**File:** [components/layout/replaced.rs](../../components/layout/replaced.rs)
**Role:** Wraps a loaded vector image for the layout thread. Produced from the image cache response.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | `PendingImageId` | Image ID in the cache (e.g., `PendingImageId(1)`) |
| `metadata` | `ImageMetadata` | Natural dimensions from the cached image (e.g., `200×200`) |
| `svg_id` | `String` | UUID of the originating SVGSVGElement |

**Concrete value in P4:**
```rust
ReplacedVectorImage {
    id: PendingImageId(1),
    metadata: ImageMetadata { width: 200, height: 200 },
    svg_id: "b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned(),
}
```

**Stages:** Stage 7 (created from cache response), Stage 8 (consumed by `make_fragments`)

---

#### `Fragment` (enum)

**File:** [components/layout/fragment_tree/fragment.rs](../../components/layout/fragment_tree/fragment.rs)
**Role:** Represents a piece of rendered content in the fragment tree. SVG produces the `Image` variant.

**Relevant variant:**

```rust
enum Fragment {
    Image {
        base: BaseFragment,
        image_key: Option<WebRenderImageKey>,
        image_rendering: ImageRendering,
        showing_broken_image_icon: bool,
    },
    // ... other variants
}
```

**Key fields of the `Image` variant:**

| Field | Type | Description |
|-------|------|-------------|
| `base` | `BaseFragment` | Base fragment data (rect, style, clip) |
| `base.rect` | `PhysicalRect<Au>` | Position and size in layout coordinates |
| `image_key` | `Option<WebRenderImageKey>` | WebRender image handle — `None` if async, `Some(key)` when ready |
| `image_rendering` | `ImageRendering` | Rendering quality hint |
| `showing_broken_image_icon` | `bool` | Whether to show broken image placeholder |

**Concrete value in P4:**
```rust
Fragment::Image {
    base: BaseFragment {
        rect: PhysicalRect { x: Au(0), y: Au(0), width: Au(12000), height: Au(12000) },
        // ...
    },
    image_key: Some(ImageKey(IdNamespace(1), 91)),
    image_rendering: ImageRendering::Auto,
    showing_broken_image_icon: false,
}
```

**Stages:** Stage 8 (produced by `make_fragments`)

---

#### `BaseFragment`

**File:** [components/layout/fragment_tree/fragment.rs](../../components/layout/fragment_tree/fragment.rs)
**Role:** Shared data for all fragment types (position, size, style, border, padding, etc.).

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `rect` | `PhysicalRect<Au>` | Content rectangle in physical layout coordinates |
| `clip` | `PhysicalRect<Au>` | Clipping rectangle |
| `style` | `ServoArc<ComputedValues>` | Computed style for this fragment |
| `info` | `BaseFragmentInfo` | Metadata (node pointer, flags) |

---

### 13.4 Image Cache Classes

These classes live on the **image cache thread** or are shared between layout and image cache via `Arc`.

---

#### `PendingImageId`

**File:** [components/shared/net/image_cache.rs](../../components/shared/net/image_cache.rs)
**Role:** Opaque identifier for a pending image in the cache. Increments from 1.

```rust
struct PendingImageId(pub u32);
```

**Concrete value:** `PendingImageId(1)` (first SVG image in the session)

**Stages:** Stage 5 (assigned on cache miss), Stage 6 (used for rasterization lookup), Stage 7 (returned in response)

---

#### `ImageResource` (enum)

**File:** [components/shared/net/image_cache.rs](../../components/shared/net/image_cache.rs)
**Role:** Represents the state of an image in the cache.

**Relevant variants:**

```rust
enum ImageResource {
    Pending(PendingImageId),
    DataAvailable(Arc<ImageResourceData>),
    LoadError,
    NotRequested,
}
```

| Variant | Meaning | Pass |
|---------|---------|------|
| `Pending(id)` | Load in progress, not ready | P3 |
| `DataAvailable(data)` | Image loaded and ready | P4 (Stage 7) |
| `LoadError` | Loading failed | Never (for SVG) |
| `NotRequested` | Not yet queued | — |

---

#### `VectorImage` / `LoadedVectorImage`

**File:** [components/shared/net/image_cache.rs](../../components/shared/net/image_cache.rs)
**Role:** A loaded SVG vector image, containing the parsed `usvg::Tree`.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `svg_tree` | `usvg::Tree` | Parsed SVG document tree from `usvg` |
| `metadata` | `ImageMetadata` | Natural dimensions (e.g., `200×200`) |
| `svg_id` | `Option<String>` | UUID of the source SVGSVGElement |

**Concrete value:**
```rust
LoadedVectorImage {
    svg_tree: usvg::Tree { /* contains <svg viewBox="0 0 200 200"> + <circle> */ },
    metadata: ImageMetadata { width: 200, height: 200 },
    svg_id: Some("b3c8d2f4-1e5a-4d7c-9b0a-6f2e3d1c8a7b".to_owned()),
}
```

**Stages:** Stage 5 (stored in cache), Stage 7 (returned to layout)

---

#### `RasterImage`

**File:** [components/shared/net/image_cache.rs](../../components/shared/net/image_cache.rs)
**Role:** A rasterized (pixel) image produced from a `VectorImage`. This is what WebRender consumes.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `metadata` | `ImageMetadata` | Dimensions (width, height) |
| `format` | `PixelFormat` | Pixel format (always `RGBA8` for SVG) |
| `bytes` | `Arc<[u8]>` | Raw pixel data (width × height × 4 bytes) |
| `id` | `Option<WebRenderImageKey>` | WebRender image key — `None` before key assignment, `Some(key)` after |

**Concrete value:**
```rust
RasterImage {
    metadata: ImageMetadata { width: 200, height: 200 },
    format: PixelFormat::RGBA8,
    bytes: Arc::new([/* 160,000 RGBA bytes */]),
    id: Some(ImageKey(IdNamespace(1), 91)),
}
```

**Stages:** Stage 6 (produced by rasterization)

---

#### `WebRenderImageKey` / `ImageKey`

**File:** [components/shared/net/image_cache.rs](../../components/shared/net/image_cache.rs)
**Role:** WebRender's handle to a stored image texture. Used in display list commands.

```rust
pub struct ImageKey(pub IdNamespace, pub u32);
```

| Field | Type | Example |
|-------|------|---------|
| `namespace` | `IdNamespace` | Groups related keys — `IdNamespace(1)` for SVG |
| `id` | `u32` | Unique ID within namespace — `91` |

**Concrete value:** `ImageKey(IdNamespace(1), 91)`

**Stages:** Stage 6 (assigned during `load_image_with_keycache`), Stage 8 (placed into `Fragment::Image`)

---

### 13.5 Key Relationships Between Classes

This diagram shows how the important classes connect across thread boundaries:

```
Script Thread                      Layout Thread                    Image Cache Thread
─────────────                      ─────────────                    ──────────────────

SVGSVGElement                      SVGElementData                   PendingImageId(1)
  ├── uuid ──────────LayoutDom────►  ├── svg_id                        │
  ├── cached_serialized ──────────►  ├── source                        │
  │   _data_url                      ├── width, height                VectorImage
  │                                  └── view_box                       ├── svg_tree (usvg)
  │                                       │                            ├── metadata
  │                                       ▼                            └── svg_id
  │                               ReplacedContentKind::SVGElement
  │                                    │                              RasterImage
  │                                    ├── vector_image ──Arc──►        ├── metadata
  │                                    └── has_viewbox                  ├── format
  │                                                                     ├── bytes
  │                              ┌─────┘                               └── id → ImageKey
  │                              ▼
  │                         Fragment::Image                               │
  │                              ├── base.rect                     ImageKey(1, 91)
  │                              └── image_key ───────────────────► WebRender
  │                                                                   GPU
  ▼
Display List ──────────────────► WebRender
```

### 13.6 Thread Boundary Crossings

How data moves between threads:

| Data | From → To | Mechanism | When |
|------|-----------|-----------|------|
| `uuid` attribute values | Script → Layout | `LayoutDom<'dom, SVGSVGElement>` — layout-accessible borrow of the DOM node | Stage 2, 5, 7 (`svg_kind_size`) |
| `cached_serialized_data_url` | Script (written) → Layout (read) | Same `LayoutDom` borrow; the cell is `#[no_trace]` for GC safety | Stage 2 (read as None), Stage 5/7 (read as Some) |
| SVG UUID for serialization | Layout → Script | `pending_svg_uuids: Vec<String>` stored on `LayoutThread`, read by script in post-reflow callback | Stage 3 (dispatch) |
| `VectorImage` | Image Cache → Layout | `Arc<ImageResource>` shared pointer; returned from `get_cached_image_for_url()` | Stage 5 (async), Stage 7 (ready) |
| `RasterImage` | Image Cache → Layout | Return value of `rasterize_vector_image()` — `Option<RasterImage>` | Stage 6 (first: None, re-entry: Some) |
| `ImageKey` | Image Cache → Layout | Carried inside `RasterImage.id` | Stage 6 (assigned), Stage 8 (consumed) |
| Notifications (reflow triggers) | Any → Layout | `ScriptThread::notify()` or image cache callback → sets `PendingRestyles` | Pass transitions |

---

## 14. Thread & Ownership Model

### Object Lifetime & Thread Safety

| Class | Created | Owned By | Thread Safety | GC |
|-------|---------|----------|---------------|----|
| `SVGSVGElement` | Script (HTML parser) | Script DOM tree (GC-managed) | Not `Send`/`Sync`; layout accesses via `LayoutDom` (unsafe borrow) | `#[dom_struct]` |
| `SVGElementData` | Layout (`data()`) | Stack/local in layout | `'dom` — borrow of script data; discarded after layout pass | No |
| `ReplacedContentKind` | Layout | Stack/local in layout | Local type, no sharing | No |
| `NaturalSizes` | Layout | Stack/local in layout | Local type | No |
| `ReplacedVectorImage` | Layout (from cache) | Stack/local in layout | Created from `Arc<ImageResource>` | No |
| `Fragment` | Layout | Fragment tree (Layout Thread) | `Send` (sent to WR as display list) | No |
| `PendingImageId` | Image Cache | Shared (layout reads it) | `Copy`, used in map lookups | No |
| `VectorImage` | Image Cache | Image Cache store | `Send`/`Sync` via `Arc` | No |
| `RasterImage` | Image Cache | Rasterized cache | `Send`/`Sync` via `Arc` | No |
| `ImageKey` | WebRender | WR internal | `Copy`, opaque handle | No |

### Layout Borrow Model

Layout does not "own" DOM data — it borrows it through an unsafe mechanism:

```rust
// In SVGSVGElement (script thread):
impl<'dom> LayoutDom<'dom, SVGSVGElement> {
    pub(crate) fn data(self) -> SVGElementData<'dom> {
        let svg_id = self.unsafe_get().uuid.clone();         // unsafe: layout borrow
        let source = unsafe {                                 // unsafe: layout borrow
            self.unsafe_get().cached_serialized_data_url
                .borrow_for_layout()                          // special layout-safe borrow
                .clone()
        };
        // ...
    }
}
```

Key rules:
1. Layout can **read** DOM data but never **write** it
2. `borrow_for_layout()` is a special method that bypasses runtime borrow checking — safe because layout and script never run concurrently
3. The data is cloned (`uuid.clone()`, `source.clone()`) so layout holds its own copy

---

## 15. Cache Invalidation Reference

The pipeline relies on several cached values. This table documents when each cache is invalidated:

### Invalidation Events

| Cached Value | Location | Invalidated By | Effect |
|-------------|----------|----------------|--------|
| `cached_serialized_data_url` | `SVGSVGElement` | `attribute_mutated()` — any attribute change on `<svg>` | Sets to `None` → re-serialized on next P2 |
| `cached_serialized_data_url` | `SVGSVGElement` | `children_changed()` — child elements added/removed | Sets to `None` → re-serialized on next P2 |
| `cached_serialized_data_url` | `SVGSVGElement` | `unbind_from_tree()` — element removed from document | Sets to `None` + evicts from image cache |
| `ElementData.styles` (ComputedValues) | Per-element `ElementData` | `NodeDamage::Style` — CSS class change, inline style change | Restyle recomputation on next layout |
| `RasterImage` (rasterized cache) | Image Cache keyed by `(image_id, size)` | Not explicitly invalidated (persists for session) | — |
| `VectorImage` (parsed usvg tree) | Image Cache `vector_images` map | `evict_rasterized_image(&uuid)` called from `unbind_from_tree()` | Removes both vector image and rasterized cache entries |

### Reflow Triggers Summary

```
Event                           Damage Flag                 Triggers Pass
─────                           ───────────                 ─────────────
DOM node inserted               NodeDamage::Node            P1
Attribute changed               NodeDamage::Other           Re-serialization → P2
Children changed                NodeDamage::Other           Re-serialization → P2
Style changed                   NodeDamage::Style           Restyle + P1
P1: serialization queued        (post-reflow hook)          P2
P2: data URL cached             NodeDamage::Other           P3
P3: VectorImage loaded          (cache notification)        P4
P4: RasterImage keycache stored (cache notification)        P4 (re-entry)
```
