# SVG Architecture Study - Phase 1: DOM Layer

## Overview

This document details the DOM layer implementation of SVG elements in Servo, focusing on how SVG elements are represented, their lifecycle, and the current serialization-based approach that causes the three test failures (CSS inheritance, web fonts, crisp transforms).

## DOM Architecture Overview

In Servo's architecture, SVG elements are DOM objects that inherit from the general DOM element hierarchy:

```
Node
└── Element
    └── SVGGraphicsElement (for SVG elements with graphics capabilities)
        ├── SVGSVGElement (root <svg> element)
        ├── Other SVG graphics elements (rect, circle, text, etc.)
        └── SVGImageElement (<image> element)
```

## Key Files and Their Roles

| File | Purpose | Importance |
|------|---------|------------|
| [components/script/dom/svg/svgsvgelement.rs](components/script/dom/svg/svgsvgelement.rs) | Root `<svg>` element implementation, handles serialization and caching | **Most Critical** |
| [components/script/dom/svg/svgelement.rs](components/script/dom/svg/svgelement.rs) | Base SVG element class, provides common SVG functionality | High |
| [components/script/dom/svg/svggraphicselement.rs](components/script/dom/svg/svggraphicselement.rs) | Graphics element base class for SVG elements with rendering | High |
| [components/script/dom/svg/svgimageelement.rs](components/script/dom/svg/svgimageelement.rs) | SVG `<image>` element implementation | Medium |
| [components/script/dom/svg/mod.rs](components/script/dom/svg/mod.rs) | Module exports for SVG DOM elements | Low |

## SVGSVGElement - Root SVG Element

The `SVGSVGElement` struct is the heart of SVG handling in Servo's current implementation.

### Structure and Inheritance

```rust
#[dom_struct]
pub(crate) struct SVGSVGElement {
    svggraphicselement: SVGGraphicsElement,
    uuid: String,
    // The XML source of subtree rooted at this SVG element, serialized into
    // a base64 encoded `data:` url. This is cached to avoid recomputation
    // on each layout and must be invalidated when the subtree changes.
    #[no_trace]
    cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl, ()>>>,
}
```

**Key Fields:**
- `uuid`: Unique identifier for caching rasterized images
- `cached_serialized_data_url`: Cached serialized SVG as data URL (base64 encoded)

### Serialization and Caching Mechanism

The core of the current "hacky" approach is in the `serialize_and_cache_subtree()` method (lines 79-103):

```rust
pub(crate) fn serialize_and_cache_subtree(&self) {
    // TODO: https://github.com/servo/servo/issues/43142
    let mut cx = unsafe { script_bindings::script_runtime::temp_cx() };
    let cx = &mut cx;
    let cloned_nodes = self.process_use_elements(cx);
    
    let serialize_result = self
        .upcast::<Node>()
        .xml_serialize(TraversalScope::IncludeNode);
    
    self.cleanup_cloned_nodes(cx, &cloned_nodes);
    
    let Ok(xml_source) = serialize_result else {
        *self.cached_serialized_data_url.borrow_mut() = Some(Err(()));
        return;
    };
    
    let xml_source: String = xml_source.into();
    let base64_encoded_source = base64::engine::general_purpose::STANDARD.encode(xml_source);
    let data_url = format!("data:image/svg+xml;base64,{}", base64_encoded_source);
    match ServoUrl::parse(&data_url) {
        Ok(url) => *self.cached_serialized_data_url.borrow_mut() = Some(Ok(url)),
        Err(error) => error!("Unable to parse serialized SVG data url: {error}"),
    };
}
```

**Process Flow:**
1. Process `<use>` elements (clone referenced elements into the tree)
2. Serialize the entire SVG subtree to XML using `xml_serialize()`
3. Encode XML as base64
4. Create a data URL: `data:image/svg+xml;base64,...`
5. Cache the result in `cached_serialized_data_url`

### Data Transfer to Layout

The `LayoutDom` implementation (lines 170-191) provides data to the layout system:

