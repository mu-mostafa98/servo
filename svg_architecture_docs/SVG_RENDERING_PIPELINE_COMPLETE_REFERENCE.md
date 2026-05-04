# SVG Rendering Pipeline — Complete Technical Reference

> **Test case:** `<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg"><circle cx="100" cy="100" r="50" fill="blue" /></svg>`
---

## Table of Contents

1. [Pipeline Architecture Overview](#1-pipeline-architecture-overview)
2. [The Four-Pass Flow](#2-the-four-pass-flow)
3. [Stage 1 — DOM Construction](#3-stage-1--dom-construction)
4. [Stage 2 — Style Resolution & Layout Dispatch](#4-stage-2--style-resolution--layout-dispatch)
5. [Stage 3 — Queue & Serialization Dispatch](#5-stage-3--queue--serialization-dispatch)
6. [Stage 4 — SVG Subtree Serialization](#6-stage-4--svg-subtree-serialization)
7. [Stage 5 — Image Cache Load (SVG Parsing)](#7-stage-5--image-cache-load-svg-parsing)
8. [Stage 6 — Vector Image Rasterization](#8-stage-6--vector-image-rasterization)
9. [Stage 7 — WebRender Image Key Assignment](#9-stage-7--webrender-image-key-assignment)
10. [Stage 8 — Fragment Construction](#10-stage-8--fragment-construction)
11. [Stage 9 — Display List & GPU Rendering](#11-stage-9--display-list--gpu-rendering)
12. [Complete Breakpoint Reference](#12-complete-breakpoint-reference)
13. [Trace Prefix Reference](#13-trace-prefix-reference)

---

## 1. Pipeline Architecture Overview

### Thread Architecture

```
HTML Parser (Script Thread)    Layout Thread       Image Cache (Async)      WebRender (GPU)
           │                        │                      │                     │
    ┌──────▼──────┐          ┌──────▼──────┐       ┌───────▼───────┐      ┌──────▼──────┐
    │  Stage 1    │          │  Stage 2    │       │  Stage 5      │      │  Stage 9    │
    │  DOM        │          │  Style &    │       │  Parse SVG    │      │  GPU Render │
    │  Creation   │          │  Dispatch   │       │  (usvg)       │      │             │
    └─────────────┘          └──────┬──────┘       └───────┬───────┘      └─────────────┘
                                    │                      │                     ▲
                                    ▼                      │                     │
                            ┌───────▼────────┐             │                     │
                            │  Stage 3       │◄────────────┤                     │
                            │  Queue SVG     │             │                     │
                            │  Post-Reflow   │             │                     │
                            └───────┬────────┘             │                     │
                                    │                      │                     │
                                    ▼                      ▼                     │
                            ┌───────▼────────┐      ┌──────▼───────┐             │
                            │  Stage 4       │      │  Stage 6     │             │
                            │  Serialize     │      │  Rasterize   │             │
                            │  (XML→base64)  │      │  (tiny_skia) │             │
                            └────────────────┘      └──────┬───────┘             │
                                                           │                     │
                                                           ▼                     │
                                                   ┌───────▼───────┐             │
                                                   │  Stage 7      │─────────────┤
                                                   │  WR Key       │             │
                                                   └───────────────┘             │
                                                                                 │
                             ┌──────────────┐                                    │
                             │  Stage 8     │────────────────────────────────────┘
                             │  make_frags  │
                             └──────────────┘
```

### Key Threads and Their Stages

| Thread | Stages | Responsibility |
|--------|--------|----------------|
| **Script** (HTML Parser) | Stage 1, Stage 4 | DOM construction, XML serialization |
| **Script** (Layout → Script bridge) | Stage 3 (post-reflow) | Processing pending serialization queue |
| **Layout** | Stage 2, Stage 8, Stage 9 | Style resolution, fragment tree, display list |
| **Image Cache** (async thread pool) | Stage 5, Stage 6, Stage 7 | usvg parsing, tiny_skia rasterization, WR key binding |

### Observable Reflow Passes (asynchronous pipeline)

| Pass | Reflow Reason | What Happens | SVG Visible? |
|------|---------------|--------------|--------------|
| **1** | `DOMChanged \| PendingRestyles` | DOM → layout detects `source=None` → queue serialization | No |
| **2** | `PendingRestyles` | Serialize XML → base64 → data URL → dirty node | No |
| **3** | `PendingRestyles` | `source=Some(url)` but image cache not yet loaded | No |
| **4** | `PendingRestyles` | Image loaded, rasterized, WR key bound → full fragment | **Yes** |

---

## 2. The Four-Pass Flow

### Pass 1 — Reflow(DOMChanged | PendingRestyles)

```
HTML Parser                                          Layout Thread
    │                                                     │
    ▼                                                     │
create_svg_element("svg") → SVGSVGElement                 │
create_svg_element("circle") → SVGElement                  │
    │                                                     │
    │  (after style resolves)                             │
    │────────────────────────────────────────────────►     │
    │                                              │
    │                                   traverse_element(svg)
    │                                   display = Inline, is_svg = true
    │                                   │
    │                                   ▼
    │                                   Contents::for_element(svg)
    │                                   → Replaced(SVGElement)
    │                                   │
    │                                   ▼
    │                                   SVGElementData::data()
    │                                   source = None (not serialized)
    │                                   width = 200, height = 200
    │                                   │
    │                                   ▼
    │                                   svg_kind_size()
    │                                   source=None → QUEUE FOR SERIALIZATION
    │                                   │
    │                                   ▼
    │                                   queue_svg_element_for_serialization()
    │                                   │
    │                                   ▼
    │                                   make_fragments()
    │                                   vector_image=None → vec![] (empty)
    │                                              │
    │◄─────────────────────────────────────────────────│
    ▼
handle_pending_images_post_reflow()
    → serialize_and_cache_subtree()  (triggers Pass 2)
    → node.dirty(NodeDamage::Other)
```

### Pass 2 — Reflow(PendingRestyles)

```
Script Thread
    │
    ▼
serialize_and_cache_subtree()
    ├── xml_serialize() → 231 bytes of XML
    ├── base64::encode() → 334 characters
    ├── ServoUrl::parse("data:image/svg+xml;base64,...") → Ok(url)
    ├── cached_serialized_data_url = Some(Ok(url))
    └── node.dirty(NodeDamage::Other) → triggers Pass 3
```

### Pass 3 — Reflow(PendingRestyles)

```
Layout Thread
    │
    ▼
svg_kind_size()
    source = Some(Ok(data_url))    ← available now!
    get_cached_image_for_url() → "ERR/NOT_CACHED"
    → vector_image = NONE
    → make_fragments() → empty (still no image)

Background: Image cache starts loading the data URL
    → complete_load(key=1, LoadedVectorImage)
    → usvg::Tree parsed, natural dimensions = 200×200
```

### Pass 4 — Reflow(PendingRestyles)

```
Layout Thread                           Image Cache
    │                                        │
    ▼                                        │
svg_kind_size()                              │
    get_cached_image_for_url() → "OK"        │
    → GOT VectorImage { id: 1, 200×200 }     │
    → vector_image = SOME                    │
    │                                        │
    ▼                                        │
make_fragments()                             │
    vector_image.is_some = true              │
    metadata = 200×200                       │
    rasterize_vector_image(id=1, 200×200)    │
    │                                        │
    ▼                                        │
rasterize_vector_image()                     │
    → found vector_image                     │
    → cache miss → spawn thread pool task    │
    → returns None (async)                   │
    │                                        │
    │  (thread pool)                         │
    │  ─────────────                         │
    │  usvg tree → tiny_skia pixmap          │
    │  resvg::render() → 160000 bytes RGBA   │
    │  load_image_with_keycache(Svg)          │
    │                                        │
    ▼                                        │
set_key_and_finish_load()                    │
    set_webrender_image_key()                 │
    → ImageKey(IdNamespace(1), 90)           │
    complete_load_svg() → notify pipeline    │
    │                                        │
    ▼                                        │
(second layout call this pass)               │
rasterize_vector_image(id=1, 200×200)        │
    → CACHED result, returning early         │
    → image.id = Some(ImageKey(1, 90))       │
    │                                        │
    ▼                                        │
Fragment::Image { image_key: Some(ImageKey(1, 90)) }
    │
    ▼
Display List: push_image(ImageKey(1, 90), rect=200×200)
    │
    ▼
GPU: Renders blue circle
```

---

## 3. Stage 1 — DOM Construction

> **Thread:** Script (HTML Parser)
> **Key files:**
> - `components/script/dom/create.rs`
> - `components/script/dom/servoparser/mod.rs`
> - `components/script/dom/svg/svgsvgelement.rs`

### Purpose

Convert the `<svg>` HTML tag into a fully constructed `SVGSVGElement` DOM node with all attributes, inserted into the document tree.

---

### Sub-stage 1.1 — HTML Tokenization & Tree Building

**Where:** `html5ever` (integrated HTML parser library)

The raw HTML bytes `<svg width="200" height="200" ...>` are tokenized into a **start tag token**:

```rust
// Token produced by html5ever tokenizer
StartTagToken {
    name: "svg",
    ns: "http://www.w3.org/2000/svg",  // determined by HTML spec foreign content algorithm
    attrs: [
        ("width", "200"),
        ("height", "200"),
        ("viewBox", "0 0 200 200"),
        ("xmlns", "http://www.w3.org/2000/svg"),
    ],
}
```

The tree builder calls the tree sink's `create_element()` which produces a `QualName`:

```rust
QualName {
    ns: "http://www.w3.org/2000/svg",  // = ns!(svg)
    local: "svg",                        // = local_name!("svg")
    prefix: None,
}
```

**Call frequency:** Once per element during parsing. For our test: 7 elements total (html, head, meta, title, body, svg, circle). Only `svg` takes the SVG path.

---

### Sub-stage 1.2 — Element Creation Dispatch

**File:** `create_element_for_token()` in `servoparser/mod.rs` (line ~1982)

Implements the HTML spec's *create an element for the token* algorithm:

```rust
fn create_element_for_token(name: QualName, attrs, document, creator, ...) {
    let is = attrs.iter()
        .find(|attr| attr.name.local.eq_str_ignore_ascii_case("is"))
        .map(|attr| LocalName::from(&attr.value));
    // → None for <svg> (no "is" attribute)

    let definition = document.lookup_custom_element_definition(
        &name.ns, &name.local, is.as_ref()
    );
    // → None (SVG elements are not customizable)

    // Dispatch to Element::create()
    Element::create(cx, name, is, document, creator, creation_mode, None);
    // → create_element() in create.rs
}
```

**Dispatch in `create_element()` (create.rs:440):**

```rust
pub(crate) fn create_element(cx, name, is, document, creator, mode, proto) {
    match name.ns {
        ns!(html) => create_html_element(...),
        ns!(svg) => create_svg_element(cx, name, prefix, document, proto),  // ← OUR PATH
        _ => Element::new(cx, name.local, name.ns, prefix, document, proto),
    }
}
```

**Input:**
```rust
QualName { ns: "http://www.w3.org/2000/svg", local: "svg", prefix: None }
```

**Output:** Dispatches to `create_svg_element()` with the `QualName`.

**Trace:**
```
[SVG_TRACE_STAGE_1] create_svg_element() name.local=Atom('svg' type=inline) → creating SVGSVGElement
[SVG_TRACE_STAGE_1] create_svg_element() name.local=Atom('circle' type=inline) → creating SVGElement
```

---

### Sub-stage 1.3 — SVG Element Type Selection

**File:** `create_svg_element()` in `create.rs` (line 96)

```rust
fn create_svg_element(cx, name: QualName, prefix, document, proto) -> DomRoot<Element> {
    assert_eq!(name.ns, ns!(svg));

    macro_rules! make(($ctor:ident) => ({
        let obj = $ctor::new(cx, name.local, prefix, document, proto);
        DomRoot::upcast(obj)
    }));

    match name.local {
        local_name!("image") => make!(SVGImageElement),
        local_name!("svg")   => make!(SVGSVGElement),   // ← OUR CASE
        _                    => make!(SVGElement),       // circle, rect, path, etc.
    }
}
```

**Input:** `name.local` = `Atom('svg')`

**Output:**

| Tag | Constructor | Element Type |
|-----|------------|--------------|
| `<svg>` | `SVGSVGElement::new()` | `SVGSVGElement` |
| `<circle>` | `SVGElement::new()` | `SVGElement` |

---

### Sub-stage 1.4 — SVGSVGElement Construction

**File:** `SVGSVGElement::new_inherited()` in `svgsvgelement.rs` (line 49)

**Step 1 — Inheritance chain:**

```
SVGSVGElement
  └── SVGGraphicsElement::new_inherited()
        └── SVGElement::new_inherited_with_state()
              └── Element::new_inherited_with_state(local_name, ns!(svg), prefix, document)
                    └── Node::new_inherited(document)
                          └── EventTarget::new_inherited()
```

```rust
fn new_inherited(local_name, prefix, document) -> SVGSVGElement {
    SVGSVGElement {
        svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        uuid: Uuid::new_v4().to_string(),
        // Example: "90b40da2-767a-432d-b6ff-56875f1ee205"
        cached_serialized_data_url: Default::default(),  // = None
    }
}
```

**Key fields initialized per level:**

| Level | Key Fields | Example Value |
|-------|-----------|---------------|
| `EventTarget` | type_id | `NodeType(SVGSVGElement)` |
| `Node` | owner_doc, flags, layout_data | `owner_doc: &Document` |
| `Element` | local_name, namespace, attrs, state | `local_name: "svg"`, `namespace: "http://www.w3.org/2000/svg"` |
| `SVGSVGElement` | **uuid**, **cached_serialized_data_url** | `uuid: "90b40da2-767a-432d-b6ff-56875f1ee205"`, `cached: None` |

**Step 2 — DOM Reflection (`new()`, line 62):**

```rust
pub(crate) fn new(cx, local_name, prefix, document, proto) -> DomRoot<SVGSVGElement> {
    Node::reflect_node_with_proto(
        cx,
        Box::new(SVGSVGElement::new_inherited(local_name, prefix, document)),
        document,
        proto,
    )
}
```

Creates the **JavaScript/SpiderMonkey wrapper** for the SVG element via `reflect_dom_object_with_proto_and_cx()`. Every DOM object has:
1. A **Rust struct** (`SVGSVGElement`) with native data
2. A **JS wrapper object** (`JSObject`) accessible from JavaScript

JS prototype chain: `SVGSVGElement → SVGGraphicsElement → SVGElement → Element → Node → EventTarget`

---

### Sub-stage 1.5 — Attribute Initialization

**File:** `create_element_for_token()` (mod.rs, line ~2049)

After creation, all attributes from the HTML token are set:

```rust
for attr in attrs {
    element.set_attribute_from_parser(attr.name, attr.value, None, CanGc::from_cx(cx));
}
```

**For our SVG element — 4 attributes processed:**

| Attribute | Value | After `parse_plain_attribute` | AttrValue Variant |
|-----------|-------|------------------------------|-------------------|
| `width` | `"200"` | `AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0)))))` | `LengthPercentage` |
| `height` | `"200"` | `AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0)))))` | `LengthPercentage` |
| `viewBox` | `"0 0 200 200"` | `AttrValue::String("0 0 200 200")` | `String` |
| `xmlns` | `"http://www.w3.org/2000/svg"` | `AttrValue::String("http://www.w3.org/2000/svg")` | `String` |

**The `parse_plain_attribute` override in SVGSVGElement (line 221):**

```rust
fn parse_plain_attribute(&self, name, value) -> AttrValue {
    match *name {
        local_name!("width") | local_name!("height") => {
            let val = LengthPercentage::parse_quirky(
                &context, parser, AllowQuirks::Always
            );
            AttrValue::LengthPercentage(value.to_string(), val.ok())
        },
        _ => self.super_type().unwrap().parse_plain_attribute(name, value),
    }
}
```

Width/height are pre-parsed as `LengthPercentage` values — no re-parsing needed in layout. Other attributes like `viewBox` and `xmlns` use the default string storage.

---

### Sub-stage 1.6 — Tree Insertion

**File:** `insert()` in `servoparser/mod.rs` (line ~1581)

The SVG element is inserted into the DOM tree under `<body>`:

```rust
parent.InsertBefore(cx, &n, reference_child).unwrap();
```

**Post-insertion DOM tree:**

```
Document
  └── html (HTMLHtmlElement)
        └── body (HTMLBodyElement)
              └── svg (SVGSVGElement)
                    ├── width  = "200"     (AttrValue::LengthPercentage)
                    ├── height = "200"     (AttrValue::LengthPercentage)
                    ├── viewBox = "0 0 200 200" (AttrValue::String)
                    ├── xmlns  = "http://www.w3.org/2000/svg" (AttrValue::String)
                    └── circle (SVGElement)
                          ├── cx = "100"
                          ├── cy = "100"
                          ├── r  = "50"
                          └── fill = "blue"
```

Node flags set: `IS_IN_A_DOCUMENT_TREE | IS_CONNECTED`

---

### Sub-stage 1.7 — Post-Creation VirtualMethods Hooks

**File:** `svgsvgelement.rs` (line 193)

These hooks fire on dynamic DOM changes after initial creation:

| Hook | Trigger | Action |
|------|---------|--------|
| `attribute_mutated()` | Any attribute changes | Calls `invalidate_cached_serialized_subtree()` → sets `cached_serialized_data_url = None` |
| `children_changed()` | Children added/removed | Calls `invalidate_cached_serialized_subtree()` |
| `unbind_from_tree()` | Element removed from DOM | Evicts cached image from image cache, invalidates serialization |

**`invalidate_cached_serialized_subtree()` (line 164):**
```rust
fn invalidate_cached_serialized_subtree(&self) {
    *self.cached_serialized_data_url.borrow_mut() = None;
    self.upcast::<Node>().dirty(NodeDamage::Other);
}
```

> **Important:** These do NOT fire during initial parse. They only fire for dynamic DOM manipulation.

---

## 4. Stage 2 — Style Resolution & Layout Dispatch

> **Thread:** Layout
> **Also known as:** The two-pass replaced content dispatch for SVG
> **Key files:**
> - `components/layout/dom_traversal.rs`
> - `components/layout/replaced.rs`
> - `components/script/layout_dom/servo_layout_element.rs`
> - `components/script/dom/svg/svgsvgelement.rs`

### Purpose

Stage 2 is the most complex stage. It resolves CSS styles for the SVG element, identifies it as replaced content, and extracts its natural size. The critical branching happens here based on the `source` field of `SVGElementData`.

---

### Sub-stage 2.1 — CSS Cascade via Stylo

**File:** `ServoLayoutElement::style()` in `servo_layout_element.rs` (line 163)

**Purpose:** Resolve the full CSS cascade for the `<svg>` element via Stylo (Servo's CSS engine).

```rust
fn style(&self, context: &SharedStyleContext) -> ServoArc<ComputedValues> {
    let data = self.element_data();           // read ElementData
    let primary = data.styles.primary();       // get primary ComputedValues
    // ... pseudo-element handling ...
    primary
}
```

**Input:** `ServoLayoutElement` for `<svg>`

**Output:** `Arc<ComputedValues>` with resolved CSS properties:

| Property | Typical Value | Why It Matters |
|----------|---------------|----------------|
| `display` | `inline` (or `block`) | Determines if SVG goes through replaced content path |
| `width`/`height` | `auto` (or specified) | Used by CSS sizing |
| `object-fit` | `fill` | Controls SVG content scaling |
| `visibility` | `visible` | Whether element renders |
| `image-rendering` | `auto` | Affects WebRender display list |

**Trace:**
```
[SVG_TRACE_STAGE_2.1] ServoLayoutElement::style() local=Atom('svg' type=inline) is_html=false pseudo_chain=PseudoElementChain { primary: None, secondary: None }
```

**Call frequency:** Called for EVERY element in the DOM tree, depth-first. For our test: ~14 calls total across passes 1-2, only 2 of which are for the SVG element.

---

### Sub-stage 2.2 — Layout Traversal Entry

**File:** `traverse_element()` in `dom_traversal.rs` (line 139)

**Purpose:** Walk the DOM tree depth-first during layout, determining how each element should be processed.

```rust
fn traverse_element<'dom>(element, context, handler) {
    let style = element.style(&context.style_context);   // ← Sub-stage 2.1
    let info = NodeAndStyleInfo::new(element, style);
    let display_val = Display::from(info.style.get_box().display);

    match display_val {
        Display::None => {},
        Display::Contents => { /* handle display: contents */ },
        Display::GeneratingBox(display) => {
            let contents = Contents::for_element(element, context);  // ← 2.3
            let display = display.used_value_for_contents(&contents);
            handler.handle_element(&info, display, contents, box_slot); // ← 2.7
        },
    }
}
```

**For `<svg>`:**

| Variable | Value |
|----------|-------|
| `display_val` | `GeneratingBox(OutsideInside { outside: Inline, inside: Flow { is_list_item: false } })` |
| `is_svg` | `true` |
| Arm taken | `GeneratingBox` → calls `Contents::for_element()` |

**Trace:**
```
[SVG_TRACE_STAGE_2.2] traverse_element() display=GeneratingBox(OutsideInside { outside: Inline, inside: Flow }) is_svg=true
[SVG_TRACE_STAGE_2.2] SVG ELEMENT local_name=Some("Atom('svg' type=inline)") display=GeneratingBox(...)
```

---

### Sub-stage 2.3 — Contents Type Detection

**File:** `Contents::for_element()` in `dom_traversal.rs` (line 274)

**Purpose:** Determine if the element is "replaced content" (SVG, image, iframe, video, canvas) or "non-replaced" (regular elements).

```rust
pub(crate) fn for_element(node, context) -> Contents {
    if let Some(replaced) = ReplacedContents::for_element(node, context) {
        return Contents::Replaced(replaced);    // ← SVG goes here
    }
    // Otherwise: Widget or NonReplaced
}
```

**Input:** `ServoLayoutNode` for `<svg>`

**Output:** `Contents::Replaced(ReplacedContents { kind: SVGElement(None/Some(...)), ... })`

**For non-SVG elements:** Returns `Contents::NonReplaced(...)` or `Contents::Widget(...)`.

**Trace:**
```
[SVG_TRACE_STAGE_2.3] Contents::for_element() → SVG element, checking ReplacedContents
[SVG_TRACE_STAGE_2.3] Contents::for_element() → SVG element, GOT ReplacedContents
```

---

### Sub-stage 2.4 — ReplacedContent Dispatch

**File:** `ReplacedContents::for_element()` in `replaced.rs` (line 149)

**Purpose:** Main dispatch function that checks what kind of replaced content the node is.

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
            Self::svg_kind_size(svg_data, context, node)   // → 2.6
        } else if /* <audio> */ {
            // → ReplacedContentKind::Audio
        }
    };
    Some(Self { kind, natural_size, base_fragment_info: node.into() })
}
```

**The `as_svg()` call chain:**
1. Layout trait: `dom.rs:378` → `self.svg_data()`
2. Trait impl: `servo_layout_node.rs:279` → `self.node.svg_data()`
3. Node dispatch: `node.rs:2377` → `self.downcast::<SVGSVGElement>().map(|svg| svg.data())`
4. SVG data builder: `svgsvgelement.rs:172` → builds `SVGElementData`

**Input (for `<svg>`):** `ServoLayoutNode` with `type_id == SVGSVGElement`

**Output:**
```rust
Some(ReplacedContents {
    kind: SVGElement(None/Some(VectorImage{...})),
    natural_size: NaturalSizes { width, height, ratio },
    base_fragment_info: ...,
})
```

**Trace:**
```
[SVG_TRACE_STAGE_2.4] ReplacedContents::for_element() → SVG DETECTED source=None width=Some("200") height=Some("200")
```

---

### Sub-stage 2.5 — SVG Element Data Construction

**File:** `SVGSVGElement::data()` in `svgsvgelement.rs` (line 170)

**Purpose:** Build the `SVGElementData` struct that drives the serialization barrier.

```rust
pub(crate) fn data(self) -> SVGElementData<'dom> {
    let svg_id = self.unsafe_get().uuid.clone();
    // Example: "90b40da2-767a-432d-b6ff-56875f1ee205"

    let element = self.upcast::<Element>();
    let width = element.get_attr_for_layout(&ns!(), &local_name!("width"));
    // → Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0))))))

    let height = element.get_attr_for_layout(&ns!(), &local_name!("height"));
    // → Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0))))))

    let view_box = element.get_attr_for_layout(&ns!(), &local_name!("viewBox"));
    // → Some(AttrValue::String("0 0 200 200"))

    SVGElementData {
        source: unsafe {
            self.unsafe_get()
                .cached_serialized_data_url
                .borrow_for_layout()
                .clone()
        },
        // Pass 1:  None
        // Pass 2+: Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))
        width, height, view_box, svg_id,
    }
}
```

**THE CRITICAL FIELD — `source`:**

| Value | Meaning | Passes |
|-------|---------|--------|
| `None` | Not serialized yet → queue serialization | Pass 1 only |
| `Some(Ok(ServoUrl("data:...")))` | Serialized and cached | Passes 2+ |
| `Some(Err(()))` | Previous serialization failed | N/A |

**Full output struct:**

```rust
SVGElementData {
    source: None / Some(Ok(ServoUrl("data:..."))),
    width: Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0)))))),
    height: Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0)))))),
    view_box: Some(AttrValue::String("0 0 200 200")),
    svg_id: "90b40da2-767a-432d-b6ff-56875f1ee205",
}
```

**Trace:**
```
[SVG_TRACE_STAGE_2.5] SVGSVGElement::data() svg_id=90b40da2-767a-432d-b6ff-56875f1ee205 source="None" width=Some("Some") height=Some("Some") view_box=Some
```

---

### Sub-stage 2.6 — SVG Natural Size & Source Resolution (THE CORE)

**File:** `svg_kind_size()` in `replaced.rs` (line ~232)

**Purpose:** This is the single most important function in the SVG pipeline. It computes the SVG's natural dimensions and either queues serialization (first pass) or resolves the image from the cache (subsequent passes).

#### Step 1 — Parent Style Access

```rust
let parent_style = node.style(&context.style_context);
let style_builder = StyleBuilder::new(
    context.style_context.stylist.device(),
    Some(context.style_context.stylist),
    Some(&parent_style),  // inherits from parent
    None, None, false,
);
```

Creates a CSS computation context **inherited from the parent element** (body).

#### Step 2 — Width/Height from SVG Attributes

```rust
let attr_to_computed = |attr_val: &AttrValue| {
    if let AttrValue::LengthPercentage(_, length_percentage) = attr_val {
        length_percentage.to_computed_value(&to_computed_context)?
            .to_length()
    } else { None }
};
let width = svg_data.width.and_then(&attr_to_computed);
let height = svg_data.height.and_then(&attr_to_computed);
```

**Input:**
```rust
svg_data.width  = Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0))))))
svg_data.height = Some(AttrValue::LengthPercentage("200", Ok(Length(Absolute(Px(200.0))))))
```

**Output:**
```rust
width  = Some(Au(12000))   // 200px × 60 Au/px
height = Some(Au(12000))   // 200px × 60 Au/px
```

#### Step 3 — Aspect Ratio

```rust
let ratio = match (width, height) {
    (Some(w), Some(h)) if !w.is_zero() && !h.is_zero() => Some(w.px() / h.px()),
    _ => svg_data.ratio_from_view_box(),
};
```

**Output:** `ratio = Some(1.0)` (200/200)

#### Step 4 — Natural Size

```rust
let natural_size = NaturalSizes {
    width: width.map(|w| Au::from_f32_px(w.px())),    // Some(Au(12000))
    height: height.map(|h| Au::from_f32_px(h.px())),   // Some(Au(12000))
    ratio,                                               // Some(1.0)
};
```

#### Step 5 — THE BRANCHING POINT (source match)

```rust
let svg_source = match svg_data.source {
    None => {
        // Pass 1: SVG not yet serialized
        context.image_resolver
            .queue_svg_element_for_serialization(node);  // → Stage 3
        None
    },
    Some(svg_source_result) => svg_source_result.ok(),  // Passes 2+: data URL available
};
```

| `svg_data.source` | Action | Passes |
|-------------------|--------|--------|
| `None` | `queue_svg_element_for_serialization()` → Stage 3 | Pass 1 only |
| `Some(Ok(url))` | `svg_source = Some(url)` → continue to image cache lookup | Passes 2+ |
| `Some(Err(()))` | `svg_source = None` → serialization previously failed, skip | N/A |

#### Step 6 — Image Cache Lookup (Passes 2+)

```rust
let cached_image = svg_source.and_then(|svg_source| {
    context.image_resolver.get_cached_image_for_url(
        node.opaque(),
        svg_source,                               // "data:image/svg+xml;base64,..."
        LayoutImageDestination::BoxTreeConstruction,
    ).ok()
});

