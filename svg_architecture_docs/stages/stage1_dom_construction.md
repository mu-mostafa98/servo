# Stage 1 — DOM Construction

> **Thread:** HTML Parser → Script (DOM)
> **Also known as:** Element creation from HTML token
> **Key files:**
> - [components/script/dom/create.rs](../../components/script/dom/create.rs)
> - [components/script/dom/servoparser/mod.rs](../../components/script/dom/servoparser/mod.rs)
> - [components/script/dom/servoparser/async_html.rs](../../components/script/dom/servoparser/async_html.rs)
> - [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
> - [components/script/dom/svg/svgelement.rs](../../components/script/dom/svg/svgelement.rs)
> - [components/script/dom/svg/svggraphicselement.rs](../../components/script/dom/svg/svggraphicselement.rs)
> - [components/script/dom/element/element.rs](../../components/script/dom/element/element.rs)
> - [components/script/dom/node/node.rs](../../components/script/dom/node/node.rs)

---

## Overview

Stage 1 converts the `<svg>` tag in the HTML source into a fully constructed `SVGSVGElement` DOM node, inserted into the document tree with all its attributes. This happens as part of the normal HTML parsing process — Servo reuses the standard `html5ever` tree builder for SVG elements.

Unlike later stages, there is **nothing SVG-specific** about how this works. The same parser infrastructure that creates `<div>`, `<img>`, or `<table>` also creates `<svg>`. The only difference is the namespace dispatch: elements in the SVG namespace (`ns!(svg)`) go through a different creation function.

---

## Sub-stage 1.1 — HTML Tokenization & Tree Building

**Where:** `html5ever` (the HTML parser library, integrated into Servo)

The browser receives the raw HTML bytes. The tokenizer (`html5ever`) processes:

```html
<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
```

The tokenizer produces a **start tag token** with:
- `name: "svg"`
- `ns: http://www.w3.org/2000/svg` (determined by html5ever's tree construction algorithm — when it sees `<svg>` in the "in body" insertion mode, the HTML spec says to switch to the "SVG" foreign content mode and create the element with the SVG namespace)
- `attrs: [("width", "200"), ("height", "200"), ("viewBox", "0 0 200 200"), ("xmlns", "http://www.w3.org/2000/svg")]`

The tree builder calls the **tree sink**'s `create_element` method:

- **Sync parser:** [mod.rs:1695-1724](components/script/dom/servoparser/mod.rs#L1695-L1724)
- **Async parser:** [async_html.rs:834-849](components/script/dom/servoparser/async_html.rs#L834-L849)

Both eventually call [`create_element_for_token()`](components/script/dom/servoparser/mod.rs#L1982-L2052).

**Key input:**
```rust
QualName {
    ns:   "http://www.w3.org/2000/svg",  // = ns!(svg) atom
    local: "svg",                          // = local_name!("svg") atom
    prefix: None,
}
```

#### Debugging this sub-stage

**Breakpoints:**
- [mod.rs:1695](components/script/dom/servoparser/mod.rs#L1695) — tree sink `create_element()` (sync parser)
- [async_html.rs:834](components/script/dom/servoparser/async_html.rs#L834) — tree sink `create_element()` (async parser)

**SVG identification:**
Check the `name` parameter in the debugger. For all HTML elements (html, head, body), `name.ns` will be `ns!(html)`. For SVG, `name.ns` will be `ns!(svg)`. However, you can't easily inspect atom values in LLDB — see the workaround below.

**How to filter for SVG only:**
Instead of breaking here, break at [create.rs:440](components/script/dom/create.rs#L440) in sub-stage 1.2, where you CAN check `name.ns` against `ns!(svg)` by looking at the match arm taken.

**Call frequency:**
Once per element created during parsing. For our SVG test page:
- html, head, meta, title, body, **svg**, circle = **7 calls total**
- Only 1 of them (svg) takes the SVG path

---

## Sub-stage 1.2 — Element Creation Dispatch

**Where:** [create_element_for_token()](components/script/dom/servoparser/mod.rs#L1982-L2052)

This function implements the HTML spec's *create an element for the token* algorithm:

```
create_element_for_token(QualName, attrs, document, creator, ...)
```

**Step 1** (line 2009-2012): Check for `is` attribute (custom elements):
```rust
let is = attrs.iter()
    .find(|attr| attr.name.local.eq_str_ignore_ascii_case("is"))
    .map(|attr| LocalName::from(&attr.value));
```
For `<svg>`, there's no `is` attribute, so this is `None`.

**Step 2** (line 2019): Look up custom element definition:
```rust
let definition = document.lookup_custom_element_definition(
    &name.ns, &name.local, is.as_ref()
);
```
For `<svg>` in the SVG namespace: `None` (SVG elements are not customizable).

**Step 3** (line 2042-2047): Create the element:
```rust
let element = Element::create(cx, name, is, document, creator, creation_mode, None);
```

This calls `Element::create()` → [create_element()](components/script/dom/create.rs#L440-L455):

```rust
pub(crate) fn create_element(cx, name, is, document, creator, mode, proto) {
    let prefix = name.prefix.clone();
    match name.ns {
        ns!(html) => create_html_element(cx, name, prefix, is, document, creator, mode, proto),
        ns!(svg) => create_svg_element(cx, name, prefix, document, proto),    // ← OUR PATH
        _ => Element::new(cx, name.local, name.ns, prefix, document, proto),
    }
}
```

Because the namespace is `ns!(svg)` (not `ns!(html)`), it dispatches to `create_svg_element()`. Note that custom element checks (`is`, `definition`) are skipped for SVG elements.

**Breakpoint:** [create.rs:440](components/script/dom/create.rs#L440)
**Watch:** `name` variable in debugger:
```rust
name = QualName { prefix: None, ns: "http://www.w3.org/2000/svg", local: "svg" }
//                     ^ Option<Prefix>     ^ ns!(svg)             ^ local_name!("svg")
```

#### Debugging this sub-stage

**Breakpoints:**
- [mod.rs:1982](components/script/dom/servoparser/mod.rs#L1982) — `create_element_for_token()` entry
- [create.rs:440](components/script/dom/create.rs#L440) — `Element::create()` → `create_element()` dispatch

**SVG identification:**
At [create.rs:440](components/script/dom/create.rs#L440), examine the `name.ns` match:
- `ns!(html)` arm taken → HTML element (not SVG)
- `ns!(svg)` arm taken → **SVG element** ← this is ours
- Since the debugger can't show atom strings, look at which match arm the debugger stepped into

**Stepping trick:**
Set a breakpoint at `create.rs:440` and a breakpoint at `create.rs:96` (`create_svg_element`). When the line-440 breakpoint hits, step over to see which branch is taken. If it jumps to line 96 (`create_svg_element`), it's the SVG element. If it jumps elsewhere, it's an HTML element — continue (`F5`).

**Call frequency:**
Called for EVERY element in the page (html, head, meta, title, body, svg, circle).
Only the `svg` element hits the `ns!(svg)` branch.

---

## Sub-stage 1.3 — SVG Element Type Selection

**Where:** [create_svg_element()](components/script/dom/create.rs#L96-L117)

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
        _                    => make!(SVGElement),       // circle, rect, path, g, etc.
    }
}
```

For our `<svg>` tag, `name.local == local_name!("svg")`, so it creates an `SVGSVGElement`.

**Breakpoint:** [create.rs:114](components/script/dom/create.rs#L114)
**Watch:** `name.local` in debugger:
```rust
name.local = "svg"   // = local_name!("svg"), matches => make!(SVGSVGElement)
```

#### Debugging this sub-stage

**Breakpoints:**
- [create.rs:96](components/script/dom/create.rs#L96) — `create_svg_element()` entry
- [create.rs:114](components/script/dom/create.rs#L114) — `match name.local` (the type selection)

**SVG identification:**
This function is ONLY called for elements in the SVG namespace (`ns!(svg)`). But that includes child elements like `<circle>`, not just the root `<svg>`. To filter for ONLY the root `<svg>` element:
- Check `name.local` — if it equals `local_name!("svg")`, this is the root SVG element
- For `<circle>`, `name.local` would be `local_name!("circle")`, which falls to the `_ => make!(SVGElement)` default arm

**Stepping trick:**
At [create.rs:114](components/script/dom/create.rs#L114), step over the `match name.local` to see which constructor is called:
- `make!(SVGSVGElement)` — this IS our `<svg>` element
- `make!(SVGImageElement)` — an `<image>` element inside SVG
- `make!(SVGElement)` — any other SVG child (circle, rect, path, etc.)

**Call frequency:**
Once per SVG-namespace element: our page has `<svg>` and `<circle>` = **2 calls**.
Only the first call (svg) hits the `SVGSVGElement` branch.

---

## Sub-stage 1.4 — SVGSVGElement Construction

**Where:** [svgsvgelement.rs:49-76](components/script/dom/svg/svgsvgelement.rs#L49-L76)

### Step 1 — Inheritance chain initialization (`new_inherited`, lines 50-60)

```rust
fn new_inherited(local_name, prefix, document) -> SVGSVGElement {
    SVGSVGElement {
        svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
        uuid: Uuid::new_v4().to_string(),
        cached_serialized_data_url: Default::default(),  // = None
    }
}
```

This constructs the full inheritance chain:

```
SVGSVGElement
  └── SVGGraphicsElement::new_inherited()
        └── SVGElement::new_inherited_with_state()
              └── Element::new_inherited_with_state(local_name, ns!(svg), prefix, document)
                    └── Node::new_inherited(document)
                          └── EventTarget::new_inherited()
```

Each level initializes its own fields:

| Level | Fields Initialized | Debugger Values |
|-------|-------------------|-----------------|
| `EventTarget` | Event listener lists, type ID | `EventTarget { type_id: NodeType(SVGSVGElement), ... }` |
| `Node` | `owner_doc`, `flags: Cell<NodeFlags>`, `children_count: Cell<0>`, `layout_data: None` | `Node { owner_doc: &Document, flags: NodeFlags(0), children_count: Cell(0), layout_data: DomRefCell(None), ... }` |
| `Element` | `local_name: "svg"`, `namespace: ns!(svg)`, `prefix: None`, `attrs: Default::default()`, `state: Cell(empty)` | `Element { local_name: "svg", namespace: "http://www.w3.org/2000/svg", prefix: None, attrs: UniqueVec<Attr>([]), state: Cell(0), ... }` |
| `SVGElement` | `style_decl: None` | `SVGElement { style_decl: MutNullableDom(None) }` |
| `SVGGraphicsElement` | (no additional fields) | `SVGGraphicsElement { /* empty */ }` |
| `SVGSVGElement` | `uuid: "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"`, `cached_serialized_data_url: None` | `SVGSVGElement { uuid: "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a", cached_serialized_data_url: DomRefCell(None) }` |

**Key details:**
- **`uuid`** — A unique UUID v4 string generated at creation time via `Uuid::new_v4()`. Example value for this SVG: `"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"`. This is used as the `svg_id` throughout the pipeline to identify this specific SVG element (for cache lookups in image cache, rasterization tracking, etc.)
- **`cached_serialized_data_url`** — Initialized to `DomRefCell { value: None }`. This will be populated later in Stage 4 when the SVG subtree is serialized to a data URL.
- The `namespace` is set to `"http://www.w3.org/2000/svg"` (= `ns!(svg)`) — this is what distinguishes SVG elements from HTML elements in the DOM tree and is used later for style resolution and layout dispatch.

**Breakpoint:** [svgsvgelement.rs:50](components/script/dom/svg/svgsvgelement.rs#L50)
**Watch:**
```rust
uuid = "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"  // Uuid::new_v4().to_string()
cached_serialized_data_url = DomRefCell { value: None }
```

#### Debugging this sub-stage

**Breakpoints:**
- [svgsvgelement.rs:50](components/script/dom/svg/svgsvgelement.rs#L50) — `new_inherited()` (Rust struct construction)
- [svgsvgelement.rs:70](components/script/dom/svg/svgsvgelement.rs#L70) — `new()` → `reflect_node_with_proto()` (JS wrapper creation)

**SVG identification:**
This function is ONLY called for the root `<svg>` element (`SVGSVGElement`). Child SVG elements like `<circle>` use `SVGElement::new()`, not `SVGSVGElement::new()`. So if you hit this breakpoint, **it IS the SVG element** — no filtering needed.

**Key values to verify:**
- `uuid` — note this value (e.g., `"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"`). You'll see this same UUID again in Stages 2, 5, and 7 as `svg_id` / `svg_data.svg_id`. This is how you track the element across threads.
- `cached_serialized_data_url` — must be `DomRefCell(None)` at this point. It stays `None` until Stage 4.

**Call frequency:**
**Exactly once** for the root `<svg>` element. No other element creates an `SVGSVGElement`.

### Step 2 — DOM Reflection (`new`, lines 62-76)

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

**`reflect_node_with_proto`** ([node.rs:2794-2805](../../components/script/dom/node/node.rs#L2794-L2805)):

```rust
fn reflect_node_with_proto(cx, node, document, proto) -> DomRoot<N> {
    let window = document.window();
    reflect_dom_object_with_proto_and_cx(node, window, proto, cx)  // [reflector.rs:64](components/script/dom/bindings/reflector.rs#L64)
}
```

This creates the **JavaScript/SpiderMonkey wrapper** for the SVG element. Every DOM object in Servo has a two-part representation:
1. The **Rust struct** (`SVGSVGElement`) with all the native data
2. A **JS wrapper object** (a `JSObject` in SpiderMonkey) that makes it accessible from JavaScript

The JS wrapper is associated with the global `Window` object and uses the prototype chain from the WebIDL bindings (`SVGSVGElement.prototype → SVGGraphicsElement.prototype → SVGElement.prototype → Element.prototype → Node.prototype → EventTarget.prototype`).

**Breakpoint:** [svgsvgelement.rs:70](components/script/dom/svg/svgsvgelement.rs#L70)
**Watch:** Return value in debugger:
```rust
DomRoot<SVGSVGElement> {
    ptr: NonNull(0x0...)  // pointer to heap-allocated SVGSVGElement struct
}
// The Rust struct is boxed on the heap. Its JS wrapper (JSObject) is created
// by reflect_dom_object_with_proto_and_cx and linked via the global Window.
```

---

## Sub-stage 1.5 — Attribute Initialization

**Where:** [create_element_for_token()](components/script/dom/servoparser/mod.rs#L2049-L2052)

After the element is created, the parser sets all attributes from the HTML token:

```rust
for attr in attrs {
    element.set_attribute_from_parser(attr.name, attr.value, None, CanGc::from_cx(cx));  // [element.rs:2058](components/script/dom/element/element.rs#L2058)
}
```

For our test case, this processes four attributes:

| Attribute | Value | AttrValue Variant | Debugger Display |
|-----------|-------|-------------------|------------------|
| `width` | `"200"` | `AttrValue::LengthPercentage` | `LengthPercentage("200", Ok(Length(Length::Fixed(CSSPixelLength(200.0)))))` |
| `height` | `"200"` | `AttrValue::LengthPercentage` | `LengthPercentage("200", Ok(Length(Length::Fixed(CSSPixelLength(200.0)))))` |
| `viewBox` | `"0 0 200 200"` | `AttrValue::String` | `String("0 0 200 200")` |
| `xmlns` | `"http://www.w3.org/2000/svg"` | `AttrValue::String` | `String("http://www.w3.org/2000/svg")` |

The `width` and `height` attributes are parsed specially because `SVGSVGElement` overrides `parse_plain_attribute` ([svgsvgelement.rs:221-250](components/script/dom/svg/svgsvgelement.rs#L221-L250)):

```rust
fn parse_plain_attribute(&self, name, value) -> AttrValue {
    match *name {
        local_name!("width") | local_name!("height") => {
            // Parse as CSS LengthPercentage (allows unitless values)
            let val = LengthPercentage::parse_quirky(&context, parser, AllowQuirks::Always);
            AttrValue::LengthPercentage(value.to_string(), val.ok())
        },
        _ => self.super_type().unwrap().parse_plain_attribute(name, value),
    }
}
```

This means later in Stage 2, when layout reads `svg_data.width`, it gets an already-parsed `LengthPercentage` value rather than a raw string — no re-parsing needed.

Other attributes (like `viewBox`) are stored as raw strings via the default `Element::parse_plain_attribute`.

**Breakpoint:** [svgsvgelement.rs:221](components/script/dom/svg/svgsvgelement.rs#L221) — `parse_plain_attribute`
**Watch:** Input and return in debugger:
```rust
// First call:
name  = &LocalName("width")    // matches local_name!("width")
value = DOMString("200")
// → returns AttrValue::LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))

// Second call:
name  = &LocalName("height")   // matches local_name!("height")
value = DOMString("200")
// → returns AttrValue::LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))

// Third & fourth calls (viewBox, xmlns):
name  = &LocalName("viewBox")  // does NOT match width/height
value = DOMString("0 0 200 200")
// → falls through to super_type().parse_plain_attribute() → AttrValue::String(...)
```

#### Debugging this sub-stage

**Breakpoints:**
- [svgsvgelement.rs:221](components/script/dom/svg/svgsvgelement.rs#L221) — `parse_plain_attribute()` for width/height parsing
- [element.rs:2058](components/script/dom/element/element.rs#L2058) — `set_attribute_from_parser()` for all attribute setting

**SVG identification:**
`parse_plain_attribute()` at [svgsvgelement.rs:221](components/script/dom/svg/svgsvgelement.rs#L221) is a method on `SVGSVGElement`, so it's ONLY called for the root `<svg>` element. If you're at this breakpoint, it's definitely for our SVG.

The breakpoint hits once per attribute that's being set on the SVG element (width, height, viewBox, xmlns). You may also see calls from `SVGElement::parse_plain_attribute` for child elements.

**Call frequency for our SVG:**
- `parse_plain_attribute` hits **4 times** (width, height, viewBox, xmlns)
- `set_attribute_from_parser` is called for ALL attributes of ALL elements during parsing

**What to track across the 4 calls:**
| Call | `name` | `value` | Return type |
|------|--------|---------|-------------|
| 1st | `"width"` | `"200"` | `AttrValue::LengthPercentage` |
| 2nd | `"height"` | `"200"` | `AttrValue::LengthPercentage` |
| 3rd | `"viewBox"` | `"0 0 200 200"` | `AttrValue::String` (falls through to parent) |
| 4th | `"xmlns"` | `"http://www.w3.org/2000/svg"` | `AttrValue::String` (falls through to parent) |

---

## Sub-stage 1.6 — Tree Insertion

**Where:** [mod.rs:1581-1609](components/script/dom/servoparser/mod.rs#L1581-L1609)

After the element is created and its attributes are set, the parser inserts it into the DOM tree:

```rust
fn insert(cx, parent, reference_child, child, parsing_algorithm, ...) {
    match child {
        NodeOrText::AppendNode(n) => {
            // https://html.spec.whatwg.org/multipage/#insert-a-foreign-element
            let element_in_non_fragment =
                parsing_algorithm != ParsingAlgorithm::Fragment && n.is::<Element>();
            if element_in_non_fragment {
                custom_element_reaction_stack.push_new_element_queue();
            }
            parent.InsertBefore(cx, &n, reference_child).unwrap();
            if element_in_non_fragment {
                custom_element_reaction_stack.pop_current_element_queue(cx);
            }
        },
        // ...
    }
}
```

When `<svg>` is inserted, the `<body>` node is the parent. `InsertBefore` triggers:

1. `Node::InsertBefore()` — DOM spec's *insert* algorithm
2. Sets `IS_IN_A_DOCUMENT_TREE`, `IS_CONNECTED` flags on the SVG element and all its descendants
3. The SVG element is now a child of `<body>` in the DOM tree
4. Each child element inside `<svg>` (like `<circle>`) goes through the same create/insert process, creating `SVGElement` nodes with `namespace: ns!(svg)`

After insertion, the DOM tree looks like:

```
Document
  └── html (HTMLHtmlElement)
        └── body (HTMLBodyElement)
              └── svg (SVGSVGElement) ← just created by Stage 1
                    ├── width  = "200"     (AttrValue::LengthPercentage)
                    ├── height = "200"     (AttrValue::LengthPercentage)
                    ├── viewBox = "0 0 200 200" (AttrValue::String)
                    ├── xmlns  = "http://www.w3.org/2000/svg" (AttrValue::String)
                    └── circle (SVGElement) ← created when parser continued
                          ├── cx = "100"
                          ├── cy = "100"
                          ├── r  = "50"
                          └── fill = "blue"
```

#### Debugging this sub-stage

**Breakpoints:**
- [mod.rs:1581](components/script/dom/servoparser/mod.rs#L1581) — the `insert()` function in the tree sink

**SVG identification:**
Check the `child` parameter — if it's `NodeOrText::AppendNode(n)` where `n` is an `SVGSVGElement`, then this is SVG insertion. Since you can't easily check types in LLDB, use the call stack: if the previous call was `create_svg_element` → `SVGSVGElement::new`, then this insertion is for the SVG element.

**What happens during insertion:**
1. `parent.InsertBefore(cx, &n, reference_child)` — adds the SVG to body's child list
2. `IS_IN_A_DOCUMENT_TREE` and `IS_CONNECTED` flags are set on the node
3. The SVG element now has a parent in the live DOM tree

**Call frequency:**
Once per created element. The SVG element itself is inserted once. Its child `<circle>` is inserted separately.

**No separate breakpoint needed** — the insertion is standard DOM tree manipulation, visible from the call stack after sub-stage 1.5.

---

## Sub-stage 1.7 — Post-Creation (what happens *after* Stage 1)

Once the SVG element is in the DOM tree, several things happen automatically as part of the normal page load cycle:

1. **Style resolution** (Style thread/Stylo): The element gets its `ComputedValues` computed via the CSS cascade — this is what Stage 2 reads.

2. **Layout** (Layout thread): The layout traversal encounters the node and determines it's replaced content — this enters Stage 2.

3. **VirtualMethods hooks** ([svgsvgelement.rs:193-280](components/script/dom/svg/svgsvgelement.rs#L193-L280)):
   - [`attribute_mutated`](components/script/dom/svg/svgsvgelement.rs#L198) (line 198): If any attribute changes, invalidates the cached serialized data URL
   - [`children_changed`](components/script/dom/svg/svgsvgelement.rs#L253) (line 253): If children are added/removed, also invalidates
   - [`unbind_from_tree`](components/script/dom/svg/svgsvgelement.rs#L261) (line 261): If removed from DOM, evicts cached/rasterized images from the image cache

But these are triggered later — they're not part of Stage 1 itself.

#### Debugging this sub-stage

**Breakpoints:**
- [svgsvgelement.rs:198](components/script/dom/svg/svgsvgelement.rs#L198) — `attribute_mutated()` (fires when SVG attributes change after creation)
- [svgsvgelement.rs:253](components/script/dom/svg/svgsvgelement.rs#L253) — `children_changed()` (fires when SVG children are added/removed)
- [svgsvgelement.rs:261](components/script/dom/svg/svgsvgelement.rs#L261) — `unbind_from_tree()` (fires when SVG is removed from DOM)

**SVG identification:**
All three are methods on `SVGSVGElement`, so they ONLY fire for the root `<svg>` element. No filtering needed.

**When they fire:**
- `attribute_mutated`: If script modifies an attribute on the SVG element (e.g., `svgElement.setAttribute('width', '300')`). Not during initial parsing — attribute setting during parsing uses a different code path.
- `children_changed`: If child elements are added/removed dynamically. Not during initial parse.
- `unbind_from_tree`: Only if the SVG element is removed from the document.

**These don't fire during normal page load** — they're only triggered by dynamic DOM manipulation from script.

---

## Complete Data Flow Diagram

```
     HTML source "<svg width='200' height='200' viewBox='0 0 200 200' xmlns='...'>"
                               │
                               ▼
               html5ever tokenizer
               (produces StartTag token)
           name="svg", ns="http://www.w3.org/2000/svg"
           attrs=[("width","200"), ("height","200"),
                  ("viewBox","0 0 200 200"), ("xmlns","http://www.w3.org/2000/svg")]
                               │
                               ▼
                    html5ever tree builder
                    (determines SVG namespace per HTML spec)
                               │
                    QualName { prefix: None, ns: "http://www.w3.org/2000/svg", local: "svg" }
                               │
                               ▼
                    ServoParser tree sink
                    create_element() [mod.rs:1695]
                               │
                               ▼
                    create_element_for_token() [mod.rs:1982]
                    (steps 1-12 of the HTML spec algorithm)
                               │
                               ▼
                    Element::create() → create_element() [create.rs:440]
                               │
                    ┌──────────┴──────────┐
                    │ ns!(svg)            │ ns!(html)
                    ▼                     ▼
              create_svg_element()   create_html_element()
                    │
                    │ local_name!("svg")
                    ▼
              SVGSVGElement::new() [svgsvgelement.rs:63]
                    │
            ┌───────┴───────┐
            ▼               ▼
   new_inherited()     reflect_node_with_proto()
   (builds chain)      (creates JS wrapper)
            │               │
            ▼               ▼
   SVGSVGElement {       SpiderMonkey JSObject
     uuid: "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a",  (prototype: SVGSVGElement → SVGGraphicsElement
     cached_serialized_data_url: DomRefCell(None)        → SVGElement → Element → Node → EventTarget)
   }
            └───────┬───────┘
                    ▼
              DomRoot<SVGSVGElement>
                    │
                    ▼
              set_attribute_from_parser()  × 4 calls
              ├── width="200"  → AttrValue::LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))
              ├── height="200" → AttrValue::LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))
              ├── viewBox="0 0 200 200" → AttrValue::String("0 0 200 200")
              └── xmlns="http://www.w3.org/2000/svg" → AttrValue::String("http://www.w3.org/2000/svg")
                    │
                    ▼
              parent.InsertBefore(body, svg) → DOM tree
              Node flags: IS_IN_A_DOCUMENT_TREE | IS_CONNECTED
                    │
                    ▼
              ──→ Stage 2
              (style resolution on next layout traversal)
```

---

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | What You'll See in Debugger |
|---|------|-----------|------------------------------|
| 1.2 | Namespace dispatch | [create.rs:440](components/script/dom/create.rs#L440) | `name = QualName { ns: "http://www.w3.org/2000/svg", local: "svg", prefix: None }` |
| 1.3 | SVG type selection | [create.rs:114](components/script/dom/create.rs#L114) | `name.local = "svg"` → matches `local_name!("svg")` |
| 1.4-i | Inheritance init | [svgsvgelement.rs:50](components/script/dom/svg/svgsvgelement.rs#L50) | `uuid = "9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"`, `cached_serialized_data_url = DomRefCell(None)` |
| 1.4-ii | JS reflection | [svgsvgelement.rs:70](components/script/dom/svg/svgsvgelement.rs#L70) | Returns `DomRoot<SVGSVGElement>` with pointer to heap-allocated struct |
| 1.5 | Attribute parsing | [svgsvgelement.rs:221](components/script/dom/svg/svgsvgelement.rs#L221) | `name = "width"` → returns `AttrValue::LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))` |

### Key Variables to Track

| Variable | Type | Source | Debugger Display |
|----------|------|--------|------------------|
| `element.namespace` | `Namespace` | Element init | `"http://www.w3.org/2000/svg"` (= `ns!(svg)`) |
| `element.local_name` | `LocalName` | QualName from parser | `"svg"` |
| `element.attrs` | `Vec<Attr>` | Parser `set_attribute_from_parser()` | `[width, height, viewBox, xmlns]` — 4 Attr entries |
| `uuid` | `String` | `Uuid::new_v4()` | `"9c7b6a3d-2f44-4e80-b2f5-8d5c9c3b1e8a"` |
| `cached_serialized_data_url` | `DomRefCell<Option<Result<ServoUrl, ()>>>` | Default | `DomRefCell { value: None }` |
| `width` attr | `AttrValue::LengthPercentage` | `parse_plain_attribute` | `LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))` |
| `height` attr | `AttrValue::LengthPercentage` | `parse_plain_attribute` | `LengthPercentage("200", Ok(Length(CSSPixelLength(200.0))))` |
| `viewBox` attr | `AttrValue::String` | Default Element parser | `String("0 0 200 200")` |
| `xmlns` attr | `AttrValue::String` | Default Element parser | `String("http://www.w3.org/2000/svg")` |
| Parent node | `Node` | Tree insertion (body) | `body` (HTMLBodyElement) |
| Node flags | `NodeFlags` | Tree insertion | `NodeFlags(IS_IN_A_DOCUMENT_TREE \| IS_CONNECTED)` |

### Inheritance Chain

```
EventTarget
  └── Node              — flags, owner_doc, children_count, layout_data
        └── Element     — local_name="svg", namespace=ns!(svg), attrs, state
              └── SVGElement  — style_decl
                    └── SVGGraphicsElement  — (no extra fields)
                          └── SVGSVGElement — uuid, cached_serialized_data_url
```

### Important: What Stage 1 Does NOT Do

Stage 1 is purely about DOM construction. It does **not**:
- Resolve any CSS or compute styles (that's Stage 2)
- Serialize the SVG subtree to a data URL (that's Stage 4)
- Interact with the image cache (that's Stages 5+)
- Do any layout or rendering (that's Stages 8+)
- Set up `SVGElementData.source` for layout (the field stays `None` — it will be populated in Stage 4)

The entire Stage 1 runs on the **Script thread's HTML parser** and produces a fully functional `SVGSVGElement` in the DOM tree, ready for styling and layout.
