# Known Limitations of Inline SVG Support

> Current approach: serialize SVG DOM subtree → base64 data URL → load and rasterize as external image.
> This works for simple standalone SVGs but fundamentally breaks the cases below.

---

## Case 1 — CSS doesn't cross the SVG boundary

### Test case

```html
<div style="fill: green">
    <svg width="200" height="20">
        <text y="18">Text should be green</text>
    </svg>
</div>
```

### Explanation

CSS properties set on an ancestor of `<svg>` are not inherited by elements inside the SVG. In the test case above, `fill: green` on the `<div>` should propagate to the `<text>` element, but it doesn't. This also breaks Tailwind CSS and any other framework that relies on CSS inheritance or utility classes crossing into SVG content.

### Root cause

Serializing the SVG subtree into a standalone data URL severs the DOM relationship between the parent document and the SVG content. The serialized XML is parsed independently by `usvg`, which has no knowledge of the parent document's CSS cascade. Styles that were computed by Stylo are lost because only raw DOM attributes survive serialization — computed values do not.

### Status

Fundamental limitation of the serialize-as-image architecture. Requires a native SVG engine that keeps the subtree in the document tree and applies the parent cascade.

---

## Case 2 — Web fonts can't be used

### Test case

```html
<div>
    <svg width="200" height="20">
        <style>
        @import url('https://fonts.googleapis.com/css2?family=Saira+Stencil:ital,wght@0,100..900;1,100..900&display=swap');
        .web-font { font-family: "Saira Stencil"; }
        </style>
        <text class="web-font" y="18">Web font should be used</text>
    </svg>
</div>
```

### Explanation

A web font imported inside the SVG's `<style>` element is never loaded. The text falls back to the default font even when a valid `@import` URL is provided.

### Root cause

The serialized SVG is loaded as an external image via the Image Cache pipeline. The image loader decodes the SVG with `usvg`, which parses XML structure but does not fetch external resources like web fonts. Even if the font URL were fetched, there is no mechanism to apply the font during rasterization since `resvg` operates on a standalone `usvg::Tree` with no access to the browser's font system.

### Status

Fundamental limitation. A native SVG engine would need to integrate with Servo's font loading and resource fetch infrastructure.

---

## Case 3 — Pixel-level error when SVG is transformed

### Test case

```html
<div style="width: fit-content; transform: scale(4) translate(50%, 50%)">
    Circle should be crisp<br>
    <svg width="10" height="10">
        <circle cx="5" cy="5" r="5" fill="green"></circle>
    </svg>
</div>
```

### Explanation

When the SVG is scaled via CSS `transform`, the rendered output becomes blurry or pixelated instead of remaining crisp. A 10×10 SVG scaled 4× should render as a sharp 40×40 shape, but the serialization pipeline rasterizes at a fixed intermediate resolution, losing detail.

### Root cause

The SVG is rasterized to a `tiny_skia::Pixmap` at the fixed size determined by `width`/`height` attributes (10×10 in this case). This fixed-resolution pixmap is then stored in the cache and served to the display list as an `ImageKey`. When WebRender scales the resulting texture to 40×40 on the GPU, it must interpolate the 10×10 rasterization, producing visible pixelation. There is no mechanism to re-rasterize at the final display resolution after the transform is known.

### Status

Partial workaround possible (re-rasterize at target resolution after layout), but fundamentally constrained by the image-cache-based architecture where the rasterized bitmap is produced before the final render transform is available.

---

## Case 4 — Animations don't work

### Test case

```html
<svg width="200" height="200">
    <circle cx="100" cy="100" r="50" fill="blue">
        <animate attributeName="r" from="50" to="100" dur="2s" repeatCount="indefinite"/>
    </circle>
</svg>
```

### Explanation

SVG animation elements (`<animate>`, `<set>`, `<animateTransform>`, `<animateMotion>`) and CSS animations inside SVG have no effect. The rendered output is static.

### Root cause

The SVG is serialized once and rasterized as a single static frame. There is no tick loop, interpolation engine, or frame scheduling for SVG animations. Even if the SVG were re-rendered each frame, the current pipeline would need to re-serialize, re-base64, re-fetch, re-parse, and re-rasterize on every animation frame — which would be prohibitively expensive.

### Status

Requires a native SVG engine with its own animation clock integrated into Servo's rendering update cycle.

---

## Case 5 — `<foreignObject>` doesn't work

### Test case

```html
<svg width="400" height="200">
    <foreignObject x="50" y="50" width="300" height="100">
        <div xmlns="http://www.w3.org/1999/xhtml">
            <p>HTML content inside SVG</p>
        </div>
    </foreignObject>
</svg>
```

### Explanation

The `<foreignObject>` element, which allows HTML content to be embedded inside SVG, is silently ignored. The HTML content never appears.

### Root cause

`usvg` does not support `<foreignObject>` — it strips or ignores the element during XML parsing. Even if `usvg` preserved it, `resvg` cannot render arbitrary HTML. The serialize-as-image architecture has no mechanism to composite HTML layout output inside an SVG rasterization.

### Status

Requires a native SVG engine with hybrid SVG/HTML compositing support.

---

## Case 6 — Scripting inside SVG doesn't work

### Test case

```html
<svg width="200" height="200">
    <circle cx="100" cy="100" r="50" fill="blue">
        <script type="text/javascript">
            // SVG event handlers and DOM manipulation
        </script>
    </circle>
</svg>
```

### Explanation

`<script>` elements inside SVG and inline event handlers (e.g. `onclick`, `onmouseover`) are not executed.

### Root cause

The serialized SVG is loaded via the Image Cache, which decodes image data — it does not execute scripts. Even with a native SVG engine, scripting would require a dedicated JS execution context and event dispatch system integrated with the SVG subtree.

### Status

Requires a native SVG engine with scripting support.

---

## Root Cause Summary

All six cases stem from the same architectural decision:

```
SVG DOM subtree → XML serialize → base64 encode → data URL → Image Cache → usvg parse → resvg rasterize → ImageKey → Fragment::Image
```

Each case represents a symptom of the information loss at the serialization boundary:

| Case | What is lost | At which step |
|------|-------------|---------------|
| CSS inheritance | Computed styles (Stylo cascade) | XML serialization — only raw DOM attributes survive |
| Web fonts | Resource fetch + font system integration | `usvg` parse — no external resource loading |
| Transform pixelation | Final render resolution | Rasterization — fixed resolution before layout knows the target size |
| Animations | Animation state + tick loop | Entire pipeline — each frame would require full re-serialization |
| `<foreignObject>` | HTML subtree + layout | `usvg` parse — element is stripped |
| Scripting | JS engine + event system | Image Cache — never executes code |

A proper fix requires replacing the serialize-as-image approach with a native SVG engine that keeps the SVG subtree as part of the document tree, applies the full Stylo CSS cascade inside the SVG, and produces fragment trees / display lists directly rather than through an image intermediate.

---

> See also: [svg_engine_design.md](svg_engine_design.md) — architecture proposal for a native SVG engine replacement.
