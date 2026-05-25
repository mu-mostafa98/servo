# SVG Presentation Attributes — Implementation Summary

## Column Key

| Column | Meaning |
|--------|---------|
| **Stylo (CSS property)** | Was the CSS property definition enabled for Servo in the Stylo? |
| **Presentational hint** | Did we wire the HTML attribute → CSS declaration mapping in `element.rs`? |
| `—` | Already existed prior to this work |
| `Enabled` | Was restricted to `engine = "gecko"` — we removed that restriction |
| `Added` | We added the `svg_presentation_attr!` / `svg_length_attr!` call |
| `Removed` | Was added but later removed (CSS syntax incompatible) |

---

### Fill Family

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `fill` | Enabled | Added | InheritedSVG |
| `fill-opacity` | Enabled | Added | InheritedSVG |
| `fill-rule` | Enabled | Added | InheritedSVG |
| `clip-rule` | Enabled | Added | InheritedSVG |

### Stroke Family

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `stroke` | Enabled | Added | InheritedSVG |
| `stroke-width` | Enabled | Added | InheritedSVG |
| `stroke-opacity` | Enabled | Added | InheritedSVG |
| `stroke-dasharray` | Enabled | Added | InheritedSVG |
| `stroke-dashoffset` | Enabled | Added | InheritedSVG |
| `stroke-linecap` | Enabled | Added | InheritedSVG |
| `stroke-linejoin` | Enabled | Added | InheritedSVG |
| `stroke-miterlimit` | Enabled | Added | InheritedSVG |

### Marker Family

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `marker-start` | Enabled | Added | InheritedSVG |
| `marker-mid` | Enabled | Added | InheritedSVG |
| `marker-end` | Enabled | Added | InheritedSVG |

### Clip

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `clip-path` | Enabled | Added | SVG struct |

### Color Interpolation

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `color-interpolation` | Enabled | Added | InheritedSVG |
| `color-interpolation-filters` | Enabled | Added | InheritedSVG |

### Rendering

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `shape-rendering` | Enabled | Added | InheritedSVG |
| `paint-order` | Enabled | Added | InheritedSVG |

### Text

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `text-anchor` | Enabled | Added | InheritedSVG |

### Path Geometry

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `vector-effect` | Enabled | Added | SVG struct |
| `d` | Enabled | Removed | CSS syntax incompatible with SVG path data; engine reads from DOM directly |

### Filter Primitives

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `flood-color` | Enabled | Added | SVG struct |
| `flood-opacity` | Enabled | Added | SVG struct |
| `lighting-color` | Enabled | Added | SVG struct |

### Gradient Stops

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `stop-color` | Enabled | Added | SVG struct |
| `stop-opacity` | Enabled | Added | SVG struct |

### Masking

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `mask-type` | Enabled | Added | SVG struct |

### General Visual Properties

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `opacity` | — (standard CSS) | Added | Effects struct |
| `color` | — (standard CSS) | Added | InheritedText struct |
| `visibility` | — (standard CSS) | Added | InheritedBox struct |
| `pointer-events` | — (standard CSS) | Added | InheritedUI struct |

### Rendering Hints

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `image-rendering` | — (standard CSS) | Added | InheritedBox struct |
| `text-rendering` | — (standard CSS) | Added | InheritedText struct |

### Text / Font Properties

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `font-family` | — (standard CSS) | Added | Font struct |
| `font-style` | — (standard CSS) | Added | Font struct |
| `font-weight` | — (standard CSS) | Added | Font struct |
| `font-size` | — (standard CSS) | Added (length) | Font struct; bare numbers → `px` |
| `letter-spacing` | — (standard CSS) | Added (length) | InheritedText struct |
| `word-spacing` | — (standard CSS) | Added (length) | InheritedText struct |

### Writing / Bidi Properties

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `direction` | — (standard CSS) | Added | InheritedBox struct |
| `unicode-bidi` | — (standard CSS) | Added | InheritedBox struct |
| `writing-mode` | — (standard CSS) | Added | InheritedBox struct |

### Geometry Lengths

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `cx` | Enabled | Added (length) | SVG struct; bare numbers → `px` |
| `cy` | Enabled | Added (length) | SVG struct |
| `r` | Enabled | Added (length) | SVG struct |
| `rx` | Enabled | Added (length) | SVG struct |
| `ry` | Enabled | Added (length) | SVG struct |
| `x` | Enabled | Added (length) | SVG struct |
| `y` | Enabled | Added (length) | SVG struct |