```rust
impl<'dom> LayoutDom<'dom, SVGSVGElement> {
    #[expect(unsafe_code)]
    pub(crate) fn data(self) -> SVGElementData<'dom> {
        let svg_id = self.unsafe_get().uuid.clone();
        let element = self.upcast::<Element>();
        let width = element.get_attr_for_layout(&ns!(), &local_name!("width"));
        let height = element.get_attr_for_layout(&ns!(), &local_name!("height"));
        let view_box = element.get_attr_for_layout(&ns!(), &local_name!("viewBox"));
        SVGElementData {
            source: unsafe {
                self.unsafe_get()
                    .cached_serialized_data_url
                    .borrow_for_layout()
                    .clone()
            },
            width,
            height,
            view_box,
            svg_id,
        }
    }
}
```

This creates an `SVGElementData` struct that contains:
- `source`: The cached data URL (or `None` if not serialized yet)
- `width`, `height`, `view_box`: SVG dimension attributes
- `svg_id`: Unique identifier for caching

### Invalidation Triggers

The cache must be invalidated when the SVG subtree changes. This happens through several virtual method overrides:

**1. Attribute Changes** (lines 198-219):
```rust
fn attribute_mutated(
    &self,
    cx: &mut js::context::JSContext,
    attr: &Attr,
    mutation: AttributeMutation,
) {
    self.super_type()
        .unwrap()
        .attribute_mutated(cx, attr, mutation);
    
    self.invalidate_cached_serialized_subtree();  // Invalidate cache
}
```

**2. Children Changes** (lines 253-259):
```rust
fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation) {
    if let Some(super_type) = self.super_type() {
        super_type.children_changed(cx, mutation);
    }
    
    self.invalidate_cached_serialized_subtree();  // Invalidate cache
}
```

**3. Tree Unbinding** (lines 261-279):
```rust
fn unbind_from_tree(&self, context: &UnbindContext<'_>, can_gc: CanGc) {
    // ... cleanup code ...
    self.invalidate_cached_serialized_subtree();
}
```

The actual invalidation just clears the cache:
```rust
fn invalidate_cached_serialized_subtree(&self) {
    *self.cached_serialized_data_url.borrow_mut() = None;
    self.upcast::<Node>().dirty(NodeDamage::Other);
}
```

### `<use>` Element Processing

SVG `<use>` elements require special handling during serialization (lines 105-162):

```rust
fn process_use_elements(&self, cx: &mut JSContext) -> Vec<DomRoot<Node>> {
    let mut cloned_nodes = Vec::new();
    let root_node = self.upcast::<Node>();
    
    for node in root_node.traverse_preorder(ShadowIncluding::No) {
        if let Some(element) = node.downcast::<Element>() {
            if element.local_name() == &local_name!("use") {
                if let Some(cloned) = self.process_single_use_element(cx, element) {
                    cloned_nodes.push(cloned);
                }
            }
        }
    }
    
    cloned_nodes
}
```

**Process for each `<use>` element:**
1. Extract `href` attribute (e.g., `#someId`)
2. Find referenced element by ID
3. Clone the referenced element and its children
4. Insert clone into the tree for serialization
5. Remove clones after serialization

This is necessary because serialization needs the actual content, not just references.

### Attribute Parsing

SVG-specific attributes like `width` and `height` need special parsing (lines 221-251):

```rust
fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
    match *name {
        local_name!("width") | local_name!("height") => {
            let value = &value.str();
            let parser_input = &mut ParserInput::new(value);
            let parser = &mut Parser::new(parser_input);
            let doc = self.owner_document();
            let url = doc.url().into_url().into();
            let context = ParserContext::new(
                Origin::Author,
                &url,
                None,
                ParsingMode::ALLOW_UNITLESS_LENGTH,
                doc.quirks_mode(),
                /* namespaces = */ Default::default(),
                None,
                None,
            );
            let val = LengthPercentage::parse_quirky(
                &context,
                parser,
                style::values::specified::AllowQuirks::Always,
            );
            AttrValue::LengthPercentage(value.to_string(), val.ok())
        },
        _ => self
            .super_type()
            .unwrap()
            .parse_plain_attribute(name, value),
    }
}
```

This handles percentage and unitless values for SVG dimensions.

## SVGElement - Base SVG Element

The `SVGElement` struct (in [svgelement.rs](components/script/dom/svg/svgelement.rs)) provides the foundation for all SVG elements.

### Key Structure

```rust
#[dom_struct]
pub(crate) struct SVGElement {
    element: Element,
    #[ignore_malloc_size_of = "Arc is shared with style system"]
    style_decl: DomRefCell<Option<Arc<Locked<PropertyDeclarationBlock>>>>,
}
```

