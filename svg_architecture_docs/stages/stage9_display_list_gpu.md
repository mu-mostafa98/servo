# Stage 9 — Display List Construction & GPU Rendering

> **Thread:** Layout → WebRender
> **Also known as:** Fragment::Image → WR display item → GPU
> **Key files:**
> - [components/layout/display_list/mod.rs](../../components/layout/display_list/mod.rs)
> - [components/layout/display_list/stacking_context.rs](../../components/layout/display_list/stacking_context.rs)

---

## Overview

Stage 9 is the **final rendering stage**. The `Fragment::Image` produced in Stage 8 is converted into a WebRender display list item, which is then sent to the GPU for rendering.

This is where the SVG finally becomes visible to the user — a `push_image` command tells WebRender to draw the rasterized SVG texture at the correct position on the page.

---

## Sub-stage 9.1 — Display List Building

**File:** [display_list/mod.rs](../../components/layout/display_list/mod.rs)
**Fragment handler:** `Fragment::Image` at line 680

During display list construction, each fragment in the fragment tree is visited. When a `Fragment::Image` is encountered:

```rust
Fragment::Image(image) => {
    let image = image.borrow();
    let style = image.base.style();
    match style.get_inherited_box().visibility {
        Visibility::Visible => {
            let image_rendering = style.get_inherited_box()
                .image_rendering.to_webrender();
            let rect = image.base.rect
                .translate(containing_block.origin.to_vector())
                .to_webrender();
            let clip = image.clip
                .translate(containing_block.origin.to_vector())
                .to_webrender();
            let common = builder.common_properties(clip, &style);

            if let Some(image_key) = image.image_key {
                builder.wr().push_image(
                    &common,
                    rect,
                    image_rendering,
                    wr::AlphaType::PremultipliedAlpha,
                    image_key,                    // ImageKey(IdNamespace(1), 90)
                    wr::ColorF::WHITE,
                );
            }
            // ...
        },
        Visibility::Hidden => (),     // not visible, skip
        Visibility::Collapse => (),    // collapsed, skip
    }
}
```

**Input:**
```rust
Fragment::Image {
    base: BaseFragment {
        rect: PhysicalRect(200px × 200px at (0px, 0px)),
        style: ComputedValues { ... },
        ...
    },
    clip: PhysicalRect(200px × 200px at (0px, 0px)),
    image_key: Some(ImageKey(IdNamespace(1), 90)),
    showing_broken_image_icon: false,
    url: None,
}
```

### Step 1 — Visibility Check

```rust
match style.get_inherited_box().visibility {
    Visibility::Visible => { /* render */ },
    Visibility::Hidden => (),      // image exists but invisible
    Visibility::Collapse => (),     // like hidden for table elements
}
```

### Step 2 — Coordinate Conversion

The fragment's rect is in layout coordinates (relative to the containing block). It's translated to the containing block's origin and converted to WebRender coordinates:

```rust
let rect = image.base.rect
    .translate(containing_block.origin.to_vector())
    .to_webrender();
// → wr::LayoutRect(200×200 at containing_block_position)
```

### Step 3 — Push Image to Display List

```rust
builder.wr().push_image(
    &common,                                        // clip, spatial id, etc.
    rect,                                           // 200×200 at position
    image_rendering,                                // auto → LinearRGB
    wr::AlphaType::PremultipliedAlpha,              // standard alpha
    ImageKey(IdNamespace(1), 90),                   // ← the SVG texture!
    wr::ColorF::WHITE,                              // default color
);
```

**The `push_image` call** is the final output of the entire SVG pipeline — it tells WebRender to draw the SVG texture at the specified position with the specified clipping and rendering properties.

---

## Sub-stage 9.2 — Background Image Path (for SVG via CSS)

**File:** [display_list/mod.rs](../../components/layout/display_list/mod.rs)
**Lines:** 1507-1530

When SVG appears as a CSS background image (via `background-image: url(data:...)`), a different code path handles it:

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
            let scale = builder.device_pixel_ratio.get();
            let default_size: DeviceIntSize =
                Size2D::new(size.width * scale, size.height * scale).to_i32();
            // Request rasterization at the appropriate size
            builder.image_resolver.rasterize_vector_image(
                vector_image.id,
                default_size,
                node,
                vector_image.svg_id.clone(),
            ).and_then(|rasterized_image| rasterized_image.id)
        },
    };
    // → background image drawn with the resulting image_key
}
```

This path handles the case where an SVG data URL is used as a CSS background image (not as a replaced element). It calls `rasterize_vector_image()` to get the WebRender `ImageKey`.

---

## Sub-stage 9.3 — Border Image Path

**File:** [display_list/mod.rs](../../components/layout/display_list/mod.rs)
**Lines:** 1756-1779

Similar path for SVG used in `border-image` CSS property:

```rust
Ok(ResolvedImage::Image { image, size }) => {
    let image_key = match image {
        CachedImage::Raster(raster_image) => raster_image.id,
        CachedImage::Vector(vector_image) => {
            let scale = builder.device_pixel_ratio.get();
            let size = Size2D::new(size.width * scale, size.height * scale).to_i32();
            node.and_then(|node| {
                builder.image_resolver.rasterize_vector_image(
                    vector_image.id, size, node, vector_image.svg_id,
                )
            }).and_then(|rasterized_image| rasterized_image.id)
        },
    };
    // → border image drawn with the resulting image_key
}
```

---

## GPU Rendering (WebRender)

After the display list is built, it's sent to the WebRender renderer thread. WebRender:

1. **Batches** the `push_image` command with other similar commands
2. **Uploads** the texture data associated with `ImageKey(IdNamespace(1), 90)` to the GPU (if not already uploaded in Stage 7)
3. **Renders** the 200×200 RGBA pixels at the correct screen position with the correct clip rect
4. **Composites** the final frame to the screen

The user sees a **blue circle** rendered at the SVG's position on the page.

---

## Data Flow

```
Fragment Tree
    │
    ▼