let vector_image = cached_image.map(|image| match image {
    Image::Vector(mut vector_image) => {
        vector_image.svg_id = Some(svg_data.svg_id);  // tag with SVG UUID
        vector_image
    },
    _ => unreachable!("SVG element can't contain a raster image."),
});
```

**Image cache states across passes:**

| Pass | `get_cached_image_for_url` result | Reason |
|------|-----------------------------------|--------|
| 2 | `ERR/NOT_CACHED` | Data URL just created, load not started |
| 3 | `ERR/NOT_CACHED` | Load in progress, not yet parsed |
| 4 | `"OK"` → `VectorImage { id: 1, metadata: 200×200 }` | usvg parsing complete |

#### Step 7 — Return

```rust
(ReplacedContentKind::SVGElement(vector_image), natural_size)
```

| Pass | `vector_image` | `kind` |
|------|---------------|--------|
| 1 | `None` | `SVGElement(None)` |
| 2-3 | `None` | `SVGElement(None)` (image not cached yet) |
| 4+ | `Some(VectorImage { id: PendingImageId(1), metadata: {200, 200}, svg_id: Some("..."), ... })` | `SVGElement(Some(...))` |

**Trace:**
```
# Pass 1 (source=None):
[SVG_TRACE_STAGE_2.6] svg_kind_size() BRANCH: source=None → QUEUING FOR SERIALIZATION
[SVG_TRACE_STAGE_2.6] svg_kind_size() RETURN vector_image=NONE natural_size=(Some(200px), Some(200px), Some(1.0))

