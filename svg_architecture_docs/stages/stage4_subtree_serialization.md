# Stage 4 — SVG Subtree Serialization

> **Thread:** Script
> **Also known as:** XML serialization → base64 → data URL
> **Key files:**
> - [components/script/dom/svg/svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)

---

## Overview

Stage 4 converts the SVG element's DOM subtree into a `data:` URL that the image cache can process. This is the **serialization barrier** — the SVG starts as a live DOM tree and becomes a self-contained data URL that can be processed by the standard image pipeline (usvg → rasterization → WebRender).

**Why a data URL?** SVG is a replaced element, and Servo's image pipeline handles replaced content through its image cache. By serializing the SVG to a data URL, the pipeline treats it like any other image resource — parse, cache, rasterize, render.

---

## Sub-stage 4.1 — Serialize & Cache Subtree

**File:** [svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
**Function:** `serialize_and_cache_subtree()`
**Lines:** 79-103

**Called from:** `handle_pending_images_post_reflow()` in Stage 3

### Step 1 — Process `<use>` Elements

```rust
let cloned_nodes = self.process_use_elements(cx);
```

Handles SVG `<use>` elements by cloning referenced elements into the tree before serialization. For a simple SVG without `<use>` elements, this returns an empty Vec.

**Input:** `&self` — the SVGSVGElement

**Output:** `Vec<DomRoot<Node>>` — cloned nodes to clean up after serialization

### Step 2 — XML Serialization

```rust
let serialize_result = self
    .upcast::<Node>()
    .xml_serialize(TraversalScope::IncludeNode);
```

Uses xml5ever's serializer to convert the SVG element and its entire subtree to XML text. `TraversalScope::IncludeNode` means the serialization includes the SVG element itself as the root.

**Input:** The SVG element node (as `&Node`)

**Output:** `Result<String, ()>` — the XML source

For our test SVG:
```xml
<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <circle xmlns="http://www.w3.org/2000/svg" cx="100" cy="100" r="50" fill="blue"></circle>
</svg>
```

**Serialized length:** 231 bytes for our test case.

### Step 3 — Base64 Encoding

```rust
let base64_encoded_source = base64::engine::general_purpose::STANDARD.encode(&xml_source);
let data_url = format!("data:image/svg+xml;base64,{}", base64_encoded_source);
```

Encodes the XML string as base64 and wraps it in a `data:image/svg+xml;base64,` URL.

**Input:** XML string (231 bytes)

**Output:** Base64 data URL (334 characters):
```
data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMDAiIGhlaWdodD0iMjAwIiB2aWV3Qm94PSIwIDAgMjAwIDIwMCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICAgICAgICA8Y2lyY2xlIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgY3g9IjEwMCIgY3k9IjEwMCIgcj0iNTAiIGZpbGw9ImJsdWUiPjwvY2lyY2xlPgogICAgPC9zdmc+
```

### Step 4 — Cache the Data URL

```rust
match ServoUrl::parse(&data_url) {
    Ok(url) => *self.cached_serialized_data_url.borrow_mut() = Some(Ok(url)),
    Err(error) => error!("Unable to parse serialized SVG data url: {error}"),
};
```

Parses the data URL string as a `ServoUrl` and caches it. On success, the SVG's `cached_serialized_data_url` field transitions from `None` to `Some(Ok(url))`.

**Output:** `cached_serialized_data_url` set to `Some(Ok(ServoUrl("data:image/svg+xml;base64,...")))`

### Step 5 — Cleanup

```rust
self.cleanup_cloned_nodes(cx, &cloned_nodes);
```

Removes any `<use>`-cloned nodes that were temporarily inserted for serialization.

---

## Sub-stage 4.2 — Cache Invalidation

**File:** [svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs)
**Function:** `invalidate_cached_serialized_subtree()`
**Lines:** 164-167

When the SVG's DOM subtree changes (attributes modified, children added/removed), the cached data URL must be invalidated:

```rust
fn invalidate_cached_serialized_subtree(&self) {
    *self.cached_serialized_data_url.borrow_mut() = None;
    self.upcast::<Node>().dirty(NodeDamage::Other);
}
```

This is called from:
- `attribute_mutated()` (line 209) — any attribute change
- `children_changed()` (line 259) — child elements added/removed
- `unbind_from_tree()` (line 280) — element removed from DOM

After invalidation, the next layout pass will see `source: None` again and re-queue serialization in Stage 3.

---

## Data Flow

```
handle_pending_images_post_reflow()
           │
           ▼
serialize_and_cache_subtree()
           │
     ┌─────┴─────┐
     │           │
     ▼           ▼
process_use_elements()    xml_serialize()
(for <use> clones)        (SVG → XML text)
     │           │
     └─────┬─────┘
           ▼
    base64::encode(XML)
           │
           ▼
    format!("data:image/svg+xml;base64,{b64}")
           │
           ▼
    ServoUrl::parse(data_url)
           │
           ▼
    cached_serialized_data_url = Some(Ok(url))
           │
           ▼
    node.dirty(NodeDamage::Other)
           │
           ▼
    next layout pass sees source=Some(Ok(url))
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 4.1-i | Entry | [svgsvgelement.rs:79](../../components/script/dom/svg/svgsvgelement.rs#L79) | Function entry |
| 4.1-ii | XML serialization | [svgsvgelement.rs:85](../../components/script/dom/svg/svgsvgelement.rs#L85) | XML result length |
| 4.1-iii | Data URL cache | [svgsvgelement.rs:99](../../components/script/dom/svg/svgsvgelement.rs#L99) | `ServoUrl::parse()` result |
| 4.2 | Invalidation | [svgsvgelement.rs:164](../../components/script/dom/svg/svgsvgelement.rs#L164) | Called on attribute/children change |

### Trace Output

```
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() ENTER
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() processing use elements...
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() xml_serializing subtree...
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() xml_source_len=231
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() data_url_len=334
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() CACHED OK url=data:image/svg+xml;base64,...
[SVG_TRACE_STAGE_4] serialize_and_cache_subtree() EXIT
```

### Key Variables

| Variable | Before Stage 4 | After Stage 4 |
|----------|----------------|---------------|
| `cached_serialized_data_url` | `DomRefCell(None)` | `DomRefCell(Some(Ok(ServoUrl("data:..."))))` |
| Node damage | Clean | `NodeDamage::Other` |
| `svg_data.source` (next layout) | `None` | `Some(Ok(data_url))` |

### Important Notes

- **Only runs once** for each SVG element (unless the subtree is invalidated)
- The data URL is **cached until the SVG subtree changes** (attribute mutation or child changes)
- The cache is invalidated when the element is removed from the DOM (`unbind_from_tree`), also evicting the image from the image cache
- The serialized size (231 bytes for our test) includes the full XML with all namespaces and attributes
