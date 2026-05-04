# SVG DOM Classes and Methods Detailed Documentation

## Overview
This document provides a comprehensive breakdown of all classes and methods in the SVG DOM implementation in Servo, located in `components/script/dom/svg/`. Each file, class, and method is documented with its purpose, inputs, and outputs.

## File Structure
```
components/script/dom/svg/
├── mod.rs              # Module exports
├── svgelement.rs       # Base SVG element class
├── svggraphicselement.rs # SVG graphics element base class
├── svgimageelement.rs  # SVG <image> element
└── svgsvgelement.rs    # Root <svg> element (most complex)
```

---

## File: mod.rs

**Purpose:** Module declaration file that exports all SVG DOM element modules.

**Exports:**
- `pub(crate) mod svgelement;`
- `pub(crate) mod svggraphicselement;`
- `pub(crate) mod svgimageelement;`
- `pub(crate) mod svgsvgelement;`

**No classes or methods defined in this file.**

---

## File: svgelement.rs

### Class: `SVGElement`

**Purpose:** Base class for all SVG elements. Inherits from `Element` and provides SVG-specific functionality like style handling, focus management, and attribute parsing.

**Inheritance Chain:** `Node` ← `Element` ← `SVGElement`

**Fields:**
- `element: Element` - Base element instance
- `style_decl: MutNullableDom<CSSStyleDeclaration>` - Cached style declaration object

### Methods

#### Constructor Methods

**`fn new_inherited(tag_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGElement`**
- **Purpose:** Creates a new SVGElement with default element state
- **Inputs:**
  - `tag_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGElement` instance
- **Calls:** `new_inherited_with_state(ElementState::empty(), ...)`

**`pub(crate) fn new_inherited_with_state(state: ElementState, tag_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGElement`**
- **Purpose:** Creates a new SVGElement with specified element state
- **Inputs:**
  - `state`: Initial element state flags
  - `tag_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGElement` instance
- **Note:** Always uses SVG namespace (`ns!(svg)`)