# Pass 4 (image cached):
[SVG_TRACE_STAGE_2.6] svg_kind_size() BRANCH: source=Some(Ok/Err) → resolved_svg_source=Some(url)
[SVG_TRACE_STAGE_2.6] svg_kind_size() get_cached_image_for_url result="OK"
[SVG_TRACE_STAGE_2.6] svg_kind_size() GOT VectorImage id=PendingImageId(1) metadata=ImageMetadata { width: 200, height: 200 }
[SVG_TRACE_STAGE_2.6] svg_kind_size() RETURN vector_image=SOME natural_size=(Some(200px), Some(200px), Some(1.0))
```

---

### Sub-stage 2.7 — Layout Box Construction

**File:** `IndependentFormattingContext::construct_contents()` in `formatting_contexts.rs` (line ~143)

**Purpose:** When `Contents::Replaced(contents)` is matched, creates a `Replaced` variant in the formatting context:

```rust
Contents::Replaced(contents) => {
    base_fragment_info.flags.insert(FragmentFlags::IS_REPLACED);
    // Check for user-agent widgets
    let widget = (node.pseudo_element_chain().is_empty() &&
        node.is_root_of_user_agent_widget()).then(|| { /* ... */ });
    return IndependentFormattingContextContents::Replaced(contents, widget);
},
```

Sets `FragmentFlags::IS_REPLACED` — tells the layout system: "this box has special layout logic."

---

### Sub-stage 2.8 — Layout of Replaced Content

**File:** `layout_without_caching()` in `formatting_contexts.rs` (line ~391)

During the layout phase, the replaced content box needs its size and fragments:

```rust
IndependentFormattingContextContents::Replaced(replaced, widget) => {
    let mut replaced_layout = replaced.layout(
        layout_context, containing_block_for_children,
        preferred_aspect_ratio, &self.base, lazy_block_size,
    );
}
```

This calls `ReplacedContents::layout()` → `self.make_fragments()` → **Stage 8**.

---

## 5. Stage 3 — Queue & Serialization Dispatch

> **Thread:** Layout → Script (post-reflow bridge)
> **Key files:**
> - `components/layout/context.rs`
> - `components/script/dom/window.rs`

### Purpose

Bridge between layout and script: when layout detects `source: None`, it queues the SVG node for serialization. After the layout pass completes, the script thread processes the queue.

---

### Sub-stage 3.1 — Queue SVG Element

**File:** `ImageResolver::queue_svg_element_for_serialization()` in `context.rs` (line 240)

**Called from:** `svg_kind_size()` when `svg_data.source == None`

```rust
pub(crate) fn queue_svg_element_for_serialization(&self, element: ServoLayoutNode<'_>) {
    self.pending_svg_elements_for_serialization
        .lock()
        .push(element.opaque().into())    // → UntrustedNodeAddress
}
```

**Input:**
```rust
element.opaque() = OpaqueNode(17776695034752)
```

**Output:** `UntrustedNodeAddress` pushed to `pending_svg_elements_for_serialization` (a `Mutex<Vec<UntrustedNodeAddress>>` on `ImageResolver`).

**Trace:**
```
[SVG_TRACE_STAGE_3] queue_svg_element_for_serialization() node=OpaqueNode(17776695034752)
```

---

### Sub-stage 3.2 — Post-Reflow Image Handler

**File:** `handle_pending_images_post_reflow()` in `window.rs` (line ~3570)

**Called:** After each layout pass completes, on the script thread.

```rust
fn handle_pending_images_post_reflow(&self) {
    // Process pending SVG serializations
    for untrusted_node in self.layout().pending_svg_elements_for_serialization() {
        let node = ...;  // resolve UntrustedNodeAddress → DomRoot<Node>
        if let Some(svg) = node.downcast::<SVGSVGElement>() {
            svg.serialize_and_cache_subtree();       // → Stage 4
            node.dirty(NodeDamage::Other);           // triggers next reflow
        }
    }
    // Process pending image requests...
}
```

**Processing steps for each SVG:**
1. Resolve `UntrustedNodeAddress` → `DomRoot<Node>` via node map
2. Downcast to `SVGSVGElement`
3. Call `serialize_and_cache_subtree()` — **triggers Stage 4**
4. Call `node.dirty(NodeDamage::Other)` — marks node for re-layout

**Output after this function:**
- `cached_serialized_data_url = Some(Ok(data_url))` (set in Stage 4)
- Node is dirty → triggers next reflow
- Next layout pass: `svg_data.source = Some(Ok(url))`

**Trace:**
```
[SVG_TRACE_STAGE_3] handle_pending_images_post_reflow() processing SVG node, about to serialize
[SVG_TRACE_STAGE_3] handle_pending_images_post_reflow() SVG serialized, dirty flag set → triggers next reflow
```

---

## 6. Stage 4 — SVG Subtree Serialization

> **Thread:** Script
> **Key files:**
> - `components/script/dom/svg/svgsvgelement.rs`

### Purpose

Convert the SVG DOM subtree into a `data:` URL for processing by the standard image pipeline.

---

### Sub-stage 4.1 — Serialize & Cache Subtree

**File:** `serialize_and_cache_subtree()` in `svgsvgelement.rs` (line 79)

**Called from:** `handle_pending_images_post_reflow()` (Stage 3.2)

#### Step 1 — Process `<use>` Elements

```rust
let cloned_nodes = self.process_use_elements(cx);
```

For our simple SVG (no `<use>` elements): returns empty Vec.

#### Step 2 — XML Serialization

```rust
let serialize_result = self
    .upcast::<Node>()
    .xml_serialize(TraversalScope::IncludeNode);
