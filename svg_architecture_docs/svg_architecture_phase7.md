# SVG Architecture Study - Phase 7: resvg/usvg Integration & Future Architecture

## Overview

This document covers the integration of the `resvg` and `usvg` libraries in Servo's SVG rendering pipeline and presents the architectural changes needed for proper SVG support. The current approach relies entirely on `resvg` for SVG parsing and rasterization, treating SVG as an opaque image format. A proper implementation would integrate SVG directly into Servo's native layout and rendering systems.

## Part 1: resvg/usvg Integration

### Library Roles

| Library | Crate | Role |
|---------|-------|------|
| **usvg** | `resvg::usvg` | SVG XML parser, builds a typed tree representation |
| **resvg** | `resvg` | Renders `usvg::Tree` to `tiny_skia::Pixmap` |
| **tiny_skia** | `resvg::tiny_skia` | 2D raster graphics library, pixmap + transform |

### Dependency Flow

```
Cargo.toml:
    resvg = { workspace = true }  (single dependency pulls all three)

usvg::Tree ──► resvg::render() ──► tiny_skia::Pixmap
```

### SVG Parsing: parse_svg_document_in_memory()

**Location**: [image_cache.rs:67](components/net/image_cache.rs:67)

```rust
fn parse_svg_document_in_memory(
    bytes: &[u8],
    fontdb: Arc<fontdb::Database>,
) -> Result<usvg::Tree, &'static str>
```

**Configuration**:
```rust
let opt = usvg::Options {
    image_href_resolver: usvg::ImageHrefResolver {
        resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: image_string_href_resolver,  // Returns None (block local files)
    },
    fontdb,
    ..usvg::Options::default()
};

usvg::Tree::from_data(bytes, &opt)
```

**Key Restrictions**:
- `<image href="local-file.svg">` is blocked (returns `None`)
- `<image href="data:...">` works (default data resolver)
- External HTTP URLs in `<image>` are not fetched

### SVG Rendering: resvg::render()

**Location**: [image_cache.rs:1035](components/net/image_cache.rs:1035)

```rust
resvg::render(&vector_image.svg_tree, transform, &mut pixmap.as_mut());
```

**Parameters**:
- `&usvg::Tree`: The parsed SVG tree
- `tiny_skia::Transform`: Scale transform from natural size to requested size
- `&mut tiny_skia::Pixmap`: Target pixel buffer

**Transform Calculation**:
```rust
let natural_size = vector_image.svg_tree.size().to_int_size();
let transform = tiny_skia::Transform::from_scale(
    tinyskia_requested_size.width() as f32 / natural_size.width() as f32,
    tinyskia_requested_size.height() as f32 / natural_size.height() as f32,
);
```

### usvg Tree Structure

The `usvg::Tree` is a lightweight SVG representation with these node types:

```
usvg::Node (enum)
    ├── Group     — <g>, <svg> (structural containers)
    │     ├── id
    │     ├── children: Vec<Node>
    │     ├── filters
    │     └── clip_path
    ├── Path      — <path>, <circle>, <rect>, <polygon>, etc.
    │     ├── id
    │     ├── data: Path (bezier segments)
    │     ├── fill: Option<Fill>
    │     └── stroke: Option<Stroke>
    ├── Image     — <image> (embedded raster images)
    │     ├── id
    │     ├── view_box
    │     └── data: ImageData (embedded/raster)
    └── Text      — <text>, <tspan>
          ├── id
          ├── text: String
          ├── font
          ├── fill
          └── stroke
```

**Tree Resources** (stored alongside root):
- `linear_gradients`: Linear gradient definitions
- `radial_gradients`: Radial gradient definitions
- `patterns`: Pattern definitions
- `clip_paths`: Clip path definitions
- `masks`: Mask definitions
- `filters`: Filter definitions
- `fontdb`: Font database

### Font Database Integration

**Initialization** (ImageCacheFactoryImpl::new):
```rust
let mut fontdb = fontdb::Database::new();
fontdb.load_system_fonts();
let fontdb: Arc<fontdb::Database> = Arc::new(fontdb);
```

**Sharing**: The same `Arc<fontdb::Database>` is shared across all `ImageCache` instances in the process, loaded once during `ImageCacheFactoryImpl::new()`.

**Memory Reporting**:
```rust
// image_cache.rs:804
let fontdb_size = self.fontdb.conditional_size_of(ops);
vec![
    Report { path: path![prefix, "image-cache", "fontdb"], kind: ReportKind::ExplicitSystemHeapSize, size: fontdb_size },
]
```

### usvg Memory Sizing

Servo implements `MallocSizeOf` for usvg types in [malloc_size_of/lib.rs:880-1060](components/malloc_size_of/lib.rs):

- **usvg::Tree**: Root node + gradients + patterns + clip paths + masks + filters + fontdb
- **usvg::Node**: Recursive tree traversal (Group, Path, Image, Text)
- **usvg::Group**: Children + filters + clip_path
- **usvg::Path**: Path data + fill + stroke
- **usvg::ClipPath**: Clip tree

## Part 2: Future Architecture Proposal

### Problem Summary

The seven phases of study reveal a fundamental architectural issue: **SVG is treated as a replaced element (like an `<img>`) rather than as native document content**. This creates three cascading failures:

1. **CSS Inheritance**: Serialization isolates SVG from parent document styles
2. **Web Fonts**: Data URL context can't load fonts
3. **Crisp Transforms**: Bitmap rasterization loses vector data

### Proposed Architecture: Native SVG Layout Module

#### Overview

