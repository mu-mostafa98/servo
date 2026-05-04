# Stage 2 — Style Resolution & Dispatch

> **Thread:** Script (Stylo) → Layout
> **Also known as:** The two-pass replaced content dispatch for SVG
> **Key files:**
> - [components/layout/dom_traversal.rs](../../components/layout/dom_traversal.rs)
> - [components/layout/replaced.rs](../../components/layout/replaced.rs)
> - [components/layout/formatting_contexts.rs](../../components/layout/formatting_contexts.rs)
> - [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
> - [components/script/layout_dom/servo_layout_element.rs](../../components/script/layout_dom/servo_layout_element.rs)
> - [components/shared/layout/lib.rs](../../components/shared/layout/lib.rs)

---

## Overview

Stage 2 is the most complex stage because it runs **many times** per SVG element, not just twice. There are two distinct phases:

**Phase A — The serialization barrier (passes 1→2):**
```
First layout pass:    SVGElement(None)    → queue serialization → Stage 3
                             ↓
Second layout pass:   SVGElement(Some(…)) → Stage 5 onward
```

This transition happens exactly once: `source` goes from `None` → `Some(Ok(url))`.

**Phase B — Async image pipeline (passes 3+):**
After the data URL is available, the image cache loads, parses, and rasterizes the SVG asynchronously. Each completion step triggers a re-layout, calling `svg_kind_size` again:

```
Pass 3: source=Some(url), image cache still loading   → SVGElement(None)
Pass 4: source=Some(url), usvg::Tree parsed            → SVGElement(Some(VectorImage))
Pass 5: source=Some(url), rasterization queued         → SVGElement(Some(VectorImage))
Pass 6: source=Some(url), rasterization done           → SVGElement(Some(VectorImage))
         ↓ (image key set, final layout)
```

Typical total: **6–10 calls** to `svg_kind_size` for a single SVG element.

> ⚠️ **Empirical note:** Testing with the prebuilt debug binary (HEAD commit `9ba9916bad2`) showed only **2 reflow passes** with no SVG rendering in the fragment tree. The async SVG pipeline (passes 3+) did not complete in this test environment. The user observed `svg_kind_size` called ~8 times in their own debugging session, suggesting the async pipeline completes successfully in a properly configured build. See the "Empirical Test Results" section below for details.

Sub-stages 2.1-2.5 run every time. Sub-stage 2.6 is where the branching happens — but there are *two* important branches:

---

## Sub-stage 2.1 — CSS Cascade via Stylo

**File:** [servo_layout_element.rs](../../components/script/layout_dom/servo_layout_element.rs)
**Function:** `ServoLayoutElement::style()`
**Line:** 163

Before any SVG-specific code runs, Stylo (Servo's CSS engine) resolves the full CSS cascade for the `<svg>` element.

**Process:**
1. Reads the element's `ElementData` — the already-computed style data from the Script thread's style traversal (line 199: `self.element_data()`)
2. Reads the **primary style** from `data.styles.primary()` (line 200)
3. For pseudo-elements, resolves eager/precomputed/lazy pseudo styles (lines 164-197) — not applicable for our simple SVG
4. Returns `Arc<ComputedValues>` — the fully resolved, cascaded style

**Key output:**

| Property | Typical Value | Why It Matters |
|----------|---------------|----------------|
| `display` | `block` (or `inline-block`) | Determines if this goes through replaced content path |
| `width`/`height` | `auto` (or specified) | Used by CSS sizing |
| `object-fit` | `fill` | Controls how the SVG content scales within its box |
| `visibility` | `visible` | Whether the element renders at all |
| `image-rendering` | `auto` | Affects the WebRender display list later |

**Breakpoint:** [line 199](components/script/layout_dom/servo_layout_element.rs#L199) — `let data = self.element_data();`
**Watch:** `data.styles.primary()` — the resolved ComputedValues

#### Debugging this sub-stage

**⚠ This function is called for EVERY element, on BOTH passes.**

**Breakpoints:**
- [servo_layout_element.rs:163](components/script/layout_dom/servo_layout_element.rs#L163) — `style()` function entry
- [servo_layout_element.rs:199](components/script/layout_dom/servo_layout_element.rs#L199) — `self.element_data()` reading resolved style

**SVG identification:**
You CANNOT easily filter for SVG at this breakpoint because `self` type doesn't expose the element name to the debugger. Practical strategies:

1. **Skip this breakpoint and break downstream** — the first SVG-specific breakpoint is at [replaced.rs:185](components/layout/replaced.rs#L185) (sub-stage 2.4, `as_svg()`)
2. **Use the Call Stack** — if the call stack shows `svg_kind_size()` or `ReplacedContents::for_element()` in the frames below, this style call IS for SVG
3. **Add a temporary eprintln** (requires rebuild):
   ```rust
   eprintln!("style() called for: {:?}", self.local_name());
   ```

**Call frequency:**
Called for every element in the DOM tree, depth-first. For our test page:
- **Pass 1**: html → head → meta → title → body → **svg** → circle = **7 calls**
- **Pass 2**: Same 7 elements again = **7 calls**
- **Total: 14 calls**, only 2 of which (one per pass) are for the SVG element

**Key variables to inspect:**
- `data.styles.primary()` — the fully resolved `ComputedValues` (stored as `Arc`)
- `data.styles.pseudos` — pseudo-element styles (empty for our SVG)

---

## Sub-stage 2.2 — Layout Traversal Entry

**File:** [dom_traversal.rs](../../components/layout/dom_traversal.rs)
**Function:** `traverse_element()`
**Lines:** 134-168

The layout traversal walks the DOM tree depth-first. For each element node:

```rust
fn traverse_element<'dom>(element, context, handler) {
    let style = element.style(&context.style_context);   // ← Sub-stage 2.1
    let info = NodeAndStyleInfo::new(element, style);

    match Display::from(info.style.get_box().display) {
        Display::None => {},                              // hidden, skip
        Display::Contents => { /* handle display: contents */ },
        Display::GeneratingBox(display) => {
            let contents = Contents::for_element(element, context);  // ← 2.3
            let display = display.used_value_for_contents(&contents);
            handler.handle_element(&info, display, contents, box_slot); // ← to 2.7
        },
    }
}
```

**For SVG:** Since `<svg>` is `display: block`, it enters the `GeneratingBox` arm and calls `Contents::for_element()`.

**Breakpoint:** [line 139](components/layout/dom_traversal.rs#L139) — `element.style()`
**Watch:** `style.get_box().display` — should be `GeneratingBox(OutsideInside { outside: Block, inside: Flow })`

#### Debugging this sub-stage

**⚠ Called for EVERY element, on BOTH passes.**

**Breakpoints:**
- [dom_traversal.rs:139](components/layout/dom_traversal.rs#L139) — `element.style()` (the `style()` call)
- [dom_traversal.rs:162](components/layout/dom_traversal.rs#L162) — `Contents::for_element()` call (only for generating boxes)

**SVG identification:**
Same challenge as sub-stage 2.1 — the `element` is a `ServoLayoutNode` which doesn't expose a readable name in the debugger. Use the call stack or break downstream.

**How to trace:**
When this breakpoint hits, look at the `element` variable. The debugger shows `ServoLayoutNode { node: LayoutDom<Node>, pseudo_element_chain: ... }`. You can use the **Call Stack** to verify which parent element we're inside, but the best approach is to step into `Contents::for_element()` (line 162) and see if it returns `Contents::Replaced(...)`.

**Call frequency:**
Identical to sub-stage 2.1 — one call per element per pass. The SVG element is hit on both passes.

**Expected display value for SVG:**
`style.get_box().display` for `<svg>` should return:
```
GeneratingBox(OutsideInside { outside: Block, inside: Flow })
```
This triggers the `GeneratingBox` arm, which calls `Contents::for_element()` → SVG replaced content path.

---

## Sub-stage 2.3 — Contents Type Detection

**File:** [dom_traversal.rs](../../components/layout/dom_traversal.rs)
**Function:** `Contents::for_element()`
**Lines:** 253-270

This is the **type dispatcher** — determines if the element is "replaced content" (image, SVG, iframe, video, canvas) or "non-replaced" (regular divs, spans, etc.):

```rust
pub(crate) fn for_element(node, context) -> Contents {
    if let Some(replaced) = ReplacedContents::for_element(node, context) {
        return Contents::Replaced(replaced);    // ← SVG goes here
    }
    // Otherwise: Widget or NonReplaced
}
```

**Breakpoint:** [line 254](components/layout/dom_traversal.rs#L254)
**Watch:** Return value — should be `Contents::Replaced(...)` for `<svg>`

#### Debugging this sub-stage

**⚠ Called for EVERY element, on BOTH passes.**

**Breakpoints:**
- [dom_traversal.rs:254](components/layout/dom_traversal.rs#L254) — `Contents::for_element()` entry

**SVG identification:**
This is the first place where SVG elements diverge from regular elements:
- For most elements (html, head, body, div, etc.): returns `Contents::NonReplaced(...)` or `Contents::Widget(...)`
- For replaced content elements (svg, img, iframe): returns `Contents::Replaced(...)`

**How to check at the breakpoint:**
Step INTO the function ([replaced.rs:149](components/layout/replaced.rs#L149)) and check if `node.as_svg()` returns `Some(...)`. If yes, this IS the SVG element. If `as_image()` returns `Some(...)`, it's an `<img>` tag. If none match, it's not replaced content.

**Call frequency:**
One call per element per pass. For our test page, **14 calls total** (7 elements × 2 passes). Only **2 of those** (svg element, both passes) return `Contents::Replaced(...)`.

**What to verify at this breakpoint:**
The return value for SVG should be:
```
Contents::Replaced(ReplacedContents { kind: SVGElement(None/Some(...)), ... })
```

---

## Sub-stage 2.4 — ReplacedContent Dispatch

**File:** [replaced.rs](../../components/layout/replaced.rs)
**Function:** `ReplacedContents::for_element()`
**Lines:** 149-219

The **main dispatch function** checks what kind of replaced content the node is, in priority order:

```rust
pub fn for_element(node, context) -> Option<Self> {
    let (kind, natural_size) = {
        if let Some((image_info, _)) = node.as_image() {
            // → ReplacedContentKind::Image
        } else if let Some((canvas_info, _)) = node.as_canvas() {
            // → ReplacedContentKind::Canvas
        } else if let Some(iframe_info) = node.as_iframe() {
            // → ReplacedContentKind::IFrame
        } else if let Some((video_info, _)) = node.as_video() {
            // → ReplacedContentKind::Video
        } else if let Some(svg_data) = node.as_svg() {    // ← SVG path
            Self::svg_kind_size(svg_data, context, node)   // ← to 2.5
        } else if /* <audio> */ {
            // → ReplacedContentKind::Audio
        }
    };
    Some(Self { kind, natural_size, base_fragment_info: node.into() })
}
```

**The `as_svg()` call chain:**

1. Layout trait method: [dom.rs:378](components/layout/dom.rs#L378) → `self.svg_data()`
2. Trait implementation: [servo_layout_node.rs:279](components/script/layout_dom/servo_layout_node.rs#L279) → `self.node.svg_data()`
3. Node dispatch: [node.rs:2377](components/script/dom/node/node.rs#L2377) → `self.downcast::<SVGSVGElement>().map(|svg| svg.data())`
4. SVG data builder: [svgsvgelement.rs:172](components/script/dom/svg/svgsvgelement.rs#L172) → builds `SVGElementData`

**Breakpoint:** [line 185](components/layout/replaced.rs#L185) — `node.as_svg()`
**Watch:** `svg_data` — the SVGElementData struct returned

#### Debugging this sub-stage

**✅ SVG-specific** — this path is only reached for SVG elements.

**Breakpoints:**
- [replaced.rs:185](components/layout/replaced.rs#L185) — `node.as_svg()` (the SVG dispatch check)
- [replaced.rs:221](components/layout/replaced.rs#L221) — `svg_kind_size()` entry (only if `as_svg()` returned `Some`)

**SVG identification:**
At [replaced.rs:185](components/layout/replaced.rs#L185), the code checks `node.as_svg()`. If the debugger steps into `svg_kind_size()`, this IS the SVG element. If it skips to the next `else if`, then `as_svg()` returned `None` — the node is either not SVG or is an SVG child element.

Note: Child elements inside SVG (like `<circle>`) are also in the SVG namespace but are NOT `SVGSVGElement`. The `downcast::<SVGSVGElement>()` check only matches the root `<svg>`. So **children like `<circle>` do NOT reach this breakpoint**.

**How to step through the dispatch:**
Place a breakpoint at [replaced.rs:149](components/layout/replaced.rs#L149) (`ReplacedContents::for_element` entry) and step through each `else if`:
1. `as_image()` → skip (not an img)
2. `as_canvas()` → skip (not a canvas)
3. `as_iframe()` → skip (not an iframe)
4. `as_video()` → skip (not a video)
5. **`as_svg()` → HIT!** This is the SVG element

**Call frequency:**
**Multiple times** (once per layout pass, typically 6–10 total). Always for the root `<svg>` element.

**Key variable to watch:**
`svg_data` returned from `as_svg()` — this is the `SVGElementData` struct. Check its `source` field to determine whether serialization has happened yet:
- `source: None` → **First pass** (will queue serialization)
- `source: Some(Ok(...))` → **Second pass** (will resolve from image cache)

---

## Sub-stage 2.5 — SVG Element Data Construction

**File:** [svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
**Function:** `SVGSVGElement::data()` (in `impl LayoutDom`)
**Lines:** 170-191

This builds the `SVGElementData` struct that drives the two-pass logic by reading raw SVG attributes:

```rust
pub(crate) fn data(self) -> SVGElementData<'dom> {
    let svg_id = self.unsafe_get().uuid.clone();
    //   → "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"
    let element = self.upcast::<Element>();
    let width = element.get_attr_for_layout(&ns!(), &local_name!("width"));
    //   → Some(AttrValue::LengthPercentage("200", Ok(LengthPercentage::Length(CSSPixelLength(200.0)))))
    let height = element.get_attr_for_layout(&ns!(), &local_name!("height"));
    //   → Some(AttrValue::LengthPercentage("200", Ok(LengthPercentage::Length(CSSPixelLength(200.0)))))
    let view_box = element.get_attr_for_layout(&ns!(), &local_name!("viewBox"));
    //   → Some(AttrValue::String("0 0 200 200"))
    SVGElementData {
        source: unsafe {
            self.unsafe_get()
                .cached_serialized_data_url
                .borrow_for_layout()  // ← THE KEY FIELD
                .clone()
        },
        // First pass:  None (cached_serialized_data_url is empty)
        // Second pass: Some(Ok(ServoUrl("data:image/svg+xml;base64,PHN2ZyB4bWxucz0i...")))
        width, height, view_box, svg_id,
    }
}
```

**Struct definition:** [shared/layout/lib.rs:152-159](../../components/shared/layout/lib.rs#L152-L159)

```rust
pub struct SVGElementData<'dom> {
    pub source: Option<Result<ServoUrl, ()>>,
    //   First pass:  None
    //   Second pass: Some(Ok(ServoUrl("data:image/svg+xml;base64,PHN2ZyB4bWxucz0i...")))
    pub width: Option<&'dom AttrValue>,
    //   Some(AttrValue::LengthPercentage(
    //     "200",
    //     Ok(LengthPercentage::Length(CSSPixelLength(200.0)))
    //   ))
    pub height: Option<&'dom AttrValue>,
    //   Some(AttrValue::LengthPercentage(
    //     "200",
    //     Ok(LengthPercentage::Length(CSSPixelLength(200.0)))
    //   ))
    pub svg_id: String,
    //   "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"
    pub view_box: Option<&'dom AttrValue>,
    //   Some(AttrValue::String("0 0 200 200"))
}
```

**THE CRITICAL FIELD:** `source`

| Value | Meaning | For Our SVG |
|-------|---------|-------------|
| `None` | Not yet serialized → **First pass** — queue for serialization | **Line 271:** `svg_data.source = None` → triggers `queue_svg_element_for_serialization()` |
| `Some(Ok(url))` | Serialized → **Second pass** — resolve from image cache | `Some(Ok(ServoUrl("data:image/svg+xml;base64,PHN2ZyB4bWxucz0i...")))` |
| `Some(Err(()))` | Previous serialization failed → skip (don't retry) | N/A for our test |

**Breakpoint:** [line 172](components/script/dom/svg/svgsvgelement.rs#L172)
**Watch:** The returned `SVGElementData` — especially `source`, `width`, `height`, `view_box`

#### Debugging this sub-stage

**✅ SVG-specific** — only called for the root `<svg>` element, on BOTH passes.

**Breakpoints:**
- [svgsvgelement.rs:170](components/script/dom/svg/svgsvgelement.rs#L170) — `SVGSVGElement::data()` entry
- [svgsvgelement.rs:172](components/script/dom/svg/svgsvgelement.rs#L172) — `borrow_for_layout()` on `cached_serialized_data_url`

**SVG identification:**
This is a method on `LayoutDom<SVGSVGElement>`, so it's ONLY called for `<svg>` root elements. If you hit this breakpoint, it IS the SVG. No filtering needed.

**How to verify uuid matches:**
The `uuid` field (line 163) should match the UUID from Stage 1 (`"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"`). Write this UUID down when you hit Stage 1 — it's the link between DOM construction and layout.

**Call frequency:**
**Multiple times** (once per layout pass, typically 6–10 total). The returned `SVGElementData` changes over time:
- **Pass 1:** `source: None` (the `cached_serialized_data_url` is still empty)
- **Passes 2+:** `source: Some(Ok(...))` (Stage 4 has filled it in, stays this way for all remaining passes)

**What to watch at this breakpoint:**

| Variable | Pass 1 | Pass 2 |
|----------|--------|--------|
| `svg_id` | `"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"` | same |
| `source` | `None` | `Some(Ok(ServoUrl("data:...")))` |
| `width` | `Some(AttrValue::LengthPercentage("200", ...))` | same |
| `height` | `Some(AttrValue::LengthPercentage("200", ...))` | same |
| `view_box` | `Some(AttrValue::String("0 0 200 200"))` | same |

---

## Sub-stage 2.6 — SVG Natural Size & Source Resolution (THE CORE)

**File:** [replaced.rs](../../components/layout/replaced.rs)
**Function:** `svg_kind_size()`
**Lines:** 221-305

### Step 1 — Parent Style Access (lines 228-243)

```rust
let parent_style = node.style(&context.style_context);
let style_builder = StyleBuilder::new(
    context.style_context.stylist.device(),
    Some(context.style_context.stylist),
    Some(&parent_style),  // inherits from parent
    None, None, false,
);
let to_computed_context = Context::new(
    style_builder, quirks_mode, rule_cache_conditions,
    ContainerSizeQuery::none(), RuleCascadeFlags::empty(),
);
```

Creates a CSS computation context **inherited from the parent element**. This enables CSS-based attribute value resolution (e.g., parsing `width="50%"` relative to parent).

**Breakpoint:** [line 228](components/layout/replaced.rs#L228)
**Watch:** `parent_style` — the parent element's ComputedValues

### Step 2 — Width/Height from SVG Attributes (lines 246-256)

```rust
let attr_to_computed = |attr_val: &AttrValue| {
    if let AttrValue::LengthPercentage(_, length_percentage) = attr_val {
        length_percentage.to_computed_value(&to_computed_context)?
            .to_length()
    } else {
        None
    }
};
let width = svg_data.width.and_then(attr_to_computed);
let height = svg_data.height.and_then(attr_to_computed);
```

Parses the `width="200"` and `height="200"` attributes. For our test case:
- Raw attribute: `AttrValue::LengthPercentage("200", Ok(LengthPercentage::Length(CSSPixelLength(200.0))))`
- `to_computed_value()` resolves → `Some(LengthPercentage::Length(Au(12000)))`
- `.to_length()` extracts → `Some(Au(12000))`

**Breakpoint:** [line 255](components/layout/replaced.rs#L255)
**Watch:** `width`, `height` — the computed pixel lengths

### Step 3 — Aspect Ratio (lines 258-263)

```rust
let ratio = match (width, height) {
    (Some(w), Some(h)) if !w.is_zero() && !h.is_zero() => Some(w.px() / h.px()),
    _ => svg_data.ratio_from_view_box(),
};
```

Computes the aspect ratio. With both width=200 and height=200: `ratio = Some(1.0)`.

If width/height are missing, falls back to parsing `viewBox` via `ratio_from_view_box()` ([shared/layout/lib.rs:162](../../components/shared/layout/lib.rs#L162)).

**Breakpoint:** [line 258](components/layout/replaced.rs#L258)
**Watch:** `ratio`

### Step 4 — Natural Size (lines 265-269)

```rust
let natural_size = NaturalSizes {
    width: width.map(|w| Au::from_f32_px(w.px())),
    height: height.map(|h| Au::from_f32_px(h.px())),
    ratio,
};
```

The "intrinsic" dimensions. For our test: `NaturalSizes { width: Some(Au(12000)), height: Some(Au(12000)), ratio: Some(1.0) }`.

> **Note:** Servo uses App Units (Au) where 1px = 60 Au. So 200px × 60 = 12000 Au.

**Breakpoint:** [line 265](components/layout/replaced.rs#L265)
**Watch:** `natural_size`

### Step 5 — THE BRANCHING POINT (lines 271-283)

```rust
let svg_source = match svg_data.source {
    None => {
        // First pass: SVG not yet serialized
        context.image_resolver
            .queue_svg_element_for_serialization(node);  // → Stage 3, [context.rs:240](components/layout/context.rs#L240)
        None
    },
    Some(svg_source_result) => svg_source_result.ok(),  // Second pass
};
```

**This is the single most important branch in the entire SVG pipeline.**

- **First pass** (`source = None`): Queues the SVG element for script-side serialization. Fires **exactly once**.
- **Passes 2+** (`source = Some(Ok(url))`): The data URL is available. But there's a SECOND implicit branch inside `get_cached_image_for_url`:
  - **Image not yet in cache** → returns `None` → `SVGElement(None)` (passes 2–3 typically)
  - **VectorImage cached** → returns `Some(VectorImage{...})` → `SVGElement(Some(...))` (passes 4+)
- **Error case** (`Some(Err(()))`): Returns `None` — serialization previously failed, don't retry.

So `svg_kind_size` can return `SVGElement(None)` many times for DIFFERENT reasons:
| Reason | `source` | Image cache state | Passes |
|--------|----------|-------------------|--------|
| Not serialized yet | `None` | N/A | Pass 1 only |
| Serialized, not loaded | `Some(Ok(url))` | URL not in cache | Passes 2–3 |
| Loaded, not rasterized | `Some(Ok(url))` | VectorImage cached, no image key | Passes 4–5 |
| Fully ready | `Some(Ok(url))` | VectorImage + image key | Pass 6+ |

**Breakpoint:** [line 271](components/layout/replaced.rs#L271) — **THE MOST IMPORTANT BREAKPOINT**
**Watch:** `svg_data.source` — determines first vs second pass

### Step 6 — Image Cache Lookup (lines 285-302, second pass only)

```rust
let cached_image = svg_source.and_then(|svg_source| {
    context.image_resolver.get_cached_image_for_url(  // [context.rs:181](components/layout/context.rs#L181)
        node.opaque(),
        svg_source,                               // "data:image/svg+xml;base64,..."
        LayoutImageDestination::BoxTreeConstruction,
    ).ok()
});

let vector_image = cached_image.map(|image| match image {
    Image::Vector(mut vector_image) => {
        vector_image.svg_id = Some(svg_data.svg_id);  // tag with the SVG UUID
        vector_image
    },
    _ => unreachable!("SVG element can't contain a raster image."),
});
```

On the second pass, the data URL is resolved through the image cache (which parsed it in Stage 5). Returns a `VectorImage` containing:

For our SVG, the data URL is:
```
data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAgMjAwIDIwMCI+PGNpcmNsZSBjeD0iMTAwIiBjeT0iMTAwIiByPSI1MCIgZmlsbD0iYmx1ZSIvPjwvc3ZnPg==
```
(This is the `<svg>` subtree serialized to XML, then base64-encoded.)

- `id: PendingImageId` — used for rasterization lookup
- `svg_id: Option<String>` — `Some("9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a")` (tagged in Step 6)
- `metadata: ImageMetadata` — `{ width: 200, height: 200 }` from the parsed `usvg::Tree::size()`
- `cors_status: CorsStatus`

**Breakpoint:** [line 285](components/layout/replaced.rs#L285)
**Watch:** `svg_source` (the data URL), `cached_image`, `vector_image`

### Step 7 — Return (line 304)

```rust
(ReplacedContentKind::SVGElement(vector_image), natural_size)
```

- **First pass:** `ReplacedContentKind::SVGElement(None)` — no vector data, will produce an empty fragment set
- **Second pass:** `ReplacedContentKind::SVGElement(Some(VectorImage{ svg_id: Some("9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"), metadata: ImageMetadata { width: 200, height: 200 }, ... }))` — fully loaded, will trigger rasterization in Stage 8

**Breakpoint:** [line 304](components/layout/replaced.rs#L304)
**Watch:** The tuple returned — especially whether `vector_image` is `None` or `Some`

#### Debugging this sub-stage

**✅ SVG-specific** — only called for the root `<svg>` element, on BOTH passes.

**All breakpoints in order (step through the entire SVG sizing logic):**

| Step | What | File:Line | Watch For |
|------|------|-----------|-----------|
| 1 | Parent style access | [replaced.rs:228](components/layout/replaced.rs#L228) | `parent_style` (the body's ComputedValues) |
| 2 | Width/height computed | [replaced.rs:255](components/layout/replaced.rs#L255) | `width = Some(Au(12000))`, `height = Some(Au(12000))` |
| 3 | Aspect ratio | [replaced.rs:258](components/layout/replaced.rs#L258) | `ratio = Some(1.0)` |
| 4 | Natural size | [replaced.rs:265](components/layout/replaced.rs#L265) | `NaturalSizes { width: Au(12000), height: Au(12000), ratio: 1.0 }` |
| **5** | **The branching point** | **[replaced.rs:271](components/layout/replaced.rs#L271)** | **`svg_data.source` = `None` (pass 1) vs `Some(Ok(...))` (pass 2)** |
| 6 | Queue serialization | [replaced.rs:277](components/layout/replaced.rs#L277) | Hit ONLY on first pass |
| 6 | Image cache lookup | [replaced.rs:286](components/layout/replaced.rs#L286) | Hit ONLY on second pass |
| 7 | Return value | [replaced.rs:304](components/layout/replaced.rs#L304) | `SVGElement(None)` (pass 1) vs `SVGElement(Some(VectorImage{...}))` (pass 2) |

**SVG identification:**
This function is ONLY called from `ReplacedContents::for_element()` when `as_svg()` returns `Some(...)`. No other element type enters this function. If you're here, it IS the SVG element.

**Call frequency:**
**Varies by environment** — see [Empirical Test Results](#empirical-test-results):
- **Async pipeline working** (user's build): 6–10 calls total
- **Async pipeline broken** (headless test): exactly 2 calls
- **Per pass breakdown:**
  - **Pass 1** (replaced.rs:271 → None arm): Queues serialization, returns `SVGElement(None)`
  - **Passes 2–3** (replaced.rs:271 → Some arm, image not cached): `get_cached_image_for_url()` returns None → `SVGElement(None)` again
  - **Passes 4+** (replaced.rs:271 → Some arm, VectorImage cached): Returns `SVGElement(Some(VectorImage{...}))`

**How to tell which pass you're in:**
At breakpoint [replaced.rs:271](components/layout/replaced.rs#L271), check BOTH `svg_data.source` AND the image cache state:

| `svg_data.source` | `cached_image` after line 293 | Meaning |
|-------------------|-------------------------------|---------|
| `None` | N/A | **Pass 1** — queue serialization |
| `Some(Ok(...))` | `None` (Err or Pending) | **Passes 2–3** — image not yet in cache |
| `Some(Ok(...))` | `Some(Image::Vector(vector_image))` | **Passes 4+** — ready for rasterization |

**Key insight: ALL passes produce the same `natural_size`**
The natural size (Au(12000) × Au(12000)) is computed from the SVG attributes (width/height) which never change. The only thing that varies is whether `vector_image` is `None` or `Some(...)`.

---

## Sub-stage 2.7 — Layout Box Construction

**File:** [formatting_contexts.rs](../../components/layout/formatting_contexts.rs)
**Function:** `IndependentFormattingContext::construct_contents()`
**Lines:** 143-182

When `Contents::Replaced(contents)` is matched:

```rust
Contents::Replaced(contents) => {
    base_fragment_info.flags.insert(FragmentFlags::IS_REPLACED);

    // Check for user-agent widgets (e.g., <video controls>)
    let widget = (node.pseudo_element_chain().is_empty() &&
        node.is_root_of_user_agent_widget()).then(|| { /* ... */ });

    return IndependentFormattingContextContents::Replaced(contents, widget);
},
```

This creates a `Replaced` variant in the formatting context, which tells the layout system: "this box has its own special layout logic — don't treat it as a regular flow."

**Breakpoint:** [line 152](components/layout/formatting_contexts.rs#L152)
**Watch:** `FragmentFlags::IS_REPLACED` being set

#### Debugging this sub-stage

**⚠ Called for ALL replaced content, on BOTH passes.** Not SVG-specific.

**Breakpoints:**
- [formatting_contexts.rs:152](components/layout/formatting_contexts.rs#L152) — the `Contents::Replaced` match arm

**SVG identification:**
Check the `contents` variable — it should contain `ReplacedContents` with `kind: SVGElement(None)` (pass 1) or `kind: SVGElement(Some(...))` (pass 2). Other replaced content types (Image, Video, IFrame) would show different `kind` values.

**What happens here:**
The `IS_REPLACED` flag is set on the fragment, and a `Replaced` variant is returned to the formatting context. This tells the layout system: "this box has special layout logic — don't treat it as a regular flow."

**Call frequency:**
Once per replaced content element, per pass. Only the `<svg>` element triggers this in our test page.

**Key variable:**
`FragmentFlags::IS_REPLACED` is inserted into `base_fragment_info.flags` — this flag affects how the fragment tree handles this node later.

---

## Sub-stage 2.8 — Layout of Replaced Content

**File:** [formatting_contexts.rs](../../components/layout/formatting_contexts.rs)
**Function:** `layout_without_caching()`
**Lines:** 391-417

During the layout phase, when the replaced content box needs its size and fragments:

```rust
IndependentFormattingContextContents::Replaced(replaced, widget) => {
    let mut replaced_layout = replaced.layout(
        layout_context,
        containing_block_for_children,
        preferred_aspect_ratio,
        &self.base,
        lazy_block_size,
    );
    // ...
    replaced_layout
}
```

This calls into `ReplacedContents::layout()` ([replaced.rs:689-720](../../components/layout/replaced.rs#L689-L720)):

1. Computes inline/block content sizes using `natural_size` and CSS `object-fit` logic
2. Calls [`self.make_fragments(layout_context, &base.style, size)`](components/layout/replaced.rs#L474-L620) → **Stage 8**

**Breakpoint:** [line 401](components/layout/formatting_contexts.rs#L401)
**Watch:** `replaced` — the ReplacedContents being laid out

#### Debugging this sub-stage

**⚠ Called for ALL replaced content, on BOTH passes.** Not SVG-specific.

**Breakpoints:**
- [formatting_contexts.rs:401](components/layout/formatting_contexts.rs#L401) — `replaced.layout()` call
- [replaced.rs:689](components/layout/replaced.rs#L689) — `ReplacedContents::layout()` entry
- [replaced.rs:474](components/layout/replaced.rs#L474) — `make_fragments()` entry (where the actual fragment is created)

**SVG identification:**
Check the `replaced` parameter's `kind` field:
- `kind: SVGElement(None)` → First pass, SVG has no data yet
- `kind: SVGElement(Some(VectorImage{...}))` → Second pass, SVG has rasterized data
- `kind: Image(...)` or other → This is some other replaced content, skip

**Stepping through `make_fragments`:**
If you step into `replaced.layout()` at [replaced.rs:689](components/layout/replaced.rs#L689), it calls `self.make_fragments()`. Inside `make_fragments`:
1. It matches on `self.kind` to find `ReplacedContentKind::SVGElement(vector_image)`
2. It creates `Fragment::Image(ArcRefCell::new(ImageFragment { image_key, ... }))`
3. This fragment is returned to the fragment tree for display list generation

**Call frequency:**
Once per replaced content element, per pass. For our SVG: **6–10 times total**.

**How `make_fragments` behavior changes across passes:**
- **Pass 1:** `SVGElement(None)` — no image key, empty fragment
- **Passes 2–3:** `SVGElement(None)` again — image still loading/caching
- **Passes 4–5:** `SVGElement(Some(VectorImage{...}))` — image data available but `image_key` may still be `None` (rasterization not complete yet). The fragment is created but without a valid GPU texture handle.
- **Passes 6+:** `SVGElement(Some(VectorImage{...}))` — `image_key` is set. The fragment now has a valid `WebRenderImageKey` for GPU rendering.

---

## Complete Data Flow Diagram

```
                         traverse_element()
                               │
                               ▼
                    element.style()  [Stylo cascade]
                               │
                               ▼
                    Contents::for_element()
                               │
                               ▼
                    ReplacedContents::for_element()
                               │
                    ┌──────────┴──────────┐
                    ▼                     ▼
           node.as_image()         node.as_svg()
           (for <img>)             (for <svg>)
                                         │
                                         ▼
                               SVGElementData {
                                   source: None / Some(Ok(data_url)),
                                   width:  AttrValue::LengthPercentage("200", ...),
                                   height: AttrValue::LengthPercentage("200", ...),
                                   viewBox: AttrValue::String("0 0 200 200"),
                                   svg_id: "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"
                               }
                                         │
                                         ▼
                               svg_kind_size()
                                         │
                    ┌────────────────────┴────────────────────┐
                    │ source: None                            │ source: Some(Ok(url))
                    ▼                                         ▼
           queue_svg_element_for    get_cached_image_for_url()
           _serialization(node)              │
           (Pass 1 only)                     ├── not in cache yet → SVGElement(None)  [Passes 2-3]
                                             │
                                             ▼
                                      VectorImage { ... }
                                      ┌─────┴─────┐
                                      ▼            ▼
                              rasterization    SVGElement(Some)
                              queued/in-progress     │
                              [Passes 4-5]           │
                                      │              ▼
                                      ▼       Stage 8 (make_fragments)
                              final layout    with image_key
                              [Pass 6+]
```

---

## Debugging Summary

### The One Critical Breakpoint

**[replaced.rs:271](components/layout/replaced.rs#L271)** — `match svg_data.source`

This is the key decision point. But it's hit **many times** — track across ALL passes:

| Pass | `svg_data.source` at Line 271 | Image Cache State | Return Value |
|------|-------------------------------|-------------------|--------------|
| 1 | `None` | N/A | `SVGElement(None)` → queue serialization |
| 2 | `Some(Ok(url))` | URL not in cache yet | `SVGElement(None)` — loading started |
| 3 | `Some(Ok(url))` | Loading/Pending | `SVGElement(None)` — still waiting |
| 4 | `Some(Ok(url))` | VectorImage cached | `SVGElement(Some(VectorImage{...}))` |
| 5 | `Some(Ok(url))` | VectorImage, no image key | `SVGElement(Some(VectorImage{...}))` |
| 6+ | `Some(Ok(url))` | Image key set | `SVGElement(Some(VectorImage{...}))` — final |

---

## Empirical Test Results

### Test Setup

- **Test file:** [svg_tests/simple_svg.html](../../svg_tests/simple_svg.html) — a minimal SVG with a single blue circle
- **Binary:** Prebuilt debug `servoshell.exe` from HEAD commit `9ba9916bad2`
- **Command:** `./target/debug/servoshell.exe -z --exit -Z relayout-event,flow-tree,display-list file.html`
- **Async test:** Also ran without `--exit` with 15-second timeout to allow async pipeline to complete

### Observed Reflow Passes

| Pass | Event | Restyle Reason | Fragment Tree | SVG Visible? |
|------|-------|---------------|---------------|--------------|
| **1** | UpdateTheRendering | `DOMChanged \| PendingRestyles` | Full tree (html→head→body→text) | **No** |
| **2** | UpdateTheRendering | `0x0` (none) | No tree output (no changes) | **No** |

**Total: 2 reflow passes.** Async passes (3+) did not appear even with the 15-second timeout.

### Key Observations

1. **2 reflow passes** maximum — no additional passes from the async SVG pipeline
2. **No SVG fragments** in the fragment tree — `make_fragments()` returns `vec![]` because `vector_image` is `None`
3. **No SVG display items** in the display list — only `HitTest` and `Text` items for the HTML page text
4. The SVG async rendering pipeline (serialization → image cache → rasterization) does NOT complete in this test environment
5. The `--exit` flag does NOT cause this — the same result occurs without `--exit` with ample timeout

### Why Only 2 Passes?

Possible explanations:
1. **Build environment difference:** The prebuilt binary may have been built from different code or with different build flags than what the user debugs
2. **Async pipeline incomplete:** The `get_cached_image_for_url()` → `get_or_request_image_or_meta()` → image cache load cycle may not trigger a third reflow for SVG data URLs in this build
3. **Listener chain broken:** The image cache listener registered in `handle_pending_images_post_reflow()` script side may not fire for SVG data URLs, preventing the third reflow

The user observed `svg_kind_size` called **~8 times** in their own debugging session, which is consistent with a fully working async pipeline (6–10 calls expected). The discrepancy suggests the async pipeline works in the user's build environment but not in the headless test environment.

### How to Verify with Tracing

Tracing code has been pre-added to the following files:

1. **[replaced.rs:226-233](../../components/layout/replaced.rs#L226)** — Counter at `svg_kind_size` entry, logs call #, source state, width, height
2. **[replaced.rs:298-303](../../components/layout/replaced.rs#L298)** — Logs whether cached image was found
3. **[replaced.rs:318-324](../../components/layout/replaced.rs#L318)** — Logs return value with vector_image presence
4. **[replaced.rs:190-198](../../components/layout/replaced.rs#L190)** — Logs when SVG element is detected in `for_element()`
5. **[dom_traversal.rs:145-150](../../components/layout/dom_traversal.rs#L145)** — Logs when `traverse_element()` visits an SVG element

**To build with tracing:**
```bash
./mach build
```

**To run with tracing:**
```bash
./target/debug/servoshell.exe svg_tests/simple_svg.html -d 2>&1 | grep "\[SVG_TRACE\]"
```

Or with detailed reflow tracking:
```bash
./target/debug/servoshell.exe -z -Z relayout-event svg_tests/simple_svg.html 2>&1 | tee svg_trace_output.txt
```

**What to look for in the output:**
- `[SVG_TRACE] traverse_element call #N SVG ELEMENT DETECTED` — confirms the SVG is being visited
- `[SVG_TRACE] for_element call #N DETECTED SVG element source=...` — confirms SVG replaced content detection
- `[SVG_TRACE] svg_kind_size call #N source=... width=... height=...` — call order and source state
- `[SVG_TRACE] svg_kind_size call #N cached_image=SOME/NONE` — whether the image cache has the data
- `[SVG_TRACE] svg_kind_size call #N RETURN vector_image=SOME/NONE` — what the function returns
- `**** Reflow(...)` lines (with `-Z relayout-event`) — each line = one reflow pass

**Expected trace output for 2-pass scenario:**
```
[SVG_TRACE] traverse_element call #N SVG ELEMENT DETECTED
[SVG_TRACE] for_element call #N DETECTED SVG element source=None
[SVG_TRACE] svg_kind_size call #0 source=None width=Some(...) height=Some(...)
[SVG_TRACE] svg_kind_size call #0 RETURN vector_image=NONE
**** Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(DOMChanged | PendingRestyles)
[SVG_TRACE] traverse_element call #N SVG ELEMENT DETECTED
[SVG_TRACE] for_element call #N DETECTED SVG element source=Some(Ok(url))
[SVG_TRACE] svg_kind_size call #1 source=Some(Ok(url)) width=Some(...) height=Some(...)
[SVG_TRACE] svg_kind_size call #1 cached_image=NONE
[SVG_TRACE] svg_kind_size call #1 RETURN vector_image=NONE
**** Reflow(Pipeline(1,1)) => UpdateTheRendering, RestyleReason(0x0)
```

**With a fully working async pipeline, you should see additional calls** (passes 3+):
```
[SVG_TRACE] svg_kind_size call #2 source=Some(Ok(url)) width=Some(...) height=Some(...)
[SVG_TRACE] svg_kind_size call #2 cached_image=SOME
[SVG_TRACE] svg_kind_size call #2 RETURN vector_image=SOME
```

The concrete data URL:
```
Some(Ok(ServoUrl("data:image/svg+xml;base64,PHN2ZyB4bWxucz0i...")))
```

### Complete Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 2.1 | Stylo style resolution | [servo_layout_element.rs:199](components/script/layout_dom/servo_layout_element.rs#L199) | `data.styles.primary()` |
| 2.2 | Layout traversal starts | [dom_traversal.rs:139](components/layout/dom_traversal.rs#L139) | `style.get_box().display` |
| 2.3 | Contents dispatch | [dom_traversal.rs:254](components/layout/dom_traversal.rs#L254) | Return value: `Replaced` |
| 2.4 | SVG detected in for_element | [replaced.rs:185-186](components/layout/replaced.rs#L185-L186) | `svg_data` struct |
| 2.5 | SVGElementData built | [svgsvgelement.rs:172](components/script/dom/svg/svgsvgelement.rs#L172) | `source`, `width`, `height` |
| 2.6-i | Parent style acquired | [replaced.rs:228](components/layout/replaced.rs#L228) | `parent_style` |
| 2.6-ii | Width/height computed | [replaced.rs:255](components/layout/replaced.rs#L255) | `width`, `height` values |
| **2.6-v** | **source match** | **[replaced.rs:271](components/layout/replaced.rs#L271)** | **`svg_data.source` — first vs second pass** |
| 2.6-vi | Queue serialization | [replaced.rs:277](components/layout/replaced.rs#L277) | Called only on first pass |
| 2.6-vi | Image cache lookup | [replaced.rs:286](components/layout/replaced.rs#L286) | Called only on second pass |
| 2.6-vii | Return value | [replaced.rs:304](components/layout/replaced.rs#L304) | `SVGElement(None)` vs `SVGElement(Some(...))` |
| 2.7 | Replaced context built | [formatting_contexts.rs:152](components/layout/formatting_contexts.rs#L152) | `IS_REPLACED` flag |
| 2.8 | Replaced layout | [formatting_contexts.rs:401](components/layout/formatting_contexts.rs#L401) | `replaced.layout()` called |

### Key Variables to Track (across all passes)

| Variable | Type | Meaning | Pass 1 | Passes 2–3 | Passes 4+ |
|----------|------|---------|--------|------------|------------|
| `svg_data.source` | `Option<Result<ServoUrl, ()>>` | Serialized data URL? | `None` | `Some(Ok(...))` | `Some(Ok(...))` |
| `svg_data.width` | `Option<&AttrValue>` | Raw width attribute | `AttrValue::LengthPercentage("200", ...)` | same | same |
| `svg_data.height` | `Option<&AttrValue>` | Raw height attribute | `AttrValue::LengthPercentage("200", ...)` | same | same |
| `svg_data.svg_id` | `String` | Unique element UUID | `"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"` | same | same |
| `svg_data.view_box` | `Option<&AttrValue>` | Raw viewBox attribute | `AttrValue::String("0 0 200 200")` | same | same |
| `width` (computed) | `Option<Au>` | Computed width | `Au(12000)` | `Au(12000)` | `Au(12000)` |
| `height` (computed) | `Option<Au>` | Computed height | `Au(12000)` | `Au(12000)` | `Au(12000)` |
| `ratio` | `Option<f32>` | Aspect ratio | `1.0` | `1.0` | `1.0` |
| `natural_size` | `NaturalSizes` | Intrinsic dimensions | `{Au(12000), Au(12000), 1.0}` | same | same |
| `svg_source` | `Option<ServoUrl>` | Resolved source | `None` | `Some(url)` | `Some(url)` |
| `cached_image` after line 293 | `Result<Image, ...>` | Image cache state | N/A | `Err` (loading) | `Ok(VectorImage{...})` |
| `vector_image` | `Option<VectorImage>` | Vector image data | `None` | `None` | `Some(VectorImage{...})` |
| `kind` | `ReplacedContentKind` | Dispatch kind | `SVGElement(None)` | `SVGElement(None)` | `SVGElement(Some(...))` |
