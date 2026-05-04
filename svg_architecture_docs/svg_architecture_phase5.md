# SVG Architecture Study - Phase 5: SVG Element Hierarchy & Attribute Handling

## Overview

This document covers the SVG DOM element hierarchy in Servo — how SVG elements are created, structured, and how they handle attributes. Unlike HTML elements, most SVG elements share a single generic `SVGElement` type with no custom behavior. Only `<svg>` (`SVGSVGElement`), `<image>` (`SVGImageElement`), and the generic `SVGGraphicsElement` base class have dedicated implementations. This sparse hierarchy reflects Servo's current approach of treating entire SVG subtrees as opaque replaced elements rather than independently styled DOM nodes.

## Key Files

| File | Purpose | Importance |
|------|---------|------------|
| [components/script/dom/create.rs:96-115](components/script/dom/create.rs) | SVG element creation dispatch | **Most Critical** |
| [components/script/dom/svg/svgelement.rs](components/script/dom/svg/svgelement.rs) | Generic `SVGElement` (base for most SVG tags) | **Most Critical** |
| [components/script/dom/svg/svggraphicselement.rs](components/script/dom/svg/svggraphicselement.rs) | `SVGGraphicsElement` base class | High |
| [components/script/dom/svg/svgimageelement.rs](components/script/dom/svg/svgimageelement.rs) | `SVGImageElement` — external images in SVG | High |
| [components/script/dom/svg/svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs) | `SVGSVGElement` — root `<svg>` element | **Most Critical** |
| [components/script/dom/node/node.rs](components/script/dom/node/node.rs) | `Node::svg_data()` downcast for layout | Medium |

## SVG Element Type Hierarchy

### Inheritance Tree

```
Node
  └── Element (ns: svg)
        └── SVGElement                                            [all non-specific SVG tags]
              └── SVGGraphicsElement                               [graphics elements]
                    ├── SVGSVGElement                              [<svg>]
                    └── SVGImageElement                            [<image>]
```

### DOM Struct Definitions

**SVGElement** — Line 30 of svgelement.rs:
```rust
#[dom_struct]
pub(crate) struct SVGElement {
    element: Element,
    style_decl: MutNullableDom<CSSStyleDeclaration>,
}
```
- All SVG elements that don't have a specific type fall through to this
- Holds only `Element` + `CSSStyleDeclaration` (for `style` attribute)
- No SVG-specific fields, no child tracking, no presentation attribute support

**SVGGraphicsElement** — Line 15 of svggraphicselement.rs:
```rust
#[dom_struct]
pub(crate) struct SVGGraphicsElement {
    svgelement: SVGElement,
}
```
- Empty wrapper around SVGElement
- Serves as the base class for graphics elements with transform support per SVG spec
- No methods or fields beyond construction

**SVGImageElement** — Line 25 of svgimageelement.rs:
```rust
#[dom_struct]
pub(crate) struct SVGImageElement {
    svggraphicselement: SVGGraphicsElement,
}
```
- Represents `<image>` element in SVG (external raster images embedded in SVG)
- Parses `width` and `height` attributes as u32 (default 300x150)
- Currently `fetch_image_resource()` only queues an "error" event
- TODO: Fetch and display embedded image resources

**SVGSVGElement** — Line 35 of svgsvgelement.rs:
```rust
#[dom_struct]
pub(crate) struct SVGSVGElement {
    svggraphicselement: SVGGraphicsElement,
    uuid: String,
    #[no_trace]
    cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl, ()>>>,
}
```
- Root `<svg>` element with serialization support
- `uuid`: Unique identifier for cache invalidation
- `cached_serialized_data_url`: Cached data URL of serialized subtree

## SVG Element Creation Flow

### Dispatch in create.rs

When a new SVG element is created, `create_element_internal()` in [create.rs](components/script/dom/create.rs) checks the namespace:

```rust
ns!(svg) => create_svg_element(cx, name, prefix, document, proto),
```

The `create_svg_element()` function dispatches by local name:

```rust
fn create_svg_element(cx, name, prefix, document, proto) {
    assert_eq!(name.ns, ns!(svg));
    match name.local {
        local_name!("image") => make!(SVGImageElement),
        local_name!("svg") => make!(SVGSVGElement),
        _ => make!(SVGElement),    // <-- ALL other SVG tags fall here
    }
}
```

### Tags That Fall Through to Generic SVGElement

All these SVG tags produce the same generic `SVGElement` with no special behavior:

| Category | Tags |
|----------|------|
| **Basic Shapes** | `circle`, `ellipse`, `line`, `path`, `polygon`, `polyline`, `rect` |
| **Text** | `text`, `tspan`, `tref`, `textPath` |
| **Structural** | `g`, `defs`, `symbol`, `use`, `clipPath`, `mask`, `pattern` |
| **Gradients** | `linearGradient`, `radialGradient`, `stop` |
| **Filters** | `filter`, `feGaussianBlur`, `feColorMatrix`, etc. |
| **Metadata** | `desc`, `title`, `metadata` |
| **Other** | `a`, `marker`, `view`, `switch`, `foreignObject` |

All these elements share the same struct, meaning:
- No custom attribute parsing beyond what `Element` provides
- No specialized DOM methods
- No script-side rendering logic
- All rendering happens entirely through `usvg`/`resvg` after serialization

## Attribute Handling

### SVGElement Attribute Support

The generic `SVGElement` handles attributes through the standard `Element` attribute mechanisms:

**VirtualMethods for SVGElement** (svgelement.rs:76-103):
```rust
impl VirtualMethods for SVGElement {
    fn attribute_mutated(&self, cx, attr, mutation) {
        // Only handles nonce attribute updates
        // No SVG-specific attribute handling
    }
}
```