**Important Fields:**
- `element`: Inherits from base `Element`
- `style_decl`: CSS style declaration block for SVG presentation attributes

### Style Handling

SVG elements can have presentation attributes that map to CSS properties. The `style_decl` field stores these as a CSS declaration block.

**Key Methods:**
- `style()`: Returns the style declaration
- `parse_plain_attribute()`: Override for SVG presentation attributes
- `attribute_affects_presentational_hints()`: Determines which attributes affect rendering

## SVGGraphicsElement - Graphics Base Class

`SVGGraphicsElement` (in [svggraphicselement.rs](components/script/dom/svg/svggraphicselement.rs)) inherits from `SVGElement` and adds graphics-specific functionality.

### Purpose
- Base class for all SVG elements that can be rendered
- Handles transform attributes and other graphics properties
- Provides common graphics-related virtual methods

## SVGImageElement - SVG `<image>` Element

`SVGImageElement` (in [svgimageelement.rs](components/script/dom/svg/svgimageelement.rs)) represents the SVG `<image>` element.

### Key Characteristics
- Can reference external images or other SVGs
- Similar to HTML `<img>` but for SVG context
- Has `href`, `width`, `height` attributes
- Needs to integrate with image loading system

## Common Patterns and Conventions

### 1. Virtual Method Overrides
SVG elements override virtual methods for:
- Attribute parsing (`parse_plain_attribute`)
- Presentational hints (`attribute_affects_presentational_hints`)
- Tree lifecycle (`children_changed`, `unbind_from_tree`)

### 2. Layout Data Extraction
Each SVG element has a `LayoutDom` implementation with a `data()` method that extracts information needed by the layout system.

### 3. Attribute Inheritance
SVG uses a mix of XML attributes and CSS properties:
- Presentation attributes (e.g., `fill="green"`) map to CSS properties
- Some attributes have both XML and CSS forms
- CSS inheritance should work across HTML/SVG boundary (but currently doesn't due to serialization)

### 4. Namespace Awareness
SVG elements live in the SVG namespace (`http://www.w3.org/2000/svg`):
- Attribute lookups must specify namespace
- Element creation requires correct namespace

## Key Takeaways for SVG Implementation

### Current Problems (Root Causes)

1. **CSS Inheritance Failure**: SVG serialization to data URL breaks the CSS inheritance chain. The `<text>` element inside `<svg>` can't inherit `fill: green` from the parent `<div>` because:
   - SVG becomes a data URL → treated as an image → no access to parent's CSS
   - Serialized SVG doesn't include computed styles from parent elements

2. **Web Fonts Failure**: `@import` in SVG `<style>` doesn't work because:
   - Font loading happens in document context
   - Data URLs create a separate, isolated context
   - Font requests from data URL may be blocked or not processed

3. **Crisp Transforms Failure**: CSS transforms on SVG become blurry because:
   - SVG rasterized at original size via `resvg`
   - Resulting bitmap scaled, causing pixelation
   - Vector nature lost during serialization → rasterization pipeline

### Architectural Implications

1. **Serialization is a Hack**: The current approach treats SVG as external content rather than integrated document content.

2. **Layout Integration is Minimal**: SVG doesn't participate in normal layout flow; it's a black box replaced element.

3. **Style Separation**: SVG styling isolated from document CSS system.

4. **Performance Overhead**: Serialization, base64 encoding, data URL parsing, and rasterization on every change.

### What a Proper Implementation Needs

1. **Direct Fragment Tree Integration**: SVG should generate fragments directly, not through image pipeline.

2. **CSS Integration**: SVG elements should participate in CSS cascade and inheritance.

3. **Vector Rendering Pipeline**: Transforms should apply to vector data, not rasterized bitmaps.

4. **Font System Integration**: SVG should use the same font loading as HTML content.

5. **Modular Engine**: Like taffy for layout, SVG needs its own modular rendering engine that integrates with Servo's display list.

## Next Steps

After understanding the DOM layer (Phase 1), proceed to:

1. **Phase 2**: Layout integration - How SVG data flows to layout system
2. **Phase 3**: Serialization pipeline - Full trace from DOM to rasterization
3. **Phase 4**: Taffy pattern study - Modular architecture reference
4. **Phase 5**: Issue-specific analysis - Root cause investigation for each test failure

The DOM layer shows why the current approach fails: by serializing SVG to data URLs, we lose all integration with the document's style system, font system, and vector rendering capabilities.