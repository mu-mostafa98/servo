# Servo Onboarding Guide

> **Generated from the knowledge graph** — 2,519 files analyzed, 20,045 relationships mapped.
>
> *Based on commit `d268ba212ea42493bfea5fc8e51a8de6faa4a298`*

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Architecture Layers](#architecture-layers)
3. [Key Concepts](#key-concepts)
4. [Guided Tour](#guided-tour)
5. [Component Map](#component-map)
6. [Complexity Hotspots](#complexity-hotspots)
7. [Getting Started](#getting-started)

---

## Project Overview

**Servo** is a prototype web browser engine written in **Rust**, developed with a focus on **parallelism**, **safety**, and **modern web standards**.

| Attribute | Details |
|---|---|
| **Language** | Rust (primary), with Python (build/CI), WebIDL (DOM bindings), JavaScript (tests), HTML/CSS |
| **Frameworks** | Custom browser engine, WebGPU, WebGL, WebXR, GStreamer |
| **Build System** | Cargo + Mach (Python-based build orchestration) |
| **Key Dependencies** | SpiderMonkey (JS engine), WebRender (GPU rendering), Stylo (CSS, via Firefox), HarfBuzz (text shaping), FreeType (fonts) |
| **Platforms** | Windows, macOS, Linux, Android, OpenHarmony (OhOS) |
| **Architecture** | Multi-process, message-passing (IPC), parallel layout |

### What Makes Servo Unique

- **Parallel layout engine** — Servo's layout can process independent layout trees in parallel
- **Rust safety** — memory safety without a garbage collector (except for JS via SpiderMonkey)
- **Modular design** — clear component boundaries communicating via IPC channels
- **Embeds WebRender** — Servo uses Mozilla's GPU-based renderer

---

## Architecture Layers

The codebase is organized into 21 layers, each representing a distinct domain.

### 1. DOM Layer (`components/script/dom/`) — 729 files
**The largest layer.** Implements all WebIDL interfaces — HTML elements, SVG, CSSOM, events, canvas, WebGL, WebGPU, audio, fetch, streams, and more. Every `<div>`, `<canvas>`, event, and API call in a web page maps to a Rust struct here.

**Key pattern:** Each DOM type follows a standard pattern:
- `new_inherited()` — allocates the type
- `new_with_proto()` — constructor exposed to JS
- DOM method implementations with `#[dom_union]` / JS binding macros

### 2. Layout Engine (`components/layout/`) — 59 files
The parallel layout engine. Builds flow trees, fragment trees, and display lists. Handles:
- **Block/inline layout** (normal document flow)
- **Flexbox** (`components/layout/flexbox/`)
- **Tables** (`components/layout/table/`)
- **Positioned content** (`components/layout/positioned.rs`)
- **Accessibility tree** (`components/layout/accessibility_tree.rs`)

### 3. Media Playback (`components/media/`) — 49 files
Audio/video decoding via **GStreamer** (Linux/macOS/Windows) and native OhOS backends. Manages decode, playback state, and media element integration.

### 4. Script Engine (`components/script/`) — 54 files (core)
The script thread core: manages the JS runtime (SpiderMonkey), task queues, module loading, document lifecycle, timers, and the event loop. The heart of Servo's JavaScript execution.

### 5. Networking (`components/net/`) — 36 files
HTTP/HTTPS loader, caching, WebSocket, cookie management, HSTS, image caching, and resource fetching. All network communication goes through this layer.

### 6. Script Bindings (`components/script_bindings/`) — 58 files
SpiderMonkey JS engine bindings. Handles DOM-JS reflection, garbage collection tracing, root management, type conversions, and structured cloning. Auto-generated from WebIDL.

### 7. Ports (`ports/`) — 36 files
Platform-specific shells:
- **Desktop** (`ports/servoshell/desktop/`) — winit-based windowing
- **Android** — EGL/JNI integration
- **OhOS** — OpenHarmony support

### 8. Shared Infrastructure (`components/shared/`) — 46 files
Cross-cutting utilities: IPC channels, thread pools, text/rope handling, profiling, embedder bridge types, WebXR/WebGPU shared types.

### 9. Painting (`components/paint/`) — 14 files
Coordinates with **WebRender** to composite content. Manages the paint thread, display list building, pinch-zoom, scrolling, screenshot capture, and the refresh driver.

### 10. Constellation (`components/constellation/`) — 14 files
**The central orchestrator.** Manages:
- **Pipeline lifecycle** — creation, navigation, teardown
- **Browsing contexts** — tabs, iframes
- **Session history** — back/forward navigation
- **Process management** — sandboxing, service workers

### 11-21. Additional Layers

| Layer | Files | Purpose |
|---|---|---|
| **Fonts** | 19 | Font loading, HarfBuzz shaping, FreeType platform integration |
| **Canvas 2D** | 7 | Vello-based 2D canvas rendering (CPU + GPU backends) |
| **WebXR** | 25 | VR/AR session management, OpenXR integration |
| **Storage** | 10 | IndexedDB + WebStorage (SQLite-backed) |
| **WebDriver** | 8 | Remote control protocol for browser automation |
| **DevTools** | 6 | Firefox DevTools protocol integration |
| **XPath** | 8 | XPath parser, AST, tokenizer, evaluation |
| **Hang Monitor** | 6 | Detects frozen threads via platform samplers |
| **FFI/C-API** | 7 | C-compatible API for embedding Servo natively |
| **Tooling** | 66 | Mach commands, servo-tidy, CI test harnesses |

---

## Key Concepts

### 1. The Pipeline Architecture

Servo renders a web page through a **pipeline** — a set of threads that process a document:

```
[Constellation] → [Script Thread] → [Layout Thread] → [Paint Thread] → [WebRender]
```

- **Constellation** creates and manages pipelines (one per document/frame)
- **Script Thread** runs JS and builds/modifies the DOM
- **Layout Thread** computes layout from the DOM + style
- **Paint Thread** converts layout into display lists for WebRender
- All communicate via **IPC channels** (message passing)

### 2. DOM Binding Pattern

DOM types follow a consistent pattern in `components/script/dom/bindings/`:

```rust
// 1. DOMReflector — base type with JS object tracking
// 2. Root — garbage-collected root for DOM objects
// 3. Trace — JS GC tracing integration
// 4. Conversions — JS↔Rust type conversion
```

Each DOM element:
- Extends a base class (e.g. `HTMLElement` → `Element` → `Node`)
- Implements WebIDL-specified methods
- Uses `#[dom_struct]` for GC integration

### 3. Parallel Layout

Servo's layout engine can process layout **in parallel**:
- **Flow tree** — represents the formatting structure (block, inline, flex, table)
- **Fragment tree** — the result of layout (positioned boxes)
- **Display list** — ordered list of rendering commands for WebRender
- Layout operations are dispatched across thread pools where possible

### 4. IPC-Based Communication

Components communicate through **typed IPC channels** (not shared memory):
- Each component defines message types (enums)
- Messages are sent via channels and dispatched in event loops
- `components/shared/` contains the shared message type definitions
- This design enables multi-process mode (process-per-tab)

### 5. Build System (Mach)

The build is orchestrated by **Mach** (`python/servo/`):
- `mach build` — compile the engine
- `mach run` — launch Servo
- `mach test` — run tests
- `mach tidy` — run servo-tidy linter
- Bootstrap downloads toolchains and dependencies

---

## Guided Tour

Follow this path to understand Servo's architecture, from entry point to rendering.

### Step 1: Platform Shell — `ports/servoshell/`

Start here. The desktop shell (`ports/servoshell/desktop/app.rs`) creates the window, initializes the event loop, and launches Servo. The CLI entry is `ports/servoshell/desktop/cli.rs`.

### Step 2: Servo Crate — `components/servo/`

The top-level crate (`components/servo/servo.rs`) creates the **embedder**, manages **webviews**, and wires together the constellation, script, and paint threads through **proxies** (`components/servo/proxies.rs`).

### Step 3: Constellation — `components/constellation/`

The brain of the engine. `constellation.rs` is the main event loop — it creates pipelines for each page/frame, handles navigation (back/forward/refresh), manages browsing contexts, and routes messages between components.

### Step 4: Script Thread — `components/script/`

`script_thread.rs` runs the JavaScript engine (SpiderMonkey via `components/script_bindings/`). It:
- Parses HTML and builds the DOM
- Executes JavaScript
- Manages task queues and event dispatch
- Communicates layout changes to the layout thread

### Step 5: DOM Implementation — `components/script/dom/`

The bulk of the code. Every HTML element, SVG element, event type, and Web API lives here. Explore `components/script/dom/html/` for HTML elements, `components/script/dom/svg/` for SVG, and `components/script/dom/event/` for events.

### Step 6: Layout Engine — `components/layout/`

The layout crate processes the DOM + computed styles and produces positioned fragments. Key files:
- `lib.rs` — entry point
- `flow/` — flow construction from DOM
- `fragment_tree/` — generates fragments (positioned boxes)
- `display_list/` — builds WebRender-compatible display lists
- `flexbox/` and `table/` — specific layout modes
- `construct_modern.rs` — modern flow construction

### Step 7: Painting — `components/paint/`

The paint thread receives display lists from layout and sends them to WebRender. `paint.rs` manages the main paint loop, `webrender_external_images.rs` handles image data, and `webview_renderer.rs` manages rendering per webview.

### Step 8: Networking — `components/net/`

The resource thread (`resource_thread.rs`) handles all HTTP/HTTPS, WebSocket, and data fetching. `http_loader.rs` is the core HTTP implementation, `http_cache.rs` handles caching, and `websocket_loader.rs` handles WS/WSS.

### Step 9: Media & Canvas — `components/media/` + `components/canvas/`

Media playback uses GStreamer. Canvas 2D rendering uses the **Vello** backend with both CPU and GPU paths (`vello_backend.rs`, `vello_cpu_backend.rs`).

### Step 10: Platform-Specific Code — `ports/`

The final layer — platform shells wrap Servo for each target:
- **Desktop** — winit-based, with GL context management
- **Android** — JNI bridging to Java/Android APIs
- **OhOS** — OpenHarmony platform bindings

---

## Component Map

### Core Components (in dependency order)

| Crate | Path | Files | Purpose |
|---|---|---|---|
| **shared** | `components/shared/` | 46 | Shared types, IPC messages, thread pool, profiling |
| **script_bindings** | `components/script_bindings/` | 58 | SpiderMonkey JS bindings, GC, DOM reflection |
| **script** | `components/script/` | 800+ | Script thread, DOM implementation, JS execution |
| **net** | `components/net/` | 36 | HTTP/HTTPS/WebSocket networking |
| **layout** | `components/layout/` | 59 | Parallel layout engine |
| **fonts** | `components/fonts/` | 19 | Font loading and shaping |
| **paint** | `components/paint/` | 14 | WebRender integration |
| **media** | `components/media/` | 49 | Audio/video playback |
| **canvas** | `components/canvas/` | 7 | Canvas 2D rendering |
| **constellation** | `components/constellation/` | 14 | Pipeline orchestration |
| **webxr** | `components/webxr/` | 25 | VR/AR support |
| **devtools** | `components/devtools/` | 6 | Firefox DevTools protocol |
| **storage** | `components/storage/` | 10 | IndexedDB + WebStorage |
| **servo** | `components/servo/` | 14 | Top-level crate, embedder bridge |
| **webdriver** | `components/webdriver_server/` | 8 | WebDriver automation |
| **xpath** | `components/xpath/` | 8 | XPath evaluation |
| **hang_monitor** | `components/background_hang_monitor/` | 6 | Hang detection |

### Supporting Files

| Path | Purpose |
|---|---|
| `python/servo/` | Mach build commands, bootstrapping, CI automation |
| `ports/servoshell/` | Platform shells (desktop, Android, OhOS) |
| `etc/` | CI scenarios, configuration |
| `ffi/capi/` | C FFI embedding API |
| `tests/` | Web Platform Tests (WPT) integration |

---

## Complexity Hotspots

These areas have high complexity — approach with care:

### 🔴 Most Complex (Dense Logic)

| Component | Why |
|---|---|
| **`AbortSignal`** (`abortsignal.rs`) | Dependent signals, abort algorithms, timeout, throw-if-aborted semantics |
| **`CanvasRenderingContext`** (`canvas_context.rs`) | Multiple context types, WebRender integration, state management |
| **`ImageBitmap`** (`imagebitmap.rs`) | Crop-and-transform, source creation, serialization, transfer |
| **`OffscreenCanvas`** (`offscreencanvas.rs`) | Multi-context support (2D, WebGL, WebGL2, BitmapRenderer) |
| **`CharacterData`** (`characterdata.rs`) | DOM text manipulation — append, insert, delete, replace, substring |
| **`Clipboard API`** (`clipboard.rs`) | System clipboard integration via IPC |
| **`CookieStore`** (`cookiestore.rs`) | Cookie get/set/delete with IPC-based cookie jar communication |
| **`CustomElementRegistry`** (`customelementregistry.rs`) | Lifecycle callbacks, element upgrade, observed attributes |

### 🟡 Moderate (Significant Surface Area)

| Component | Why |
|---|---|
| **`EventTarget`** (`event/eventtarget.rs`) | Event dispatch, listener management, bubbling/capturing |
| **`Fetch/Request/Response`** | Full HTTP fetch implementation with body streaming |
| **`HTMLInputElement`** variants | 20+ input type implementations (text, checkbox, color, date, etc.) |
| **`TextDecoder/TextEncoder`** | Encoding state management, streaming, BOM handling |
| **`WebGL` context** | Large API surface, extension management, validation |

### ⚠️ Areas Requiring Cross-Component Understanding

- **Layout → Paint → WebRender pipeline** — data flows across 3 layers
- **Constellation → Script pipeline lifecycle** — navigation, iframes, process management
- **DOM → Layout → Accessibility tree** — accessibility is computed during layout
- **CSS → Stylo → Layout → Paint** — CSS properties flow through the full rendering pipeline

---

## Getting Started

### Building Servo

```bash
# Install dependencies
mach bootstrap

# Build
mach build

# Run
mach run https://example.com

# Run headless
mach run --headless https://example.com
```

### Development Workflow

```bash
# Run a specific test
mach test-wpt dom/events/

# Run the tidy linter
mach tidy

# Build with release optimizations
mach build --release

# Profile
mach run --profile https://example.com
```

### Key Development Tips

1. **WebIDL first** — DOM APIs are defined in `.webidl` files. The bindings are auto-generated. To add a new feature, start with the WebIDL spec.
2. **IPC message flow** — To understand a feature, trace its IPC messages: `components/shared/` defines the message types, and each component's event loop dispatches them.
3. **Test in WPT** — Servo uses the Web Platform Tests (WPT) suite. Tests live in `tests/wpt/`.
4. **Run with `RUST_LOG`** — `RUST_LOG=debug mach run` shows detailed component activity.
5. **Use the DevTools** — Start the DevTools server and connect Firefox DevTools for debugging.

### Resources

- **Architecture docs**: `docs/`
- **WebIDL specs**: `components/script/dom/webidls/`
- **IPC message types**: `components/shared/`
- **Build configuration**: Python files in `python/servo/`
- **CI configuration**: `etc/ci/`

---

*This guide was generated from the Servo knowledge graph. For the interactive version, run `/understand-dashboard`.*