Replace the serialization pipeline with a native SVG layout module (similar to how `taffy` handles flexbox/grid). SVG elements would participate directly in the layout and rendering pipeline.

```
Current:
    SVG DOM → Serialize → data URL → Image Cache → rasterize → Bitmap → WebRender

Proposed:
    SVG DOM → SVG Layout Module → Vector Display Items → WebRender
                     │
            CSS Cascade directly applied
```

#### Components Needed

**1. SVG Fragment Types**

New SVG-specific fragment types replacing `Fragment::Image`:

```rust
enum SVGFragment {
    Shape {
        path: Vec<PathCommand>,
        fill: Option<SVGPaint>,
        stroke: Option<SVGStroke>,
    },
    Text {
        content: String,
        font: FontResource,
        fill: Option<SVGPaint>,
    },
    Image {
        href: ServoUrl,
        view_box: Option<ViewBox>,
    },
    Group {
        children: Vec<SVGFragment>,
        transform: Option<Transform>,
        clip_path: Option<ClipPath>,
    },
}
```

**2. Direct CSS Cascade Integration**

SVG elements would participate in the full CSS cascade:
- `fill`, `stroke` properties inherited from HTML ancestors
- Presentational attributes mapped to CSS
- `@font-face` declarations shared with HTML document
- No serialization boundary

**3. Vector Display List Items**

New WebRender display list items for vector graphics:
- `PushPath(path, fill, stroke)` — draw a filled/stroked path
- `PushText(content, font, paint)` — render SVG text
- `PushGroup(transform, clip)` — transformed group with clipping

These would allow WebRender to render SVG at any resolution without rasterization artifacts.

**4. SVG Layout Tree (not fragment tree integration)**

Instead of treating SVG as a single replaced element, SVG children would create their own layout tree:

```
Layout Box Tree:
    HTMLDiv (block)
        └── SVGSVGRoot (svg root box)
              ├── SVGPath (path box)
              ├── SVGGroup (g box)
              │     ├── SVGCircle (circle box)
              │     └── SVGText (text box)
              └── SVGUse (use box)
                    └── [referenced content]
```

### Implementation Roadmap

#### Phase A: Break Serialization Dependency

**Goal**: Allow SVG to render without serialization.

1. **Create `SVGFragment` enum** in fragment tree
2. **Add `Fragment::SVG(SVGFragment)` variant**
3. **Bypass serialization** in `make_fragments()`: instead of creating `ImageFragment`, create `SVGFragment`
4. **Implement basic rendering**: Write display list items that can render SVG paths directly

**Files to modify**:
- `components/layout/fragment_tree/fragment.rs` — New fragment type
- `components/layout/replaced.rs` — Replace `make_fragments()` SVG branch
- `components/layout/display_list/mod.rs` — Display list generation for SVG fragments

#### Phase B: CSS Integration

**Goal**: SVG elements inherit CSS from HTML ancestors.

1. **Extend layout traversal** to descend into SVG children (currently SVG children are in the DOM but not the layout tree)
2. **Implement presentational attribute → CSS mapping** in script's SVGElement
3. **Ensure CSS cascade** flows through `<svg>` to its children

**Files to modify**:
- `components/layout/flow/construct.rs` — SVG flow construction
- `components/script/dom/svg/svgelement.rs` — Presentational attribute mapping
- `components/shared/layout/lib.rs` — Extend `SVGElementData`

#### Phase C: Web Fonts

**Goal**: SVG text elements use document fonts.

1. **Share font database** between HTML and SVG (already partially done via `fontdb`)
2. **Ensure `@font-face` declarations** apply to SVG text when SVG is inline (not serialized)
3. **Font loading**: Use document's font loading infrastructure instead of SVG's isolated font context

#### Phase D: Vector Display List

**Goal**: Replace bitmap push_image() with vector display list items.

1. **Define WebRender vector primitives** (or use existing path primitives)
2. **Implement SVG path → WebRender path translation**
3. **Remove SVG rasterization** from image cache (CPU rasterization no longer needed)
4. **Enable crisper transforms**: Transforms applied at display list level, preserving vector data

**Files to modify**:
- `components/layout/display_list/mod.rs` — Vector display list generation
- `components/net/image_cache.rs` — SVG rasterization removal
- WebRender integration layer

### Migration Strategy

The migration can be incremental:

1. **First**, add `SVGFragment` alongside existing `ImageFragment` (selective opt-in)
2. **Gradually**, migrate SVG rendering from rasterized path to vector path
3. **Finally**, remove serialization pipeline entirely for inline SVGs

### Risks and Considerations

| Risk | Mitigation |
|------|------------|
| **Performance**: Native SVG rendering may be slower than pre-rasterized | WebRender GPU paths can outperform CPU rasterization at high scales |
| **Compatibility**: Some SVG features may not render identically | Use usvg as reference; render baseline with resvg as fallback |
| **Scope**: Complete SVG layout engine is a large undertaking | Phased approach with incremental improvements |
| **Text Layout**: SVG text layout differs from HTML | Reuse existing text shaping infrastructure |
| **Animation**: SVG animations (SMIL) not covered | Start with static SVG, add animation later |

### Summary

The seven-phase study reveals that Servo's current SVG implementation has strong foundations in the DOM layer (Phase 1) and cache management (Phase 6), but the serialization pipeline (Phase 3) creates fundamental architectural barriers that cannot be fully resolved within the replaced-element model. A native SVG layout module with direct CSS cascade integration, vector display list items, and shared font infrastructure would resolve all three issues (CSS inheritance, web fonts, crisp transforms) while providing a cleaner, more performant architecture.