**Key observation**: The generic `SVGElement` has no SVG-specific attribute mutation handlers. It relies entirely on the base `Element`'s generic attribute storage. SVG presentational attributes like `fill`, `stroke`, `stroke-width` are stored as plain attributes but have no effect on rendering at the DOM level — they only matter when `usvg` re-parses the serialized XML.

### SVGSVGElement Custom Attribute Parsing

`SVGSVGElement` overrides `parse_plain_attribute()` for `width` and `height` (svgsvgelement.rs:221-251):

```rust
fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
    match *name {
        local_name!("width") | local_name!("height") => {
            // Parses as CSS LengthPercentage (with quirks mode for unitless values)
            AttrValue::LengthPercentage(value.to_string(), val.ok())
        },
        _ => self.super_type().unwrap().parse_plain_attribute(name, value),
    }
}
```

- Uses CSS length parser with `ALLOW_UNITLESS_LENGTH` (quirks mode)
- Stores as `AttrValue::LengthPercentage` with the raw string and parsed value
- These values are extracted later for `SVGElementData`

### SVGImageElement Custom Attribute Parsing

`SVGImageElement` parses `width` and `height` as u32 integers (svgimageelement.rs:99-108):

```rust
fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
    match *name {
        local_name!("width") => AttrValue::from_u32(value.into(), DEFAULT_WIDTH),
        local_name!("height") => AttrValue::from_u32(value.into(), DEFAULT_HEIGHT),
        _ => self.super_type().unwrap().parse_plain_attribute(name, value),
    }
}
```

### Attribute → SVGElementData Flow

When layout requests SVG data, `SVGSVGElement::data()` extracts:

```
SVG attributes:
    width  → SVGElementData.width   (Option<&AttrValue>)
    height → SVGElementData.height  (Option<&AttrValue>)
    viewBox → SVGElementData.view_box (Option<&AttrValue>)
    
    cached_serialized_data_url → SVGElementData.source (Option<Result<ServoUrl, ()>>)
```

Only these three attributes plus the data URL are passed to layout. All other SVG attributes (`fill`, `stroke`, `d`, `cx`, `cy`, `r`, etc.) are **never extracted or used** by the layout system — they're only present in the serialized XML for `usvg` to re-parse.

## Presentational Attributes

SVG presentational attributes (attributes like `fill="red"` that also exist as CSS properties) are **not mapped to CSS** in Servo's script layer. The standard approach per the SVG spec is:

1. Parse presentational attribute value
2. Create a CSS declaration from the attribute value
3. Apply with low specificity in the cascade

**Current Status**: Servo does not implement SVG presentational attribute → CSS mapping. This means:
- `<circle fill="red" />` does not apply `fill` via CSS cascade
- The `fill` is only preserved as a DOM attribute in the serialized XML
- `usvg` re-parses the attribute from the serialized XML during rasterization

## SVG Content Change Detection

### SVGSVGElement Change Tracking

When SVG content changes, `SVGSVGElement` detects it via `VirtualMethods::children_changed()` (svgsvgelement.rs:253-259):

```rust
fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation) {
    if let Some(super_type) = self.super_type() {
        super_type.children_changed(cx, mutation);
    }
    self.invalidate_cached_serialized_subtree();
}
```

This is called when:
- Child nodes are added or removed
- Text content changes
- Attributes on child elements change (via attribute mutation bubbling)

### Cache Invalidation Chain

```
children_changed() 
    → invalidate_cached_serialized_subtree()
        → cached_serialized_data_url = None
        → node.dirty(NodeDamage::Other)  // triggers reflow

unbind_from_tree()
    → evict_rasterized_image(self.uuid)   // image cache cleanup
    → remove_cached_image(&url)           // layout cache cleanup
    → evict_completed_image(&url, ...)    // image cache entry cleanup
    → invalidate_cached_serialized_subtree()
```

## SVG Serialization Traversal

### XML Serialization Scope

When `serialize_and_cache_subtree()` is called, it uses:

```rust
Node::xml_serialize(TraversalScope::IncludeNode)
```

This serializes the `<svg>` element and its entire subtree. The `TraversalScope::IncludeNode` means both the root `<svg>` and all descendants are included in the serialized output.

### `<use>` Element Pre-Processing

Before serialization, `process_use_elements()` handles `<use href="#id">` references:

```rust
for node in root_node.traverse_preorder(ShadowIncluding::No) {
    if element.local_name() == &local_name!("use") {
        // Clone referenced element and append to root
    }
}
```

This clones the referenced node and appends it under `<svg>`, ensuring the serialized XML includes the expanded content. Cloned nodes are removed after serialization via `cleanup_cloned_nodes()`.

## Summary

### Current State

| Aspect | Status |
|--------|--------|
| SVG element types | Only 4: SVGElement, SVGGraphicsElement, SVGImageElement, SVGSVGElement |
| Shape elements (path, circle, etc.) | All generic SVGElement, no custom behavior |
| Text elements | Generic SVGElement, no text layout in DOM |
| Presentational attributes | Not mapped to CSS, only preserved in serialized XML |
| Attribute changes | Detected via children_changed, triggers full re-serialization |
| `<use>` elements | Pre-serialization cloning for expansion |

### Key Architectural Issue

The SVG DOM hierarchy is essentially a **pass-through storage layer**. Most SVG elements have no DOM-level behavior — they exist solely as XML nodes to be serialized and re-parsed by `usvg`. This means:

1. All SVG rendering logic is delegated to `resvg`/`usvg`
2. DOM mutations require full subtree re-serialization
3. Presentational attributes have no CSS cascade presence
4. Adding new SVG element support requires changes in `usvg`, not Servo's DOM layer

A proper implementation would need SVG elements to participate in layout as native nodes, with presentational attributes converted to CSS and shape/text elements creating their own fragment tree entries.