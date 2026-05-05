# Stage 3 — Queue & Serialization Dispatch

> **Thread:** Layout → Script (post-reflow bridge)
> **Also known as:** The layout-to-script handoff for SVG serialization
> **Key files:**
> - [components/layout/context.rs](../../components/layout/context.rs)
> - [components/script/dom/window.rs](../../components/script/dom/window.rs)

---

## Overview

Stage 3 is the **bridge between layout and script**. When Stage 2 detects that the SVG has not been serialized yet (`source: None`), it queues the node for serialization. After the layout pass completes, the script thread processes this queue and triggers the actual serialization (Stage 4).

This is a **synchronous barrier** — it always happens between Pass 1 and Pass 2:

```
Pass 1: Layout detects source=None → queue_svg_element_for_serialization()
                                              ↓
                                   end of layout pass
                                              ↓
                      handle_pending_images_post_reflow() → Stage 4
                                              ↓
                                    dirty flag set on node
                                              ↓
Pass 2: Layout runs again (now source=Some(Ok(url)))
```

---

## Sub-stage 3.1 — Queue SVG Element for Serialization

**File:** [context.rs](../../components/layout/context.rs)
**Function:** `ImageResolver::queue_svg_element_for_serialization()`
**Lines:** 240-248

**Called from:** `svg_kind_size()` in [replaced.rs](../../components/layout/replaced.rs) when `svg_data.source == None`

```rust
pub(crate) fn queue_svg_element_for_serialization(&self, element: ServoLayoutNode<'_>) {
    self.pending_svg_elements_for_serialization
        .lock()
        .push(element.opaque().into())    // → UntrustedNodeAddress
}
```

**Input:** `ServoLayoutNode` — the SVG element whose `source` is `None`

**Output:** `UntrustedNodeAddress` pushed to the `pending_svg_elements_for_serialization` Vec.

This Vec is a `Mutex<Vec<UntrustedNodeAddress>>` stored on `ImageResolver`. It's protected by a mutex because `svg_kind_size()` is called from the layout thread, while the consumer runs on the script thread.

**Breakpoint:** [context.rs:240](../../components/layout/context.rs#L240)
**Watch:** `element.opaque()` — the `OpaqueNode` identifying the SVG element

---

## Sub-stage 3.2 — Post-Reflow Image Handler

**File:** [window.rs](../../components/script/dom/window.rs)
**Function:** `handle_pending_images_post_reflow()`
**Lines:** ~3570-3610

This function runs on the script thread **after each layout pass completes**. It processes all pending image-related work that layout discovered:

```rust
fn handle_pending_images_post_reflow(&self) {
    // 1. Process pending SVG serializations
    for untrusted_node in self.layout().pending_svg_elements_for_serialization() {
        let node = ...;  // resolve UntrustedNodeAddress → DomRoot<Node>
        if let Some(svg) = node.downcast::<SVGSVGElement>() {
            // → Stage 4: serialize_and_cache_subtree()
            svg.serialize_and_cache_subtree();
            // → dirty the node for next reflow
            node.dirty(NodeDamage::Other);
        }
    }

    // 2. Process pending image requests (raster images)
    // 3. Process pending rasterization images (vector images needing render)
    // ...
}
```

**Input:** The `pending_svg_elements_for_serialization` Vec from `ImageResolver` (populated in Stage 3.1)

**Processing steps for each SVG node:**
1. Resolve `UntrustedNodeAddress` back to a safe `DomRoot<Node>` using the node map
2. Downcast to `SVGSVGElement` — if it matches, proceed
3. Call `serialize_and_cache_subtree()` — **triggers Stage 4**
4. Call `node.dirty(NodeDamage::Other)` — marks the node as needing re-layout

**Key output:** After this function runs:
- The SVG's `cached_serialized_data_url` is now `Some(Ok(data_url))` (set in Stage 4)
- The node is dirty, so the next reflow pass will re-traverse it
- In the next layout pass, `svg_data.source` will be `Some(Ok(url))` instead of `None`

**Breakpoint:** [window.rs:3570](../../components/script/dom/window.rs#L3570) (approximate — search for `pending_svg_elements_for_serialization`)
**Watch:** The SVG node before/after serialization:
```rust
// Before:  cached_serialized_data_url = DomRefCell(None)
// After:   cached_serialized_data_url = DomRefCell(Some(Ok(ServoUrl("data:..."))))
```

---

## Data Flow

```
Layout Thread                          Script Thread
─────────────                          ────────────
svg_kind_size()
  └─ source=None
       └─ queue_svg_element_for_serialization(node)
                 │
                 │  (mutex boundary)
                 ▼
       pending_svg_elements_for_serialization: Vec<UntrustedNodeAddress>
                                              │
                           end of layout pass │
                                              ▼
                     handle_pending_images_post_reflow()
                       │
                       ▼
                     resolve node address
                       │
                       ▼
                     downcast to SVGSVGElement
                       │
                       ▼
                     serialize_and_cache_subtree()  → Stage 4
                       │
                       ▼
                     node.dirty(NodeDamage::Other)
                       │
                       ▼
                     next reflow triggered
                       │
              (back to layout) ────→ svg_kind_size()
                                        └─ source=Some(Ok(url))
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 3.1 | Queue node | [context.rs:240](../../components/layout/context.rs#L240) | `element.opaque()` — the SVG's OpaqueNode |
| 3.2 | Post-reflow handler | [window.rs:3570](../../components/script/dom/window.rs#L3570) | The SVG node being processed |

### Key Variables

| Variable | Type | Location | Meaning |
|----------|------|----------|---------|
| `pending_svg_elements_for_serialization` | `Mutex<Vec<UntrustedNodeAddress>>` | `ImageResolver` in [context.rs](../../components/layout/context.rs) | Queue of SVG nodes needing serialization |
| `cached_serialized_data_url` | `DomRefCell<Option<Result<ServoUrl, ()>>>` | `SVGSVGElement` in [svgsvgelement.rs](../../components/script/dom/svg/svgsvgelement.rs) | The serialization result (None → Some after Stage 3.2) |

### Trace Output

```
[SVG_TRACE_STAGE_3] queue_svg_element_for_serialization() node=OpaqueNode(17776695034752)
[SVG_TRACE_STAGE_3] handle_pending_images_post_reflow() processing SVG node, about to serialize
[SVG_TRACE_STAGE_3] handle_pending_images_post_reflow() SVG serialized, dirty flag set → triggers next reflow
```