### Dimensions

| Attribute | Stylo (CSS property) | Presentational Hint | Notes |
|-----------|---------------------|---------------------|-------|
| `width` | — (already existed in `Position` struct) | Added (length) | Used by both SVG and HTML |
| `height` | — (already existed in `Position` struct) | Added (length) | Used by both SVG and HTML |

---

## Architecture & Implementation Details

### Layer 1: CSS Property Enablement (Stylo Fork)

**Objective:** Activate 39 pre-existing but Gecko-restricted SVG CSS property definitions for the Servo build.

**Files modified:**
- `style/properties/longhands.toml` — Removed `engine = "gecko"` from all SVG longhand property definitions, replaced with `servo_restyle_damage = "repaint"` to ensure style changes trigger proper re-rendering
- `style/properties/shorthands.toml` — Enabled `marker`, `mask`, and `mask-position` shorthand groups for Servo
- `style/properties/properties.mako.rs` — Updated `ComputedValues` struct layout to accommodate the newly included style structs

**Technical detail:** Each SVG property in `longhands.toml` was originally annotated with `engine = "gecko"`, which caused the build system's code generator to exclude it from the Servo compilation target. Removing this restriction exposed the full set of SVG CSS longhands (`fill`, `stroke`, `marker-*`, `color-interpolation-*`, geometry properties, filter properties, etc.) to Servo's CSS cascade engine. Adding `servo_restyle_damage = "repaint"` ensured that mutations to these properties trigger the correct damage bits during incremental layout.

**Key decision:** No new CSS properties were defined from scratch — all definitions were sourced from upstream Mozilla Stylo. Our changes are purely enabling and integration work.

---

### Layer 2: Presentational Hints Engine (Servo Script)

**Objective:** Bridge SVG HTML attributes (e.g. `fill="red"`) into Servo's CSS cascade system as `PresHints`-origin declarations.

**File:** `components/script/dom/element/element.rs` — `synthesize_presentational_hints_for_legacy_attributes()`

**Core mechanism — `svg_presentation_attr!` macro:**

```rust
macro_rules! svg_presentation_attr {
    // Standard presentation attribute — direct CSS parse
    ($longhand:ident, $attr:tt) => {
        if let Some(val) = self.get_attr_val_for_layout(&ns!(), &local_name!($attr)) {
            svg_presentation_attr!(@parse $longhand, val);
        }
    };
    // Geometry length attribute — unitless SVG number → CSS length with px
    ($longhand:ident, $attr:tt, length) => {
        if let Some(val) = self.get_attr_val_for_layout(&ns!(), &local_name!($attr)) {
            let val = svg_length_attr_val(val);
            svg_presentation_attr!(@parse $longhand, val);
        }
    };
    // Shared parsing — single source of truth for all attribute types
    (@parse $longhand:ident, $val:expr) => {
        let mut input = ParserInput::new(&*$val);
        let mut parser = Parser::new(&mut input);
        if let Ok(decl) = longhands::$longhand::parse_declared(&context, &mut parser) {
            push(decl);
        }
    };
}
```

**Design decisions:**

1. **Single macro, two arms:** Instead of two separate macros (`svg_presentation_attr!` and `svg_length_attr!`), a unified macro with an optional `length` tag reduces duplication. The shared `@parse` internal arm is the single code path that creates the CSS parser, invokes `parse_declared`, and pushes the resulting declaration.

2. **Unitless length normalization (`svg_length_attr_val`):** SVG permits bare numbers for geometry attributes (`cx="250"`), while CSS `<length>` requires explicit units. The helper function detects pure numeric values and appends `px` before parsing:
   ```rust
   fn svg_length_attr_val(val: &str) -> Cow<'_, str> {
       if val.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && !val.is_empty() {
           Cow::Owned(format!("{}px", val))
       } else {
           Cow::Borrowed(val)
       }
   }
   ```

3. **Cascade origin:** Declarations are injected at `PresHints` origin, which sits below author styles and above UA styles in the CSS cascade. This ensures that `<style>` blocks and inline styles correctly override presentation attributes, matching the SVG 2 specification.

4. **Excluded: `d` attribute:** SVG path data syntax (`M 20,200 L 100,160 Z`) is incompatible with the CSS `d` property (which expects `path("...")` syntax). The `d` attribute is removed from presentational hints — the SVG rendering engine reads it directly from the DOM element via `get_attr_val_for_layout`.