**`pub(crate) fn new(cx: &mut JSContext, tag_name: LocalName, prefix: Option<Prefix>, document: &Document, proto: Option<HandleObject>) -> DomRoot<SVGElement>`**
- **Purpose:** Creates and reflects a new SVGElement to JavaScript
- **Inputs:**
  - `cx`: JavaScript context
  - `tag_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
  - `proto`: Optional prototype object
- **Output:** Rooted DOM reference to new SVGElement
- **Uses:** `Node::reflect_node_with_proto()`

#### Utility Methods

**`fn as_element(&self) -> &Element`**
- **Purpose:** Returns reference to base `Element`
- **Inputs:** None (self)
- **Output:** `&Element` reference
- **Uses:** `self.upcast::<Element>()`

### Virtual Method Implementations

#### `impl VirtualMethods for SVGElement`

**`fn super_type(&self) -> Option<&dyn VirtualMethods>`**
- **Purpose:** Returns parent virtual methods implementation
- **Output:** `Some(&Element as &dyn VirtualMethods)`

**`fn attribute_mutated(&self, cx: &mut JSContext, attr: &Attr, mutation: AttributeMutation)`**
- **Purpose:** Handles attribute mutation, specifically for `nonce` attribute
- **Inputs:**
  - `cx`: JavaScript context
  - `attr`: The attribute that was mutated
  - `mutation`: Type of mutation (Set or Removed)
- **Behavior:**
  - Calls parent's `attribute_mutated`
  - If attribute is `nonce`, updates element's internal nonce slot
  - On Set: stores nonce value
  - On Removed: clears nonce value (sets empty string)

### WebIDL Binding Methods (`SVGElementMethods`)

#### `impl SVGElementMethods<crate::DomTypeHolder> for SVGElement`

**`fn Style(&self) -> DomRoot<CSSStyleDeclaration>`**
- **Purpose:** Returns the `style` attribute object (getter for `element.style`)
- **Spec:** <https://html.spec.whatwg.org/multipage/#the-style-attribute>
- **Output:** CSSStyleDeclaration for this element
- **Implementation:** Lazily creates style declaration if not exists

**`fn Nonce(&self) -> DOMString`**
- **Purpose:** Returns the `nonce` attribute value
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-noncedelement-nonce>
- **Output:** Nonce value as DOMString
- **Uses:** `self.as_element().nonce_value()`

**`fn SetNonce(&self, value: DOMString)`**
- **Purpose:** Sets the `nonce` attribute value
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-noncedelement-nonce>
- **Inputs:** `value` - New nonce value
- **Behavior:** Updates internal nonce slot via `update_nonce_internal_slot()`

**`fn Autofocus(&self) -> bool`**
- **Purpose:** Returns whether element has `autofocus` attribute
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-fe-autofocus>
- **Output:** `true` if element has `autofocus` attribute

**`fn SetAutofocus(&self, autofocus: bool, can_gc: CanGc)`**
- **Purpose:** Sets or removes `autofocus` attribute
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-fe-autofocus>
- **Inputs:**
  - `autofocus`: Whether to set attribute
  - `can_gc`: GC permission context
- **Uses:** `set_bool_attribute()` with `local_name!("autofocus")`

**`fn Focus(&self, cx: &mut JSContext, options: &FocusOptions)`**
- **Purpose:** Focuses the element
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-focus>
- **Inputs:**
  - `cx`: JavaScript context
  - `options`: Focus options (preventScroll, etc.)
- **Behavior:**
  1. Runs focusing steps via `run_the_focusing_steps()`
  2. If focusing successful and `preventScroll` is false, scrolls element into view
  3. Uses `ScrollLogicalPosition::Center` for scrolling

**`fn Blur(&self, cx: &mut JSContext)`**
- **Purpose:** Removes focus from the element
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-blur>
- **Inputs:** `cx`: JavaScript context
- **Behavior:**
  1. Checks if element is currently focused
  2. Calls document's focus handler to focus viewport instead
  3. Uses `FocusOperation::Focus(FocusableArea::Viewport)`

**`fn TabIndex(&self) -> i32`**
- **Purpose:** Returns `tabindex` attribute value
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-tabindex>
- **Output:** tabindex value as i32 (defaults to -1 if not set)

**`fn SetTabIndex(&self, tab_index: i32, can_gc: CanGc)`**
- **Purpose:** Sets `tabindex` attribute value
- **Spec:** <https://html.spec.whatwg.org/multipage/#dom-tabindex>
- **Inputs:**
  - `tab_index`: New tabindex value
  - `can_gc`: GC permission context
- **Uses:** `set_int_attribute()` with `local_name!("tabindex")`

**Macro: `global_event_handlers!()`**
- **Purpose:** Implements global event handler properties (onclick, onload, etc.)
- **Location:** Line 121 in the file

---

## File: svggraphicselement.rs

### Class: `SVGGraphicsElement`

**Purpose:** Base class for all SVG elements that have graphics capabilities (can be rendered). Inherits from `SVGElement` and serves as parent for elements like `<rect>`, `<circle>`, `<text>`, etc.

**Inheritance Chain:** `Node` ← `Element` ← `SVGElement` ← `SVGGraphicsElement`

**Fields:**
- `svgelement: SVGElement` - Base SVG element instance

### Methods

#### Constructor Methods

**`pub(crate) fn new_inherited(tag_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGGraphicsElement`**
- **Purpose:** Creates a new SVGGraphicsElement with default element state
- **Inputs:**
  - `tag_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGGraphicsElement` instance
- **Calls:** `new_inherited_with_state(ElementState::empty(), ...)`

**`pub(crate) fn new_inherited_with_state(state: ElementState, tag_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGGraphicsElement`**
- **Purpose:** Creates a new SVGGraphicsElement with specified element state
- **Inputs:**
  - `state`: Initial element state flags
  - `tag_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGGraphicsElement` instance
- **Implementation:** Delegates to `SVGElement::new_inherited_with_state()`

### Virtual Method Implementations

#### `impl VirtualMethods for SVGGraphicsElement`

**`fn super_type(&self) -> Option<&dyn VirtualMethods>`**
- **Purpose:** Returns parent virtual methods implementation
- **Output:** `Some(&SVGElement as &dyn VirtualMethods)`
- **Uses:** `self.upcast::<SVGElement>()`

**Note:** `SVGGraphicsElement` doesn't override other virtual methods; it relies on `SVGElement` implementations.

---

## File: svgimageelement.rs

### Class: `SVGImageElement`

**Purpose:** Implements the SVG `<image>` element for embedding images in SVG documents. Can reference external images (PNG, JPEG, etc.) or other SVGs.

**Inheritance Chain:** `Node` ← `Element` ← `SVGElement` ← `SVGGraphicsElement` ← `SVGImageElement`

**Fields:**
- `svggraphicselement: SVGGraphicsElement` - Base graphics element instance

**Constants:**
- `DEFAULT_WIDTH: u32 = 300` - Default width if not specified
- `DEFAULT_HEIGHT: u32 = 150` - Default height if not specified

### Methods

#### Constructor Methods

**`fn new_inherited(local_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGImageElement`**
- **Purpose:** Creates a new SVGImageElement
- **Inputs:**
  - `local_name`: The local name of the element (should be `image`)
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGImageElement` instance
- **Implementation:** Delegates to `SVGGraphicsElement::new_inherited()`

**`pub(crate) fn new(cx: &mut JSContext, local_name: LocalName, prefix: Option<Prefix>, document: &Document, proto: Option<HandleObject>) -> DomRoot<SVGImageElement>`**
- **Purpose:** Creates and reflects a new SVGImageElement to JavaScript
- **Inputs:**
  - `cx`: JavaScript context
  - `local_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
  - `proto`: Optional prototype object
- **Output:** Rooted DOM reference to new SVGImageElement
- **Uses:** `Node::reflect_node_with_proto()`

#### Resource Handling Methods

**`fn fetch_image_resource(&self)`**
- **Purpose:** Initiates fetching of the image resource referenced by `href` attribute
- **Spec:** <https://svgwg.org/svg2-draft/linking.html#processingURL>
- **Behavior:**
  - Currently only queues an `error` event (TODO: implement actual fetching)
  - Uses DOM manipulation task source to queue event
  - Called when `href` attribute is set

### Virtual Method Implementations

#### `impl VirtualMethods for SVGImageElement`

**`fn super_type(&self) -> Option<&dyn VirtualMethods>`**
- **Purpose:** Returns parent virtual methods implementation
- **Output:** `Some(&SVGGraphicsElement as &dyn VirtualMethods)`
- **Uses:** `self.upcast::<SVGGraphicsElement>()`

**`fn attribute_mutated(&self, cx: &mut JSContext, attr: &Attr, mutation: AttributeMutation)`**
- **Purpose:** Handles attribute mutation, specifically for `href` attribute
- **Inputs:**
  - `cx`: JavaScript context
  - `attr`: The attribute that was mutated
  - `mutation`: Type of mutation (Set or Removed)
- **Behavior:**
  - Calls parent's `attribute_mutated`
  - If attribute is `href` (in SVG or XLink namespace) and is being Set, calls `fetch_image_resource()`

**`fn attribute_affects_presentational_hints(&self, attr: &Attr) -> bool`**
- **Purpose:** Determines if attribute affects presentation (layout/rendering)
- **Inputs:** `attr`: Attribute to check
- **Output:** `true` if attribute affects presentation
- **Behavior:**
  - Returns `true` for `width` and `height` attributes
  - Delegates to parent for other attributes

**`fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue`**
- **Purpose:** Parses plain (non-namespaced) attributes
- **Inputs:**
  - `name`: Attribute name
  - `value`: Attribute value string
- **Output:** Parsed `AttrValue`
- **Behavior:**
  - For `width`/`height`: Uses `AttrValue::from_u32()` with defaults (300x150)
  - Delegates to parent for other attributes

---

## File: svgsvgelement.rs

### Class: `SVGSVGElement`

**Purpose:** Implements the root `<svg>` element. This is the most complex SVG element with serialization, caching, and dimension handling. Responsible for the "hacky" SVG implementation via data URL serialization.

**Inheritance Chain:** `Node` ← `Element` ← `SVGElement` ← `SVGGraphicsElement` ← `SVGSVGElement`

**Fields:**
- `svggraphicselement: SVGGraphicsElement` - Base graphics element instance
- `uuid: String` - Unique identifier for caching rasterized images
- `cached_serialized_data_url: DomRefCell<Option<Result<ServoUrl, ()>>>` - Cached serialized SVG as data URL

### Methods

#### Constructor Methods

**`fn new_inherited(local_name: LocalName, prefix: Option<Prefix>, document: &Document) -> SVGSVGElement`**
- **Purpose:** Creates a new SVGSVGElement
- **Inputs:**
  - `local_name`: The local name of the element (should be `svg`)
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
- **Output:** New `SVGSVGElement` instance
- **Behavior:**
  - Generates new UUID for element
  - Initializes empty cached data URL

**`pub(crate) fn new(cx: &mut JSContext, local_name: LocalName, prefix: Option<Prefix>, document: &Document, proto: Option<HandleObject>) -> DomRoot<SVGSVGElement>`**
- **Purpose:** Creates and reflects a new SVGSVGElement to JavaScript
- **Inputs:**
  - `cx`: JavaScript context
  - `local_name`: The local name of the element
  - `prefix`: Optional namespace prefix
  - `document`: Owner document
  - `proto`: Optional prototype object
- **Output:** Rooted DOM reference to new SVGSVGElement
- **Uses:** `Node::reflect_node_with_proto()`

#### Serialization and Caching Methods

**`pub(crate) fn serialize_and_cache_subtree(&self)`**
- **Purpose:** Serializes the SVG subtree to XML, encodes as base64, and caches as data URL
- **TODO:** <https://github.com/servo/servo/issues/43142>
- **Process:**
  1. Creates temporary JavaScript context
  2. Processes `<use>` elements (clones referenced elements)
  3. Serializes XML via `xml_serialize(TraversalScope::IncludeNode)`
  4. Cleans up cloned nodes
  5. Base64 encodes XML: `data:image/svg+xml;base64,...`
  6. Parses as `ServoUrl` and caches result
- **Error Handling:** On serialization error, caches `Err(())`
- **This is the core of the current "hacky" SVG implementation**

**`fn process_use_elements(&self, cx: &mut JSContext) -> Vec<DomRoot<Node>>`**
- **Purpose:** Processes all `<use>` elements in the SVG subtree for serialization
- **Inputs:** `cx`: JavaScript context
- **Output:** Vector of cloned nodes that were inserted
- **Behavior:**
  - Traverses subtree preorder
  - For each `<use>` element, calls `process_single_use_element()`
  - Returns list of cloned nodes for cleanup

**`fn process_single_use_element(&self, cx: &mut JSContext, use_element: &Element) -> Option<DomRoot<Node>>`**
- **Purpose:** Processes a single `<use>` element by cloning its referenced element
- **Inputs:**
  - `cx`: JavaScript context
  - `use_element`: The `<use>` element to process
- **Output:** Cloned node if successful, `None` otherwise
- **Behavior:**
  1. Extracts `href` attribute (strips `#` prefix)
  2. Finds referenced element by ID via `document.GetElementById()`
  3. Checks if referenced element has SVG ancestor
  4. Clones referenced element and children
  5. Appends clone to SVG root for serialization
- **Note:** Cloned nodes are removed after serialization

**`fn cleanup_cloned_nodes(&self, cx: &mut JSContext, cloned_nodes: &[DomRoot<Node>])`**
- **Purpose:** Removes cloned nodes inserted for `<use>` element serialization
- **Inputs:**
  - `cx`: JavaScript context
  - `cloned_nodes`: Nodes to remove
- **Behavior:** Removes each cloned node from SVG root

**`fn invalidate_cached_serialized_subtree(&self)`**
- **Purpose:** Invalidates the cached serialized data URL
- **Behavior:**
  - Sets `cached_serialized_data_url` to `None`
  - Marks node as dirty with `NodeDamage::Other`
- **Called when:** Attributes change, children change, or element removed from tree

#### Layout Data Methods

**`impl<'dom> LayoutDom<'dom, SVGSVGElement>`**

**`pub(crate) fn data(self) -> SVGElementData<'dom>`**
- **Purpose:** Extracts layout data for this SVG element
- **Output:** `SVGElementData` struct containing:
  - `source`: Cached data URL (or `None` if not serialized)
  - `width`, `height`, `view_box`: Dimension attributes
  - `svg_id`: UUID for caching
- **Note:** Uses `unsafe` to borrow cached data for layout thread

### Virtual Method Implementations

#### `impl VirtualMethods for SVGSVGElement`

**`fn super_type(&self) -> Option<&dyn VirtualMethods>`**
- **Purpose:** Returns parent virtual methods implementation
- **Output:** `Some(&SVGGraphicsElement as &dyn VirtualMethods)`
- **Uses:** `self.upcast::<SVGGraphicsElement>()`

**`fn attribute_mutated(&self, cx: &mut JSContext, attr: &Attr, mutation: AttributeMutation)`**
- **Purpose:** Handles attribute mutation
- **Inputs:**
  - `cx`: JavaScript context
  - `attr`: The attribute that was mutated
  - `mutation`: Type of mutation
- **Behavior:**
  - Calls parent's `attribute_mutated`
  - Invalidates cached serialized subtree

**`fn attribute_affects_presentational_hints(&self, attr: &Attr) -> bool`**
- **Purpose:** Determines if attribute affects presentation
- **Inputs:** `attr`: Attribute to check
- **Output:** `true` if attribute affects presentation
- **Behavior:**
  - Returns `true` for `width` and `height` attributes
  - Delegates to parent for other attributes

**`fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue`**
- **Purpose:** Parses plain attributes, specifically `width` and `height`
- **Inputs:**
  - `name`: Attribute name
  - `value`: Attribute value string
- **Output:** Parsed `AttrValue`
- **Behavior:**
  - For `width`/`height`: Parses as `LengthPercentage` with quirks mode allowed
  - Creates `ParserContext` with document URL and quirks mode
  - Uses `LengthPercentage::parse_quirky()`
  - Returns `AttrValue::LengthPercentage` with original string and parsed value
  - Delegates to parent for other attributes

**`fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation)`**
- **Purpose:** Handles child node additions/removals
- **Inputs:**
  - `cx`: JavaScript context
  - `mutation`: Description of child change
- **Behavior:**
  - Calls parent's `children_changed`
  - Invalidates cached serialized subtree

**`fn unbind_from_tree(&self, context: &UnbindContext<'_>, can_gc: CanGc)`**
- **Purpose:** Cleans up when element is removed from tree
- **Inputs:**
  - `context`: Unbinding context
  - `can_gc`: GC permission
- **Behavior:**
  1. Calls parent's `unbind_from_tree`
  2. Evicts rasterized image from cache using UUID
  3. If cached data URL exists, removes from layout cache and image cache
  4. Invalidates cached serialized subtree

---

## Key Patterns and Observations

### 1. Inheritance Hierarchy
```
Node
├── Element
│   ├── SVGElement
│   │   ├── SVGGraphicsElement
│   │   │   ├── SVGSVGElement
│   │   │   └── SVGImageElement
│   │   └── (Other SVG elements would inherit here)
```

### 2. Virtual Method Delegation Pattern
All SVG elements follow the same pattern:
- Override `super_type()` to return parent implementation
- Override virtual methods, call parent first, then handle SVG-specific logic
- Common overrides: `attribute_mutated`, `attribute_affects_presentational_hints`, `parse_plain_attribute`

### 3. Style Handling
- `SVGElement` has `style_decl` field for CSS style declaration
- `Style()` getter lazily creates `CSSStyleDeclaration`
- Presentation attributes (like `fill="green"`) map to CSS properties

### 4. Serialization Architecture (The "Hacky" Part)
**Current Flow:**
1. `SVGSVGElement` serializes subtree to XML
2. Base64 encodes to data URL: `data:image/svg+xml;base64,...`
3. Caches URL in `cached_serialized_data_url`
4. Layout reads URL via `SVGElementData::source`
5. Image cache loads URL, rasterizes with `resvg`
6. Result treated as `ReplacedContentKind::SVGElement`

**Problems with this approach:**
- Breaks CSS inheritance (serialized SVG loses parent styles)
- Web fonts don't load (data URL context isolated from document)
- Transforms cause blurriness (vector → raster → scaled)

### 5. Attribute Parsing Specializations
- `SVGSVGElement`: `width`/`height` as `LengthPercentage`
- `SVGImageElement`: `width`/`height` as `u32` with defaults
- `SVGElement`: `nonce` attribute handling

### 6. Resource Handling
- `SVGImageElement`: `fetch_image_resource()` (currently stubbed)
- `SVGSVGElement`: Image caching via UUID and data URL

### 7. Focus and Interaction
- `SVGElement` implements `focus()`, `blur()`, `tabIndex`
- Uses HTML specification behavior
- Supports `autofocus` attribute

### 8. Namespace Awareness
All SVG elements are created with `ns!(svg)` namespace. Attribute lookups must check both SVG and XLink namespaces for `href`.

---

## Relationship to Current SVG Issues

### Issue 1: CSS Inheritance Failure (`css_inheritance.html`)
**Root Cause:** `SVGSVGElement.serialize_and_cache_subtree()` serializes raw XML without computed styles. The `<text>` element's `fill` property should inherit from parent `<div style="fill: green">` but doesn't because:
- Serialization happens before CSS inheritance is applied across HTML/SVG boundary
- Data URL context has no access to parent document's CSS cascade

### Issue 2: Web Fonts Failure (`web_fonts.html`)
**Root Cause:** `@import` in SVG `<style>` doesn't work in data URL context:
- Font loading uses document's URL context
- Data URLs create isolated context with no document base URL
- Font requests from data URLs may be blocked or not processed

### Issue 3: Crisp Transforms Failure (`crisp_transforms.html`)
**Root Cause:** SVG rasterized at original size, then bitmap scaled:
- `resvg` rasterizes SVG at intrinsic size
- Resulting bitmap scaled by CSS transform
- Vector scaling information lost in serialization → rasterization pipeline

---

## Next Steps for Architecture Improvement

Based on this analysis, a proper SVG implementation would need:

1. **Direct Fragment Integration**: SVG should generate fragments directly, not through image pipeline
2. **CSS Integration**: SVG elements should participate in CSS cascade
3. **Vector Rendering Pipeline**: Transforms should apply to vector data
4. **Font System Integration**: SVG should use same font loading as HTML
5. **Modular Engine**: Like taffy for layout, SVG needs modular rendering engine

This detailed understanding of the DOM layer provides the foundation for designing the improved SVG architecture.