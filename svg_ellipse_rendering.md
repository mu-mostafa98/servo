# SVG Ellipse Rendering — Approach & Challenges

## How it works

SVG `<ellipse>` is defined by a center point (`cx`, `cy`) and two radii (`rx`, `ry`):

```xml
<ellipse cx="100" cy="80" rx="60" ry="40" fill="red"/>
```

The data flows through four stages:

1. **Extraction** (`extract.rs`) — `extract_ellipse()` reads `cx`, `cy`, `rx`, `ry` from DOM attributes. `cx`/`cy` default to `0` if omitted; `rx`/`ry` are required.
2. **Render tree** (`render_tree.rs`) — The ellipse is stored as `Shape::Ellipse(Ellipse { cx, cy, rx, ry })` inside an `SvgRenderNode`.
3. **Dispatch** (`render.rs`) — `render_dispatch()` matches `Shape::Ellipse` and calls `render_ellipse()`.
4. **Rendering** (`renderers.rs`) — `render_ellipse()` generates the WebRender display list items.

## The core challenge: no native ellipse in WebRender

WebRender's display list API (`wr::DisplayListBuilder`) only supports these drawing primitives:

- `push_rect` — filled rectangle
- `push_image` — bitmap image
- `push_border` — stroked border (supports rounded corners)
- `push_gradient` / `push_radial_gradient` / `push_conic_gradient` — gradient fills

**There is no `push_ellipse` or equivalent.** To render an ellipse we must simulate it.

## Our approach: rounded-rect clip

The technique is to draw a `push_rect` bounded by the ellipse's dimensions, then clip it with a rounded-rect clip whose corner radii match `rx`/`ry`:

```
  ┌──────────────────────┐
  │     Bounding rect    │
  │  ┌────────────────┐  │  ← ComplexClipRegion with
  │  │  (rx, ry)      │  │    BorderRadius = (rx, ry)
  │  │                │  │    on all four corners
  │  │   Ellipse area │  │
  │  └────────────────┘  │
  └──────────────────────┘
```

### Step by step

1. **Compute bounds**: `rect = (cx - rx, cy - ry)` → `(cx + rx, cy + ry)`
2. **Define a rounded clip**: Call `wr.define_clip_rounded_rect()` with a `ComplexClipRegion` where the `rect` equals the ellipse bounds and `radii` = `(rx, ry)` on all four corners.
3. **Chain the clip**: Call `wr.define_clip_chain(parent=current_clip_chain_id, clips=[new_clip_id])` so page-level clipping (e.g. `overflow: hidden`) still applies.
4. **Push the rect**: `wr.push_rect()` with the new clip chain.

WebRender's GPU clip shader turns the rounded-rect clip into a perfect ellipse.

### Code (`renderers.rs`)

```rust
pub fn render_ellipse(ellipse, style, svg_origin, spatial_id, clip_chain_id, wr) {
    if ellipse.rx <= 0.0 || ellipse.ry <= 0.0 { return; }

    let bounds = LayoutRect::from_origin_and_size(
        LayoutPoint::new(svg_origin.x + ellipse.cx - ellipse.rx,
                         svg_origin.y + ellipse.cy - ellipse.ry),
        LayoutSize::new(ellipse.rx * 2.0, ellipse.ry * 2.0),
    );

    if let Some(fill) = &style.fill {
        if let Some(color) = fill.color {
            let clip_id = wr.define_clip_rounded_rect(spatial_id,
                ComplexClipRegion {
                    rect: bounds,
                    radii: BorderRadius {
                        top_left: LayoutSize::new(ellipse.rx, ellipse.ry),
                        top_right: LayoutSize::new(ellipse.rx, ellipse.ry),
                        bottom_left: LayoutSize::new(ellipse.rx, ellipse.ry),
                        bottom_right: LayoutSize::new(ellipse.rx, ellipse.ry),
                    },
                    mode: ClipMode::Clip,
                }
            );
            let ellipse_clip = wr.define_clip_chain(Some(clip_chain_id), [clip_id]);
            let common = CommonItemProperties::new(bounds,
                SpaceAndClipInfo { spatial_id, clip_chain_id: ellipse_clip });
            wr.push_rect(&common, bounds, color);
        }
    }
}
```

## Challenges & limitations

### 1. Per-ellipse clip overhead
Each `<ellipse>` creates a new `ClipId` + `ClipChainId` in the display list. For SVGs with many ellipses this generates more display list items than a dedicated primitive would. Acceptable for phase 1 but worth optimizing later.

### 2. No stroke support yet
The clip approach only handles fills. Stroked ellipses would need a different technique — either:
- A border with thick colored edges and a transparent center
- A more complex clip region subtracting the inner area
Stroke is planned for a future phase.

### 3. Clip chaining complexity
The new clip must be parented to the existing `clip_chain_id` so that ancestor clipping (page overflow, scroll frames, etc.) remains active. WebRender's clip chain API makes this explicit, but it adds boilerplate.

### 4. Circles reuse the same renderer
A `<circle>` with `cx`, `cy`, `r` is extracted separately in `extract.rs`, but both produce an ellipse shape with `rx = ry = r`. We could dispatch `Shape::Circle` to `render_ellipse` as well once the circle extraction and shape are defined.

### 5. No anti-aliasing concerns
WebRender handles clip anti-aliasing internally, so ellipse edges render smoothly at any scale.

## Comparison: old approach vs new engine

| Feature | Old approach (rasterized) | New engine (WebRender) |
|---|---|---|
| Ellipse rendering | Full SVG rasterizer support | Rounded-rect clip simulation |
| CSS from HTML `<style>` | Lost during serialization | Works via stylo |
| Fill inheritance | Lost | Works via stylo cascade |
| Vector quality | Rasterized, blurry at scale | Native GPU, crisp at any zoom |