**Re-styling trigger — `attribute_affects_presentational_hints()`:**

**File:** `components/script/dom/svg/svgelement.rs`

The `VirtualMethods` trait implementation returns `true` for all 36 SVG presentation attribute names. This tells Servo's incremental restyle engine that changing any of these attributes on an SVG element requires a full style recomputation (including re-running the presentational hints pipeline). Without this, JavaScript mutations like `element.setAttribute("fill", "blue")` would update the DOM but not trigger a visual update.

**Attribute family grouping:** Both files organize attributes into logical families separated by blank lines and section comments for maintainability:
- Fill family (fill, fill-opacity, fill-rule, clip-rule)
- Stroke family (stroke, stroke-dasharray, stroke-linecap, etc.)
- Marker family (marker-start, marker-mid, marker-end)
- Clip, Color interpolation, Rendering, Text, Path geometry, Filter primitives, Gradient stops, Masking, Geometry lengths, Dimensions

---

### Layer 3: Layout & SVG Rendering Pipeline

**Objective:** Create a debug logging mechanism for the SVG rendering pipeline and prepare for the new SVG engine integration.

**File:** `components/layout/replaced.rs`

**`svg_engine_process()` — Styled subtree walker:**

```
svg_engine_process(node, context)
  └─ svg_engine_process_inner(node, context, depth)
       ├─ Read computed style → 11 style structs per element
       │    (inherited_svg, svg, effects, box, font, position, etc.)
       ├─ Read DOM attributes → geometry data (d, points, etc.)
       ├─ Append structured log entry to svg_engine.log
       └─ Recurse into flat-tree children
```

The logger records two complementary data sources for each element:

| Source | Provides | Example |
|--------|----------|---------|
| Computed style (CSS cascade) | Paint attributes, inherited values | `fill: red`, `stroke-width: 3px` |
| DOM attributes (element) | Raw geometry, path data | `d: "M 20,200 L 100,160 Z"` |

This dual-source logging validates that presentation attributes flow correctly through the CSS cascade, while geometry data remains accessible via the DOM for the rendering engine.

**Data flow for SVG rendering:**

```
HTML/SVG markup
     │
     ▼
Servo HTML Parser (xml5ever)
     │
     ├─ Creates DOM tree with attributes
     ├─ SVG elements → generic SVGElement (no per-type DOM classes)
     │
     ▼
Style System (Stylo)
     │
     ├─ ComputedValues populated by:
     │   ├─ Author styles (<style> blocks)
     │   ├─ Presentational hints (our Layer 2)
     │   └─ UA stylesheet defaults
     │
     ▼
Layout (servo-layout)
     │
     ├─ svg_engine_process() → debug logging
     ├─ SVGSVGElement.serialize_and_cache_subtree()
     │   └─ Full subtree serialized to XML via xml5ever
     │   └─ Base64-encoded → data:image/svg+xml;base64,...
     │
     ▼
Image Cache (servo-net)
     │
     ├─ usvg::Tree::from_data() parses the XML
     │   (reads d, points, cx, cy, etc. from serialized attributes)
     ├─ resvg::render() rasterizes to pixel buffer
     │
     ▼
WebRender display list → GPU compositing
```

---

### Layer 4: Verification & Test Infrastructure

**File:** `svg_architecture_docs/WIKI/svg_presentation_attr_test.html`

A minimal test page with one SVG element per type to verify all 36 presentation attributes + width/height:

| Element | Attributes Tested |
|---------|------------------|
| `<rect>` | fill, fill-opacity, fill-rule, clip-rule, stroke + 7 sub-properties, color-interpolation, color-interpolation-filters, shape-rendering, paint-order, vector-effect, x, y, rx, ry, width, height |
| `<circle>` | cx, cy, r, clip-path |
| `<polyline>` | marker-start, marker-mid, marker-end |
| `<path>` | fill (url gradient), d (DOM-only) |
| `<text>` | text-anchor, x, y |
| `<stop>` (in defs) | stop-color, stop-opacity |
| `<feFlood>` (in defs) | flood-color, flood-opacity |
| `<feDiffuseLighting>` (in defs) | lighting-color |
| `<mask>` (in defs) | mask-type |

**Verification:** `svg_engine.log` confirms every attribute propagates correctly through the pipeline, with both computed style values and DOM attribute values logged per element.