```

Uses xml5ever's serializer. `TraversalScope::IncludeNode` means the serialization includes the SVG element itself as root.

**Input:** The SVG element as `&Node`

**Output:** `Ok(String)` containing XML — **231 bytes** for our test case.

**Serialized XML:**
```xml
<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <circle xmlns="http://www.w3.org/2000/svg" cx="100" cy="100" r="50" fill="blue"></circle>
</svg>
```

#### Step 3 — Base64 Encoding

```rust
let base64_encoded_source = base64::engine::general_purpose::STANDARD.encode(&xml_source);
let data_url = format!("data:image/svg+xml;base64,{}", base64_encoded_source);
```

**Output:** **334-character data URL:**
```
data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAgMjAwIDIwMCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICAgICAgICA8Y2lyY2xlIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgY3g9IjEwMCIgY3k9IjEwMCIgcj0iNTAiIGZpbGw9ImJsdWUiPjwvY2lyY2xlPgogICAgPC9zdmc+
```

#### Step 4 — Cache

```rust
match ServoUrl::parse(&data_url) {
    Ok(url) => *self.cached_serialized_data_url.borrow_mut() = Some(Ok(url)),
    Err(error) => error!("Unable to parse serialized SVG data url: {error}"),
};
```

**Output:** `cached_serialized_data_url = Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))`

#### Step 5 — Cleanup

```rust
self.cleanup_cloned_nodes(cx, &cloned_nodes);
```

Removes any `<use>`-cloned nodes (none in our case).

**Trace:**
```
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() ENTER
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() xml_source_len=231
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() data_url_len=334
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() CACHED OK url=data:image/svg+xml;base64,...
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() EXIT
```

---

### Sub-stage 4.2 — Cache Invalidation

**File:** `invalidate_cached_serialized_subtree()` in `svgsvgelement.rs` (line 164)

Called when SVG DOM subtree changes (attributes modified, children changed):

```rust
fn invalidate_cached_serialized_subtree(&self) {
    *self.cached_serialized_data_url.borrow_mut() = None;
    self.upcast::<Node>().dirty(NodeDamage::Other);
    // Also evicts from image cache if element is unbound from tree
}
```

**Triggered by:** `attribute_mutated()`, `children_changed()`, `unbind_from_tree()`.

---

## 7. Stage 5 — Image Cache Load (SVG Parsing)

> **Thread:** Image Cache (async)
> **Key files:**
> - `components/net/image_cache.rs`
> - `components/layout/context.rs`

### Purpose

Parse the SVG data URL into a `usvg::Tree` using the `resvg` library and store the resulting `VectorImage` metadata.

---

### Sub-stage 5.1 — Image Cache Request

**File:** `ImageResolver::get_or_request_image_or_meta()` in `context.rs` (line 127)

**Called from:** `get_cached_image_for_url()` in `svg_kind_size()`

```rust
pub(crate) fn get_or_request_image_or_meta(&self, node, url, destination) {
    let cache_result = self.image_cache
        .get_cached_image_status(url.clone(), self.origin.clone(), None);

    match cache_result {
        ImageCacheResult::Available(img_or_meta) => { /* return data */ },
        ImageCacheResult::Pending(id) => { /* add to pending list */ },
        ImageCacheResult::ReadyForRequest(id) => {
            // First time seeing this URL — queue for loading
            self.pending_images.lock().push(PendingImage {
                state: PendingImageState::Unrequested(url),
                node: node.into(), id, origin: self.origin.clone(), destination,
            });
            LayoutImageCacheResult::Pending
        },
        ImageCacheResult::FailedToLoadOrDecode => LayoutImageCacheResult::LoadError,
    }
}
```

**For a new SVG data URL (Pass 3):** Returns `ReadyForRequest(id=1)` → creates a `PendingImage` with `state: Unrequested`.

---

### Sub-stage 5.2 — Complete Load (VectorImage)

**File:** `complete_load()` in `image_cache.rs` (line 597)

Called when the SVG data URL has been loaded and parsed by usvg:

```rust
fn complete_load(&mut self, key: LoadKey, load_result: LoadResult) {
    let pending_load = match self.pending_loads.remove(&key) {
        Some(load) => load, None => return,
    };

    let image_response = match load_result {
        LoadResult::LoadedVectorImage(vector_image) => {
            self.vector_images.insert(key, vector_image.clone());
            // Store usvg::Tree keyed by PendingImageId

            let natural_dimensions = vector_image.svg_tree.size().to_int_size();
            // natural_dimensions = (200, 200) for our test

            let metadata = ImageMetadata {
                width: natural_dimensions.width(),    // 200
                height: natural_dimensions.height(),  // 200
            };

            let vector_image = VectorImage {
                id: key,                 // PendingImageId(1)
                svg_id: None,            // tagged later in svg_kind_size()
                metadata,
                cors_status: vector_image.cors_status,
            };
            ImageResponse::Loaded(Image::Vector(vector_image), url.unwrap())
        },
        LoadResult::FailedToLoadOrDecode => ImageResponse::FailedToLoadOrDecode,
        _ => unreachable!(),
    };
    // Store in completed_loads, notify listeners
}
```

**Input:**
```rust
key = PendingImageId(1)
load_result = LoadResult::LoadedVectorImage(VectorImage {
    svg_tree: usvg::Tree { size: Size(200.0, 200.0), ... },
    cors_status: CorsStatus::Uncached,
})
```

**Output:**
```rust
VectorImage {
    id: PendingImageId(1),
    svg_id: None,                                          // tagged later
    metadata: ImageMetadata { width: 200, height: 200 },    // from usvg::Tree::size()
    cors_status: CorsStatus::Uncached,
}
```

The `usvg::Tree` is stored in `self.vector_images[PendingImageId(1)]`.

**Trace:**
```
[SVG_TRACE_STAGE_5] complete_load() ENTER key=PendingImageId(1) is_vector=true
[SVG_TRACE_STAGE_5] complete_load() VectorImage detected, inserting into vector_images
[SVG_TRACE_STAGE_5] complete_load() VectorImage natural_dimensions=200x200
```

---

### Sub-stage 5.3 — Complete Load SVG (Rasterization Done Notification)

**File:** `complete_load_svg()` in `image_cache.rs` (line 569)

Called after Stage 7 assigns a WebRender key to the rasterized SVG:

```rust
fn complete_load_svg(&mut self, rasterized_image: RasterImage,
                      pending_image_id: PendingImageId,
                      requested_size: DeviceIntSize) {
    let listeners = self.rasterized_vector_images
        .get_mut(&(pending_image_id, requested_size))
        .map(|task| {
            task.result = Some(rasterized_image);
            std::mem::take(&mut task.listeners)
        })
        .unwrap_or_default();

    for (pipeline_id, callback) in listeners {
        callback(ImageCacheResponseMessage::VectorImageRasterizationComplete(
            RasterizationCompleteResponse {
                pipeline_id,                // (1, 1)
                image_id: pending_image_id,  // PendingImageId(1)
                requested_size,              // 200×200
            },
        ));
    }
}
```

**Input:**
```rust
rasterized_image = RasterImage { metadata: 200×200, bytes: 160000, id: Some(ImageKey(1,90)), ... }
pending_image_id = PendingImageId(1)
requested_size = DeviceIntSize(200, 200)
```

**Output:** Pipeline `(1,1)` notified → triggers reflow where `rasterize_vector_image()` returns cached result.

**Trace:**
```
[SVG_TRACE_STAGE_5] complete_load_svg() ENTER pending_image_id=PendingImageId(1) requested_size=200x200 rasterized_size=200x200
[SVG_TRACE_STAGE_5] complete_load_svg() found 1 listener(s)
[SVG_TRACE_STAGE_5] complete_load_svg() notifying pipeline_id=(1,1)
```

---

## 8. Stage 6 — Vector Image Rasterization

> **Thread:** Image Cache (thread pool — async)
> **Key files:**
> - `components/net/image_cache.rs`
> - `components/layout/context.rs`

### Purpose

Convert the `usvg::Tree` into a rasterized RGBA pixel buffer using `tiny_skia` / `resvg`. Runs asynchronously on a thread pool and caches the result.

---

### Sub-stage 6.1 — Rasterization Request (Layout Side)

**File:** `ImageResolver::rasterize_vector_image()` in `context.rs` (line 218)

**Called from:** `make_fragments()` in Stage 8

```rust
pub(crate) fn rasterize_vector_image(
    &self, image_id: PendingImageId, size: DeviceIntSize,
    node: OpaqueNode, svg_id: Option<String>,
) -> Option<RasterImage> {
    let result = self.image_cache.rasterize_vector_image(image_id, size, svg_id);
    if result.is_none() {
        // Async: track pending rasterization for retry
        self.pending_rasterization_images.lock().push(
            PendingRasterizationImage { id: image_id, node: node.into(), size }
        );
    }
    result
}
```

**Input:** `image_id = PendingImageId(1)`, `size = 200×200`, `svg_id = Some("9435b93e-...")`

**Output:** `None` on first call (async), `Some(RasterImage)` on subsequent calls (cached).

---

### Sub-stage 6.2 — Image Cache Rasterization

**File:** `rasterize_vector_image()` in `image_cache.rs` (line 967)

#### Step 1 — Look up VectorImage

```rust
let Some(vector_image) = store.vector_images.get(&image_id).cloned() else {
    return None;  // unknown image id
};
```

Looks up the `usvg::Tree` stored in Stage 5 by `PendingImageId(1)`.

#### Step 2 — Check Cache

```rust
let entry = store.rasterized_vector_images
    .entry((image_id, requested_size))
    .or_default();
