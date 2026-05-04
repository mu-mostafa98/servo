# Stage 8 — Fragment Construction

> **Thread:** Layout
> **Also known as:** make_fragments — SVGElement arm
> **Key files:**
> - [components/layout/replaced.rs](../../components/layout/replaced.rs)

---

## Overview

Stage 8 converts the `ReplacedContents` (with its `SVGElement(None/Some(VectorImage))` kind) into **`Fragment::Image`** entries that will be added to the fragment tree and later converted to display list items (Stage 9).

This is the **branching point** where SVG either produces a visible fragment or returns empty:
- `vector_image.is_some=false` → empty `vec![]` (no rendering)
- `vector_image.is_some=true` → `Fragment::Image` with rasterization request

---

## Sub-stage 8.1 — Fragment Entry

**File:** [replaced.rs](../../components/layout/replaced.rs)
**Function:** `make_fragments()`
**Lines:** 524-666

**Called from:** `ReplacedContents::layout()` → `ReplacedLayout::layout()` during the layout phase

```rust
pub fn make_fragments(
    &self,
    layout_context: &LayoutContext,
    style: &ServoArc<ComputedValues>,
    size: PhysicalSize<Au>,
) -> Vec<Fragment> {
    let (object_fit_size, rect) = self.calculate_fragment_rect(style, size);
    let clip = PhysicalRect::new(PhysicalPoint::origin(), size);

    let mut base = BaseFragment::new(self.base_fragment_info, style.clone().into(), rect);
    match &self.kind {
        ReplacedContentKind::Image(image_info) => { /* for <img> tags */ },
        ReplacedContentKind::Video(_) => { /* for <video> */ },
        ReplacedContentKind::IFrame(_) => { /* for <iframe> */ },
        ReplacedContentKind::Canvas(_) => { /* for <canvas> */ },
        ReplacedContentKind::SVGElement(vector_image) => {
            // ← OUR PATH
            ...
        },
        ReplacedContentKind::Audio => vec![],
    }
}
```

**Input:**
```rust
size = PhysicalSize(200px, 200px)   // from layout computation
kind = SVGElement(None)              // Passes 1-3
   or SVGElement(Some(VectorImage{...}))  // Pass 4+
```

**Output:** `Vec<Fragment>` — either empty or containing one `Fragment::Image`

---

## Sub-stage 8.2 — SVGElement Arm (Empty/NONE)

**Lines:** 616-662

```rust
ReplacedContentKind::SVGElement(vector_image) => {
    let Some(vector_image) = vector_image else {
        return vec![];    // ← Passes 1-3 take this path
    };

    // Pass 4+ continues below...
```

**Input:** `vector_image = None`

**Output:** `vec![]` — empty fragment set. The SVG takes up no visual space in the fragment tree.

**Why this happens:** The natural size is still computed and the box takes space, but there are no displayable fragments inside. The layout engine knows the SVG exists and reserves space, but since there's no rasterized image yet, no `Fragment::Image` is produced.

---

## Sub-stage 8.3 — SVGElement Arm (With VectorImage/SOME)

**Lines:** 616-662 (continued)

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
        .and_then(|image| image.id)   // Some(ImageKey(...)) after Stage 7
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

### Step 1 — Set Fragment Rect

The base rectangle is set from the `VectorImage`'s metadata dimensions (200×200). Note the TODO comment — this doesn't account for `viewBox` correctly.

### Step 2 — Compute Raster Size

The rasterization size is computed by scaling the fragment rect by the device pixel ratio:
- At `device_pixel_ratio = 1.0`: raster_size = `200 × 200`

### Step 3 — Request Rasterization & Build Fragment

Calls `rasterize_vector_image()` (Stage 6) to get the rasterized image. The result's `image.id` (an `Option<WebRenderImageKey>`) is used as the `image_key`:

- **Pass 4, first call:** `rasterize_vector_image()` returns `None` (async) → `.and_then(|image| image.id)` returns `None` → `.map(...)` returns `None` → `.into_iter().collect()` → **`vec![]`** (empty!)
- **Pass 4, second call:** `rasterize_vector_image()` returns `Some(RasterImage { id: Some(ImageKey(...)), ... })` → fragment built successfully

**Output (when successful):**
```rust
vec![Fragment::Image(ArcRefCell::new(ImageFragment {
    base: BaseFragment { rect: 200px × 200px, ... },
    clip: clip,
    image_key: Some(ImageKey(IdNamespace(1), 90)),
    showing_broken_image_icon: false,
    url: None,
}))]
```

---

## Data Flow

```
ReplacedContents { kind: SVGElement(vector_image) }
           │
     ┌─────┴─────┐
     │           │
     ▼           ▼
vector_image   vector_image
= None         = Some(VectorImage{...})
     │           │
     ▼           ▼
return vec![]   compute rect size (200×200)
(empty)         │
                ▼
          rasterize_vector_image(id=1, 200×200)
                │
           ┌────┴────┐
           │         │
           ▼         ▼
       returns    returns
       None       Some(RasterImage)
           │         │
           ▼         ▼
       vec![]    image.id = Some(ImageKey(1,90))
                     │
                     ▼
                Fragment::Image { image_key: Some(ImageKey(1,90)) }
                     │
                     ▼
                → Stage 9 (Display List)
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 8.1 | Entry | [replaced.rs:524](../../components/layout/replaced.rs#L524) | `kind` discriminant |
| 8.2 | SVGElement None | [replaced.rs:617](../../components/layout/replaced.rs#L617) | `vector_image.is_some()=false` |
| 8.3-i | SVGElement Some | [replaced.rs:617](../../components/layout/replaced.rs#L617) | `vector_image.is_some()=true` |
| 8.3-ii | Rasterization call | [replaced.rs:643](../../components/layout/replaced.rs#L643) | `rasterize_vector_image()` call |
| 8.3-iii | Fragment creation | [replaced.rs:652](../../components/layout/replaced.rs#L652) | `Fragment::Image` being built |

### Trace Output

```
# Passes 1-3 (no image data):
[SVG_TRACE_STAGE_8] make_fragments() ENTER kind=Discriminant(4) size=200pxx200px
[SVG_TRACE_STAGE_8] make_fragments() SVGElement arm, vector_image.is_some=false

# Pass 4+ (image available):
[SVG_TRACE_STAGE_8] make_fragments() ENTER kind=Discriminant(4) size=200pxx200px
[SVG_TRACE_STAGE_8] make_fragments() SVGElement arm, vector_image.is_some=true
[SVG_TRACE_STAGE_8] make_fragments() SVGElement metadata=200x200
```

### Key Variables

| Variable | Pass 1-3 | Pass 4 (first call) | Pass 4 (second call) |
|----------|----------|---------------------|----------------------|
| `vector_image` | `None` | `Some(VectorImage{...})` | `Some(VectorImage{...})` |
| `rasterize_vector_image()` | N/A | Returns `None` (async) | Returns `Some(RasterImage)` |
| `image_key` | N/A | `None` | `Some(ImageKey(1, 90))` |
| Return value | `vec![]` | `vec![]` | `vec![Fragment::Image{...}]` |

### Important Note

The fragment rect assignment has a known issue (noted in the TODO at line 621):
```rust
// TODO: This is incorrect if the SVG has a viewBox.
base.rect = PhysicalSize::new(vector_image.metadata.width, ...);
```

When the SVG has a `viewBox` that differs from its `width`/`height`, the image cache's natural dimensions (from usvg) already account for the viewBox. But the rect assignment here uses the metadata dimensions directly, which may not account for `viewBox`-based scaling correctly in all cases.
