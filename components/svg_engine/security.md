## SVG rendering pipeline & security layers

```mermaid
flowchart TD
    IN(["SVG document"]) -->|"SVG text (XML)"| SEC1["<b>XML input validation</b>"]
    SEC1 -->|"no"| WARN1["⚠ block & log"]
    SEC1 -->|"yes"| PARSE

    PARSE["<b>1. Parse</b> — script thread<br/>xml5ever — text → DOM tree"]
    PARSE -->|"DOM tree"| SEC2["<b>CSS injection guard</b>"]
    SEC2 -->|"no"| WARN2["⚠ block & log"]
    SEC2 -->|"yes"| STYLE

    STYLE["<b>2. Style</b> — layout thread<br/>Stylo — CSS cascade → computed styles"]
    STYLE -->|"styled elements"| SEC4["<b>Expansion & geometry limits</b>"]
    SEC4 -->|"no"| WARN4["⚠ block & log"]
    SEC4 -->|"yes"| BUILD

    BUILD["<b>4. Build</b> — layout thread<br/>layout::svg — build svgRenderTree"]
    BUILD -->|"SVG render tree"| SEC5["<b>Render safety guard</b>"]
    SEC5 -->|"no"| WARN5["⚠ block & log"]
    SEC5 -->|"yes"| RENDER

    RENDER["<b>5. Render</b> — layout thread<br/>svg_engine"]
    RENDER -->|"display list commands"| BACKEND

    BACKEND["<b>6. Render Service Backend</b>"]

    STYLE -->|"font / CSS request"| SEC3["<b>URL allowlist</b>"]
    BUILD -->|"image request"| SEC3
    SEC3 -->|"yes"| FETCH
    SEC3 -->|"no"| WARN3["⚠ block & log"]

    FETCH["<b>3. Fetch</b> — net thread<br/>net — load images, fonts, stylesheets"]
    FETCH -->|"stylesheets, fonts"| STYLE
    FETCH -->|"decoded images"| BUILD

    classDef stage fill:#dbeafe,stroke:#93c5fd,color:#1e3a8a;
    classDef guard fill:#93c5fd,stroke:#2563eb,color:#172554;
    classDef warn fill:#fecaca,stroke:#ef4444,color:#7f1d1d;

    class PARSE,STYLE,BUILD,RENDER,BACKEND,FETCH stage;
    class SEC1,SEC2,SEC3,SEC4,SEC5 guard;
    class WARN1,WARN2,WARN3,WARN4,WARN5 warn;
```

- **XML input validation** (Stage 1 — Parse) — blocks XXE and entity-expansion
bombs, and limits nesting depth and element count during tokenization.
- **CSS injection guard** (Stage 2 — Style) — blocks injected SVG `<style>`
from reading host-document data via attribute selectors + `url()` .
- **URL allowlist** (Stage 3 — Fetch) — rejects untrusted schemes/origins
(SSRF via `<image>`/`@import`/`@font-face`, `file://`, tracking) before any
request leaves the process; audits the image-decode FFI.
- **Expansion & geometry limits** (Stage 4 — Build) — limits `<use>` fan-out
and extreme `viewBox`, blur, path, and stroke values before the render tree
is built.
- **Render safety guard** (Stage 5 — Render) — validates geometry and text and
limits self-referencing patterns before drawing, so crafted input can't
crash the renderer or execute code.


---