if let Some(result) = entry.result.as_ref() {
    return Some(result.clone());   // ← cache hit
}
```

#### Step 3 — Update ID Maps

```rust
if let Some(svg_id) = svg_id {
    self.svg_id_image_id_map.lock().insert(svg_id, image_id);
    // Maps SVG UUID → PendingImageId for cache management
}
self.image_id_size_map.lock().insert(image_id, vec![requested_size]);
```

#### Step 4 — Spawn Thread Pool Task

```rust
self.thread_pool.spawn(move || {
    let natural_size = vector_image.svg_tree.size().to_int_size();
    // natural_size = (200, 200)

    let tinyskia_requested_size = {
        let width = requested_size.width.try_into().unwrap_or(0)
            .min(MAX_SVG_PIXMAP_DIMENSION);  // clamp to 4096 max
        let height = requested_size.height.try_into().unwrap_or(0)
            .min(MAX_SVG_PIXMAP_DIMENSION);
        tiny_skia::IntSize::from_wh(width, height).unwrap_or(natural_size)
    };
    // tinyskia_requested_size = (200, 200) — no clamping needed

    let transform = tiny_skia::Transform::from_scale(
        tinyskia_requested_size.width() as f32 / natural_size.width() as f32,
        // = 200.0 / 200.0 = 1.0
        tinyskia_requested_size.height() as f32 / natural_size.height() as f32,
        // = 200.0 / 200.0 = 1.0
    );

    let mut pixmap = tiny_skia::Pixmap::new(200, 200).unwrap();
    resvg::render(&vector_image.svg_tree, transform, &mut pixmap.as_mut());
    // ← ACTUAL SVG RENDERING HAPPENS HERE

    let bytes = pixmap.take();
    // 160000 bytes = 200 × 200 × 4 (RGBA)

    let rasterized_image = RasterImage {
        metadata: ImageMetadata { width: 200, height: 200 },
        format: PixelFormat::RGBA8,
        frames: vec![ImageFrame { delay: None, byte_range: 0..160000, width: 200, height: 200 }],
        bytes: Arc::new(bytes),  // 160000 bytes RGBA
        id: None,                // set when WR key is assigned
        cors_status: vector_image.cors_status,
        is_opaque: false,
    };

    store.lock().load_image_with_keycache(PendingKey::Svg((
        image_id, rasterized_image, requested_size,
    )));
    // → triggers Stage 7
});
```

**Rasterization parameters:**

| Parameter | Value |
|-----------|-------|
| `natural_size` | `200 × 200` |
| `tinyskia_requested_size` | `200 × 200` |
| `transform` | `scale(1.0, 1.0)` |
| `pixmap` size | `200 × 200` |
| `bytes` length | `160000` (200 × 200 × 4) |
| `MAX_SVG_PIXMAP_DIMENSION` | `4096` (safety clamp) |

**Trace:**
```
[SVG_TRACE_STAGE_6] rasterize_vector_image() ENTER image_id=PendingImageId(1) requested_size=200x200 svg_id=Some("...")
[SVG_TRACE_STAGE_6] rasterize_vector_image() found vector_image, usvg tree size=200.0x200.0
[SVG_TRACE_STAGE_6] rasterize_vector_image() spawning thread pool task...
[SVG_TRACE_STAGE_6] rasterize_vector_image() returning None (async rasterization)
[SVG_TRACE_STAGE_6] rasterize_vector_image() rasterized 200x200 -> 160000 bytes
[SVG_TRACE_STAGE_6] rasterize_vector_image() CACHED result, returning early (subsequent calls)
```

---

## 9. Stage 7 — WebRender Image Key Assignment

> **Thread:** Image Cache
> **Key files:**
> - `components/net/image_cache.rs`

### Purpose

Assign a WebRender `ImageKey` to the rasterized SVG pixel buffer, making it available as a GPU texture.

---

### Sub-stage 7.1 — Set Key and Finish Load

**File:** `set_key_and_finish_load()` in `image_cache.rs` (line 484)

**Called from:** `load_image_with_keycache()` at the end of Stage 6

```rust
fn set_key_and_finish_load(&mut self, pending_image: PendingKey, image_key: WebRenderImageKey) {
    match pending_image {
        PendingKey::Svg((pending_id, mut raster_image, requested_size)) => {
            set_webrender_image_key(&self.paint_api, &mut raster_image, image_key);
            // After: raster_image.id = Some(ImageKey(IdNamespace(1), 90))

            self.complete_load_svg(raster_image, pending_id, requested_size);
            // → notifies pipeline (1,1)
        },
        PendingKey::RasterImage(_) => { /* raster image path */ },
    }
}
```

**Input:**
```rust
pending_image = PendingKey::Svg((
    PendingImageId(1),
    RasterImage { metadata: 200×200, bytes: 160000, id: None, ... },
    DeviceIntSize(200, 200),
))
image_key = ImageKey(IdNamespace(1), 90)   // WR texture handle
```

**Step 1 — Bind WebRender Texture:**
```rust
set_webrender_image_key(&self.paint_api, &mut raster_image, image_key);
```
Uploads pixel data to WebRender GPU texture cache. After: `raster_image.id = Some(ImageKey(IdNamespace(1), 90))`.

**Step 2 — Notify Listeners:**
```rust
self.complete_load_svg(raster_image, pending_id, requested_size);
```
Notifies pipeline `(1,1)` → triggers reflow for final rendering.

**The `ImageKey` struct:**
```rust
ImageKey(IdNamespace(1), 90)
//   └── namespace      └── unique texture ID
```

**Trace:**
```
[SVG_TRACE_STAGE_7] set_key_and_finish_load() image_key=ImageKey(IdNamespace(1), 90) variant="Svg"
[SVG_TRACE_STAGE_7] set_key_and_finish_load() SVG variant, pending_id=PendingImageId(1) requested_size=200x200
```

---

## 10. Stage 8 — Fragment Construction

> **Thread:** Layout
> **Key files:**
> - `components/layout/replaced.rs`

### Purpose

Convert `ReplacedContents` (with `SVGElement(None/Some)`) into `Fragment::Image` entries for the fragment tree.

---

### Sub-stage 8.1 — Fragment Entry

**File:** `make_fragments()` in `replaced.rs` (line 524)

**Called from:** `ReplacedContents::layout()` during the layout phase

```rust
pub fn make_fragments(&self, layout_context: &LayoutContext,
                       style: &ServoArc<ComputedValues>,
                       size: PhysicalSize<Au>) -> Vec<Fragment> {
    let (object_fit_size, rect) = self.calculate_fragment_rect(style, size);
    let clip = PhysicalRect::new(PhysicalPoint::origin(), size);
    let mut base = BaseFragment::new(self.base_fragment_info, style.clone().into(), rect);

    match &self.kind {
        ReplacedContentKind::Image(image_info) => { /* <img> path */ },
        ReplacedContentKind::SVGElement(vector_image) => { /* ← OUR PATH */ },
        ReplacedContentKind::Video(_) => { /* <video> path */ },
        ReplacedContentKind::IFrame(_) => { /* <iframe> path */ },
        ReplacedContentKind::Canvas(_) => { /* <canvas> path */ },
        ReplacedContentKind::Audio => vec![],
    }
}
```

**Input:** `size = 200px × 200px`, `kind = SVGElement(None/Some(...))`

---

### Sub-stage 8.2 — SVGElement Arm (Empty / vector_image = None)

**Lines 616-618:**

```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else {
        return vec![];    // ← Passes 1-3 take this path
    };
    // ... remainder for Pass 4+ ...
}
```

**Input:** `vector_image = None`

**Output:** `vec![]` — empty fragment. SVG takes up no visual space.

**Trace (Passes 1-3):**
```
[SVG_TRACE_STAGE_8] make_fragments() ENTER kind=Discriminant(4) size=200pxx200px
[SVG_TRACE_STAGE_8] make_fragments() SVGElement arm, vector_image.is_some=false
```

---

### Sub-stage 8.3 — SVGElement Arm (With VectorImage / vector_image = Some)

**Lines 616-662 (continued):**

```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else { return vec![]; };

    // TODO: This is incorrect if the SVG has a viewBox.
    base.rect = PhysicalSize::new(
        vector_image.metadata.width.try_into()
            .map_or(MAX_AU, Au::from_px),     // 200px
        vector_image.metadata.height.try_into()
            .map_or(MAX_AU, Au::from_px),     // 200px
    ).into();

    let scale = layout_context.style_context.device_pixel_ratio();
    let raster_size = Size2D::new(
        base.rect.size.width.scale_by(scale.0).to_px(),   // 200
        base.rect.size.height.scale_by(scale.0).to_px(),  // 200
    );

    let tag = self.base_fragment_info.tag.unwrap();
    layout_context
        .image_resolver
        .rasterize_vector_image(
            vector_image.id,          // PendingImageId(1)
            raster_size,              // 200 × 200
            tag.node,                 // OpaqueNode
            vector_image.svg_id.clone(), // "9435b93e-..."
        )
        .and_then(|image| image.id)   // Some(ImageKey(1, 90)) after Stage 7
        .map(|image_key| {
            Fragment::Image(ArcRefCell::new(ImageFragment {
                base,
                clip,
                image_key: Some(image_key),    // Some(ImageKey(1, 90))
                showing_broken_image_icon: false,
                url: None,
            }))
        })
        .into_iter()
        .collect()
}
```

**Input (Pass 4+):**
```rust
vector_image = Some(VectorImage {
    id: PendingImageId(1),
    metadata: ImageMetadata { width: 200, height: 200 },
    svg_id: Some("9435b93e-ea8a-4323-996d-b048ab24a3ab"),
    ...
})
```

**Fragment rect computation:**
```rust
base.rect = PhysicalSize::new(
    200.try_into().map_or(MAX_AU, Au::from_px),   // Au(12000)
    200.try_into().map_or(MAX_AU, Au::from_px),   // Au(12000)
).into();
```

**Output (when image key is available):**
```rust
vec![Fragment::Image(ArcRefCell::new(ImageFragment {
    base: BaseFragment { rect: 200px × 200px at (0px, 0px), ... },
    clip: clip,
    image_key: Some(ImageKey(IdNamespace(1), 90)),
    showing_broken_image_icon: false,
    url: None,
}))]
```

**Note on first vs second call in Pass 4:**
- **First call:** `rasterize_vector_image()` returns `None` (async) → `.and_then(...)` returns `None` → `.map(...)` returns `None` → `vec![]`
- **Second call** (same pass, after Stage 6-7 complete): `rasterize_vector_image()` returns cached `Some(RasterImage)` with `id = Some(ImageKey(1,90))` → fragment built successfully

**Trace (Pass 4+):**
```
[SVG_TRACE_STAGE_8] make_fragments() ENTER kind=Discriminant(4) size=200pxx200px
[SVG_TRACE_STAGE_8] make_fragments() SVGElement arm, vector_image.is_some=true
[SVG_TRACE_STAGE_8] make_fragments() SVGElement metadata=200x200
```

---

## 11. Stage 9 — Display List & GPU Rendering

> **Thread:** Layout → WebRender
> **Key files:**
> - `components/layout/display_list/mod.rs`

### Purpose

Convert `Fragment::Image` into a WebRender display list item (`push_image`) for GPU rendering.

---

### Sub-stage 9.1 — Display List Building

**File:** `Fragment::Image` handler in `display_list/mod.rs` (line 680)

```rust
Fragment::Image(image) => {
    let image = image.borrow();
    let style = image.base.style();
    match style.get_inherited_box().visibility {
        Visibility::Visible => {
            let image_rendering = style.get_inherited_box()
                .image_rendering.to_webrender();
                // → wr::ImageRendering::Auto

            let rect = image.base.rect
                .translate(containing_block.origin.to_vector())
                .to_webrender();
                // → wr::LayoutRect(200×200 at position)

            let clip = image.clip
                .translate(containing_block.origin.to_vector())
                .to_webrender();

            let common = builder.common_properties(clip, &style);

            if let Some(image_key) = image.image_key {
                // → Some(ImageKey(IdNamespace(1), 90))
                builder.wr().push_image(
                    &common,
                    rect,
                    image_rendering,                              // Auto
                    wr::AlphaType::PremultipliedAlpha,            // standard alpha
                    ImageKey(IdNamespace(1), 90),                 // ← the SVG texture!
                    wr::ColorF::WHITE,                            // default color
                );
            }
        },
        Visibility::Hidden | Visibility::Collapse => (),  // skip
    }
}
```

**Input:**
```rust
Fragment::Image {
    base: BaseFragment { rect: 200px × 200px at (0px, 0px), ... },
    clip: PhysicalRect(200px × 200px at (0px, 0px)),
    image_key: Some(ImageKey(IdNamespace(1), 90)),
    showing_broken_image_icon: false,
    url: None,
}
```

**The `push_image` call** is the final output of the entire SVG pipeline — it tells WebRender to draw the SVG texture at the specified position.

---

### Sub-stage 9.2 — Background Image Path

**File:** `display_list/mod.rs` (line 1509)

When SVG appears as a CSS `background-image`:

```rust
Ok(ResolvedImage::Image { image, size }) => {
    let dppx = 1.0;
    let intrinsic = NaturalSizes::from_width_and_height(
        size.width / dppx, size.height / dppx
    );
    let layer = background::layout_layer(self, painter, builder, index, intrinsic);

    let image_wr_key = match image {
        CachedImage::Raster(raster_image) => raster_image.id,
        CachedImage::Vector(vector_image) => {
            builder.image_resolver.rasterize_vector_image(
                vector_image.id,
                default_size,
                node,
                vector_image.svg_id.clone(),
            ).and_then(|r| r.id)
        },
    };
}
```

**Trace:**
```
[SVG_TRACE_STAGE_9] DisplayList background SVG image size=200x200
```

---

### Sub-stage 9.3 — Border Image Path

**File:** `display_list/mod.rs` (line 1758)

When SVG appears in a CSS `border-image`:

```rust
Ok(ResolvedImage::Image { image, size }) => {
    let image_key = match image {
        CachedImage::Raster(raster_image) => raster_image.id,
        CachedImage::Vector(vector_image) => {
            builder.image_resolver.rasterize_vector_image(
                vector_image.id, size, node, vector_image.svg_id,
            ).and_then(|r| r.id)
        },
    };
}
```

**Trace:**
```
[SVG_TRACE_STAGE_9] DisplayList border-image SVG image size=200x200
```

---

### GPU Rendering (WebRender)

After the display list is built:

1. **Batching** — `push_image` is batched with similar commands
2. **Texture upload** — Pixel data for `ImageKey(IdNamespace(1), 90)` uploaded to GPU (if not already)
3. **Shader execution** — GPU shader renders the 200×200 RGBA pixels at the correct screen position
4. **Composition** — Frame composited to screen

**Final result:** A blue circle is visible on the page.

**Trace:**
```
[SVG_TRACE_STAGE_9] DisplayList build Fragment::Image rect=Rect(200pxx200px at (0px, 0px)) image_key=Some(ImageKey(IdNamespace(1), 90))
```

---

## 12. Complete Breakpoint Reference

### All Stages

| Stage | File | Line | Description |
|-------|------|------|-------------|
| **1.2** | `create.rs` | 440 | Namespace dispatch — `name.ns` matches `ns!(svg)` |
| **1.3** | `create.rs` | 114 | SVG type selection — `name.local` = "svg" |
| **1.4-i** | `svgsvgelement.rs` | 50 | `new_inherited()` — uuid created |
| **1.4-ii** | `svgsvgelement.rs` | 70 | `new()` — JS reflection |
| **1.5** | `svgsvgelement.rs` | 221 | `parse_plain_attribute()` — width/height parsing |
| **2.1** | `servo_layout_element.rs` | 163 | `style()` — CSS cascade |
| **2.2** | `dom_traversal.rs` | 139 | `traverse_element()` — layout traversal entry |
| **2.3** | `dom_traversal.rs` | 274 | `Contents::for_element()` — replaced content detection |
| **2.4** | `replaced.rs` | 149 | `ReplacedContents::for_element()` — SVG dispatch |
| **2.5** | `svgsvgelement.rs` | 170 | `SVGSVGElement::data()` — SVGElementData built |
| **2.6-i** | `replaced.rs` | 228 | `svg_kind_size()` — parent style acquired |
| **2.6-ii** | `replaced.rs` | 255 | Width/height computed |
| **2.6-iii** | `replaced.rs` | 258 | Aspect ratio |
| **2.6-iv** | `replaced.rs` | 265 | Natural size |
| **2.6-v** | `replaced.rs` | 271 | **THE BRANCHING POINT** — `source` match |
| **2.6-vi** | `replaced.rs` | 277 | Queue serialization (Pass 1 only) |
| **2.6-vi** | `replaced.rs` | 286 | Image cache lookup (Passes 2+) |
| **2.6-vii** | `replaced.rs` | 304 | Return value |
| **2.7** | `formatting_contexts.rs` | 152 | `IS_REPLACED` flag set |
| **2.8** | `formatting_contexts.rs` | 401 | `replaced.layout()` — fragment construction |
| **3.1** | `context.rs` | 240 | `queue_svg_element_for_serialization()` |
| **3.2** | `window.rs` | ~3570 | `handle_pending_images_post_reflow()` |
| **4.1-i** | `svgsvgelement.rs` | 79 | `serialize_and_cache_subtree()` entry |
| **4.1-ii** | `svgsvgelement.rs` | 85 | XML serialization |
| **4.1-iii** | `svgsvgelement.rs` | 99 | Data URL caching |
| **5.1** | `context.rs` | 127 | `get_or_request_image_or_meta()` |
| **5.2** | `image_cache.rs` | 597 | `complete_load()` — usvg parsed |
| **5.3** | `image_cache.rs` | 569 | `complete_load_svg()` — pipeline notified |
| **6.1** | `context.rs` | 218 | `ImageResolver::rasterize_vector_image()` |
| **6.2-i** | `image_cache.rs` | 967 | `rasterize_vector_image()` entry |
| **6.2-ii** | `image_cache.rs` | 986 | Cache hit/miss check |
| **6.2-iii** | `image_cache.rs` | 1035 | Thread pool spawn |
| **6.2-iv** | `image_cache.rs` | 1059 | `resvg::render()` call |
| **7.1** | `image_cache.rs` | 484 | `set_key_and_finish_load()` — WR key assigned |
| **8.1** | `replaced.rs` | 524 | `make_fragments()` entry |
| **8.2** | `replaced.rs` | 617 | SVGElement None arm |
| **8.3** | `replaced.rs` | 643 | Rasterization call from make_fragments |
| **9.1** | `display_list/mod.rs` | 680 | `Fragment::Image` handler |
| **9.1** | `display_list/mod.rs` | 699 | `push_image()` call |
| **9.2** | `display_list/mod.rs` | 1509 | Background SVG path |
| **9.3** | `display_list/mod.rs` | 1758 | Border SVG path |

---

## 13. Trace Prefix Reference

All tracing uses the format `[SVG_TRACE_STAGE_N]` and can be filtered with:

```powershell
./target/debug/servoshell.exe --exit "file:///..." -Z relayout-event 2>&1 | Select-String "\[SVG_TRACE"
```

| Prefix | File | Function | What It Logs |
|--------|------|----------|-------------|
| `[SVG_TRACE_STAGE_1]` | `create.rs` | `create_svg_element()` | Element name and type created |
| `[SVG_TRACE_STAGE_2.1]` | `servo_layout_element.rs` | `style()` | local_name, is_html, pseudo_chain |
| `[SVG_TRACE_STAGE_2.2]` | `dom_traversal.rs` | `traverse_element()` | display value, SVG detection, local_name |
| `[SVG_TRACE_STAGE_2.3]` | `dom_traversal.rs` | `Contents::for_element()` | SVG detection, ReplacedContents result |
| `[SVG_TRACE_STAGE_2.4]` | `replaced.rs` | `ReplacedContents::for_element()` | SVG DETECTED with source/width/height |
| `[SVG_TRACE_STAGE_2.5]` | `svgsvgelement.rs` | `SVGSVGElement::data()` | svg_id, source state, width/height/viewBox |
| `[SVG_TRACE_STAGE_2.6]` | `replaced.rs` | `svg_kind_size()` | Full trace of w/h, source branch, cache result, return value |
| `[SVG_TRACE_STAGE_3]` | `context.rs` | `queue_svg_element_for_serialization()` | Node opaque address |
| `[SVG_TRACE_STAGE_3]` | `window.rs` | `handle_pending_images_post_reflow()` | Processing SVG, dirty flag |
| `[SVG_TRACE_STAGE_4]` | `svgsvgelement.rs` | `serialize_and_cache_subtree()` | XML length, data URL length, cache result |
| `[SVG_TRACE_STAGE_5]` | `image_cache.rs` | `complete_load()` | LoadKey, VectorImage detection, natural dimensions |
| `[SVG_TRACE_STAGE_5]` | `image_cache.rs` | `complete_load_svg()` | pending_image_id, requested_size, listener count |
| `[SVG_TRACE_STAGE_6]` | `image_cache.rs` | `rasterize_vector_image()` | image_id, requested_size, cache hit/miss, rasterization result |
| `[SVG_TRACE_STAGE_7]` | `image_cache.rs` | `set_key_and_finish_load()` | ImageKey, SVG variant with pending_id |
| `[SVG_TRACE_STAGE_8]` | `replaced.rs` | `make_fragments()` | kind discriminant, SVGElement metadata, vector_image presence |
| `[SVG_TRACE_STAGE_9]` | `display_list/mod.rs` | Fragment::Image | Rect, image_key |

### How to filter specific stages:

```powershell
# Single stage
... | Select-String "\[SVG_TRACE_STAGE_2\.6\]"