Display List Builder
    │
    ▼
Fragment::Image { image_key: Some(ImageKey(1, 90)) }
    │
    ├── visibility check → Visible
    ├── coordinate translation → WR rect
    ├── clip computation → WR clip
    └── push_image(ImageKey(1, 90), rect, clip, ...)
         │
         ▼
    WebRender Display List
    [PushImage(ImageKey(1,90), rect=[x,y,200,200], ...)]
         │
         ▼
    WebRender Renderer (GPU thread)
         │
         ├── Batch with other images
         ├── Upload texture to GPU (if needed)
         ├── Render with shader
         └── Composite to screen
              │
              ▼
    Blue circle visible on screen!
```

## Debugging Summary

### Breakpoint Table

| # | What | File:Line | Watch For |
|---|------|-----------|-----------|
| 9.1-i | Fragment::Image | [display_list/mod.rs:680](../../components/layout/display_list/mod.rs#L680) | `image_key`, `rect` |
| 9.1-ii | push_image call | [display_list/mod.rs:699](../../components/layout/display_list/mod.rs#L699) | `ImageKey`, `rect`, `AlphaType` |
| 9.2 | Background SVG | [display_list/mod.rs:1509](../../components/layout/display_list/mod.rs#L1509) | `CachedImage::Vector` arm |
| 9.3 | Border SVG | [display_list/mod.rs:1758](../../components/layout/display_list/mod.rs#L1758) | `CachedImage::Vector` arm |

### Trace Output

```
[SVG_TRACE_STAGE_9] DisplayList build Fragment::Image rect=Rect(200pxx200px at (0px, 0px)) image_key=Some(ImageKey(IdNamespace(1), 90))
[SVG_TRACE_STAGE_9] DisplayList background SVG image size=200x200
[SVG_TRACE_STAGE_9] DisplayList border-image SVG image size=200x200
```

### Key Variables

| Variable | Type | Meaning | Value |
|----------|------|---------|-------|
| `image.image_key` | `Option<WebRenderImageKey>` | The GPU texture handle | `Some(ImageKey(IdNamespace(1), 90))` |
| `image.base.rect` | `PhysicalRect<Au>` | Layout position and size | `200px × 200px at (0px, 0px)` |
| `image.clip` | `PhysicalRect<Au>` | Clipping region | `200px × 200px at (0px, 0px)` |
| `image_rendering` | `wr::ImageRendering` | GPU sampling mode | `Auto` |
| `image.showing_broken_image_icon` | `bool` | Fallback icon? | `false` |

### Debugging Checklist

When the SVG doesn't appear on screen, check:
1. Is `Fragment::Image` being hit? (Stage 9.1 breakpoint)
2. Is `image_key` `Some(...)` or `None`? If None, Stage 7 hasn't completed
3. Is the rect within the viewport? `image.base.rect` position relative to scroll
4. Is `visibility` == `Visible`? Check `style.get_inherited_box().visibility`
5. Is the display list being sent to WebRender? Check for WR thread panics
6. Does the texture exist on GPU? Check `ImageKey` matches what was uploaded in Stage 7

---

## Full Pipeline Summary (All 9 Stages)

```
HTML Parser                         Image Cache / Network           GPU
    │                                     │                         │
    ▼                                     ▼                         ▼
┌──────────┐  ┌──────────┐  ┌─────────────────────┐  ┌──────────────┐
│ Stage 1  │→ │ Stage 2  │→ │ Stage 3: Queue      │  │ Stage 9      │
│ DOM      │  │ Style &  │  │ Stage 4: Serialize  │→ │ Display List │
│ Creation │  │ Dispatch │  │ Stage 5: Parse SVG  │  │ → GPU Render │
│          │  │          │  │ Stage 6: Rasterize  │  │              │
│          │  │          │  │ Stage 7: WR Key     │  │              │
└──────────┘  └──────────┘  └─────────────────────┘  └──────────────┘
                                  │
                                  ▼
                            ┌──────────┐
                            │ Stage 8  │←┘
                            │ Fragment │
                            │ Constr.  │
                            └──────────┘

Pass 1: Stages 1→2→3 (source=None → queue serialization)
Pass 2: Stage 4 (serialize XML → data URL)
Pass 3: Stages 2→8 (source=Some(url), image not cached → empty fragment)
Pass 4: Stages 5→2→6→7→8→9 (full pipeline → visible SVG)
```
