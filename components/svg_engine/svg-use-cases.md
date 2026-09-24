# SVG usage contexts

Every place an SVG document or fragment can appear or be referenced in a web
browser, and how each one is treated. The contexts split into five categories.

| Category | How the SVG is treated | Servo pipeline today |
|---|---|---|
| 1. Document mode | A full document — scripts, interaction, and external resources are **allowed** | full document loader |
| 2. Image mode | **Secure static / animated** — scripts disabled, external resources blocked (only `data:` inlined) | resvg |
| 3. Inline | Part of the host document's DOM — shares its CSS and scripts | svg_engine |
| 4. External fragment | A fragment of another SVG file, pulled in by reference | not native yet |
| 5. Web API | Depends on the API used | varies |

---

## 1. Document mode — full SVG

The SVG **is** the document (or an embedded document). Scripts run, user
interaction works, and external resources (nested images, fonts, stylesheets)
are **allowed**.

**Main example:**

```html
<!-- 1. Top-level navigation — the user browses directly to the .svg URL.
        Served with Content-Type: image/svg+xml, the SVG IS the document. -->

<!-- 2. <iframe> — embeds the SVG as a full, separate document -->
<iframe src="logo.svg"></iframe>

<!-- 3. <object> — embeds the SVG as a full document -->
<object data="logo.svg" type="image/svg+xml"></object>

<!-- 4. <embed> — embeds the SVG as a full document -->
<embed src="logo.svg" type="image/svg+xml">
```

---

## 2. Image mode — SVG as an image

The SVG is used as a **static/animated image** ("secure static mode" / "secure
animated mode"): scripts are disabled, and external resources (images,
stylesheets) cannot be loaded — only `data:` URLs are inlined.

**Main example:**

```html
<!-- 5. <img> — the classic SVG-as-image -->
<img src="logo.svg" alt="logo">

<!-- 6. <picture> / <source> — responsive image selection (SVG is one candidate) -->
<picture>
  <source srcset="logo.svg" type="image/svg+xml">
  <img src="logo.png" alt="logo">
</picture>

<!-- 7. srcset / sizes on <img> — density/resolution selection (SVG as a variant) -->
<img src="logo.png" srcset="logo.svg 2x" sizes="100vw" alt="logo">

<!-- 8. <input type="image"> — an image submit button using an SVG -->
<input type="image" src="logo.svg" alt="submit">

<!-- 9–13. CSS image values — all four are image contexts, so secure mode applies -->
<style>
  .a { background-image: url("logo.svg"); }    /* 9  background-image */
  .b { list-style-image: url("logo.svg"); }    /* 10 list-style-image */
  .c::before { content: url("logo.svg"); }     /* 11 content (pseudo-element) */
  .d { border-image-source: url("logo.svg"); } /* 12 border-image-source */
  .e { cursor: url("logo.svg"), auto; }        /* 13 cursor */
</style>

<!-- 14. SVG <image> — an image element inside an SVG body referencing an SVG -->
<svg><image href="logo.svg" width="100" height="100"/></svg>

<!-- 15. SVG <feImage> — a filter primitive that loads an SVG as its source -->
<svg>
  <filter id="f"><feImage href="logo.svg"/></filter>
  <rect width="100" height="100" filter="url(#f)"/>
</svg>

<!-- 16. Canvas — drawImage / createImageBitmap (also OffscreenCanvas) -->
<script>
  const img = new Image();
  img.onload = () => ctx.drawImage(img, 0, 0);   // 16a drawImage
  const bmp = await createImageBitmap(img);      // 16b createImageBitmap
</script>

<!-- 17. <link rel="icon"> — an SVG favicon / site icon -->
<link rel="icon" href="logo.svg">
```

---

## 3. Inline — SVG in the host document's DOM

The `<svg>` element is part of the **host document itself**. It shares the host
document's CSS cascade, scripts, and event handling — no isolation.

**Main example:**

```html
<!-- 18. Inline <svg> in an HTML document — inherits host CSS (e.g. currentColor),
        can be scripted, and participates in the host's style cascade -->
<div class="icon">
  <svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
    <circle cx="50" cy="50" r="40" fill="currentColor"/>
  </svg>
</div>

<!-- 19. <svg> in an XML / XHTML document — an SVG-namespace element in an XML
        document, still part of that document's DOM and style system -->
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect width="100" height="100"/>
</svg>
```

---

## 4. External fragment reference — a piece of another SVG

Instead of loading a whole SVG, the document references a **fragment** (an
element or definition) inside another `.svg` file by URL + `#id`.

**Main example:**

```html
<!-- 20. External <use> — clones a <symbol>/element defined in another SVG file -->
<svg>
  <use href="icons.svg#icon-home"/>
</svg>

<!-- 21. url("other.svg#id") — reference a paint server / resource defined in
        another file, via the CSS/SVG url() function -->
<svg>
  <rect width="100" height="100"
        fill="url(icons.svg#gradient)"/>   <!-- fill (gradient/pattern) -->
  <!-- the same url("other.svg#id") form is also used by:
       stroke="url(...#gradient)"   filter="url(...#filter)"
       mask="url(...#mask)"         clip-path="url(...#clip)"
       marker-start/mid/end="url(...#marker)" -->
</svg>
```

---

## 5. Web API — creating SVG at runtime

Scripts load or build SVG content through the DOM and image APIs.

**Main example:**

```js
// 22. DOMParser — parse an SVG string into a full SVG Document (document mode)
const svgDoc = new DOMParser().parseFromString(
  '<svg xmlns="http://www.w3.org/2000/svg"><circle r="10"/></svg>',
  'image/svg+xml',
);

// 23. new Image() / createElement('img') — set .src to an SVG (image mode)
const img = new Image();
img.src = 'logo.svg';
document.body.appendChild(img);

// 24. fetch() — get the raw SVG text, then parse (case 22) or insert inline
const text = await (await fetch('logo.svg')).text();
// → DOMParser.parseFromString(text, 'image/svg+xml')   (document mode)
// → or inject into the host DOM                          (inline mode)
```

---

## Cross-cutting notes

- **`data:image/svg+xml` URLs** can appear in *any* image context (category 2).
  They are the one "inlined" source that stays allowed under secure static mode.
- **Three pipelines, three behaviors:** document mode (1–4) uses the full
  document loader; image mode (5–17) is isolated (currently rasterized by
  resvg); inline + fragment (3, 18–21) go through `svg_engine`.