# Multiple stages
... | Select-String "\[SVG_TRACE_STAGE_[46]\]"

# Range of stages
... | Select-String "\[SVG_TRACE_STAGE_[2-4]\]"

# With reflow events
... | Select-String "\[SVG_TRACE|Reflow"
```

---

## Appendix: Key Data Structures

### `SVGElementData` (shared/layout/lib.rs)
```rust
pub struct SVGElementData<'dom> {
    pub source: Option<Result<ServoUrl, ()>>,     // None → queue serialization
    pub width: Option<&'dom AttrValue>,            // Some(LengthPercentage("200", ...))
    pub height: Option<&'dom AttrValue>,           // Some(LengthPercentage("200", ...))
    pub svg_id: String,                            // "90b40da2-..."
    pub view_box: Option<&'dom AttrValue>,         // Some(String("0 0 200 200"))
}
```

### `VectorImage` (net_traits/image_cache.rs)
```rust
pub struct VectorImage {
    pub id: PendingImageId,                // PendingImageId(1)
    pub svg_id: Option<String>,            // Some("uuid") or None
    pub metadata: ImageMetadata,           // { width: 200, height: 200 }
    pub cors_status: CorsStatus,
}
```

### `RasterImage` (pixels/lib.rs)
```rust
pub struct RasterImage {
    pub metadata: ImageMetadata,           // { width: 200, height: 200 }
    pub format: PixelFormat,               // RGBA8
    pub frames: Vec<ImageFrame>,           // single frame, 160000 bytes
    pub bytes: Arc<[u8]>,                  // 200 × 200 × 4 = 160000 bytes
    pub id: Option<WebRenderImageKey>,     // Some(ImageKey(1, 90)) after Stage 7
    pub is_opaque: bool,                   // false
}
```

### `ImageFragment` (layout/display_list)
```rust
pub struct ImageFragment {
    pub base: BaseFragment,                // rect, style, flags
    pub clip: PhysicalRect<Au>,            // clipping region
    pub image_key: Option<WebRenderImageKey>,  // Some(ImageKey(1, 90))
    pub showing_broken_image_icon: bool,   // false
    pub url: Option<ServoUrl>,             // None (inline SVG)
}
```

### `NaturalSizes`
```rust
pub struct NaturalSizes {
    pub width: Option<Au>,     // Some(Au(12000)) = 200px
    pub height: Option<Au>,    // Some(Au(12000)) = 200px
    pub ratio: Option<f32>,    // Some(1.0)
}
```

---

> **End of document.** This reference covers all 9 stages of the SVG rendering pipeline for the Servo browser engine, validated against empirical trace data from 4 observed reflow passes with a working async image pipeline.
