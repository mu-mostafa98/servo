# SVG Styling Architecture in Servo

> **From Parsing to Styled DOM Tree — Complete Pipeline**

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Dataflow: End-to-End Pipeline](#2-dataflow-end-to-end-pipeline)
3. [Class Hierarchy and Relationships](#3-class-hierarchy-and-relationships)
4. [Component Inputs/Outputs](#4-component-inputsoutputs)
5. [Starting Point: Parsing](#5-starting-point-parsing)
6. [End Point: Styled DOM Tree Ready for Rendering](#6-end-point-styled-dom-tree-ready-for-rendering)
7. [Current Implementation Status](#7-current-implementation-status)

---

## 1. Architecture Overview

Servo's SVG styling pipeline is a multi-stage system spanning 5 major components:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         SVG STYLING PIPELINE                                  │
│                                                                              │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌────────┐ │
│  │ PARSING  │───▶│  DOM     │───▶│  STYLO   │───▶│  LAYOUT  │───▶│ RENDER │ │
│  │ (HTML/   │    │ (Script) │    │ (Style)  │    │ (Box     │    │ (WebR) │ │
│  │  XML)    │    │          │    │          │    │  Tree)   │    │        │ │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘    └────────┘ │
│       │               │               │               │              │      │
│       │               │               │               │              │      │
│  Creates SVG      Stores attr,   Resolves CSS     Builds box      Produces  │
│  DOM nodes        style attr     → computed       tree, damage    display   │
│  via html5ever    & presentation  values using     propagation,    list for │
│  /xml5ever        hints          Stylo engine     fragments       WebRender │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Component Layers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     LAYER 5: RENDERING (WebRender)                       │
│  Display List → GPU rasterization → pixels                               │
├─────────────────────────────────────────────────────────────────────────┤
│                     LAYER 4: LAYOUT (components/layout)                  │
│  BoxTree, FragmentTree, DisplayListBuilder, DamagePropagation            │
│  SVG handled as replaced content (serialized → rasterized image)         │
├─────────────────────────────────────────────────────────────────────────┤
│                     LAYER 3: STYLE (stylo/style)                         │
│  Stylo engine: selector matching, cascade, value computation             │
│  22 style structs, 42 SVG CSS properties registered                     │
│  Property definitions: longhands.toml → data.py → properties.mako.rs     │
├─────────────────────────────────────────────────────────────────────────┤
│                     LAYER 2: SCRIPT DOM (components/script)              │
│  SVGElement → SVGGraphicsElement → SVGSVGElement / SVGImageElement       │
│  Element stores: attributes, style_attribute (PropertyDeclarationBlock)  │
├─────────────────────────────────────────────────────────────────────────┤
│                     LAYER 1: PARSING (html5ever / xml5ever)              │
│  HTML/XML → tokens → DOM tree with SVG namespace                         │
│  create.rs dispatches: ns!(svg) → create_svg_element()                  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Dataflow: End-to-End Pipeline

### Visual Pipeline Diagram

```mermaid
graph TD
    subgraph "PHASE 1: Parsing"
        A[HTML/XML Input] --> B[html5ever/xml5ever Parser]
        B --> C[create_element in create.rs]
        C --> D{namespace?}
        D -->|ns!(svg)| E[create_svg_element]
        D -->|ns!(html)| F[create_html_element]
        E --> G[SVGElement / SVGSVGElement / SVGImageElement]
        G --> H[DOM Tree with SVG nodes]
    end

    subgraph "PHASE 2: Attribute & Style Storage"
        H --> I[Element stores attributes]
        H --> J[style attribute parsed via<br/>parse_style_attribute()]
        J --> K[PropertyDeclarationBlock stored<br/>in element.style_attribute]
        I --> L[width/height attr → PresHints]
        L --> M[CascadeOrigin::PresHints]
    end

    subgraph "PHASE 3: Style Recalc (Stylo)"
        M --> N[Layout thread starts reflow]
        N --> O[RecalcStyle::process_preorder]
        O --> P[recalc_style_at in stylo/traversal.rs]
        P --> Q{Determine RestyleKind}
        Q -->|MatchAndCascade| R[Full selector matching + cascade]
        Q -->|CascadeWithReplacements| S[Re-cascade with rule replacements]
        Q -->|CascadeOnly| T[Re-cascade only]
        R --> U[StyleResolverForElement]
        U --> V[RuleCollector: match selectors]
        V --> W[collect_presentational_hints]
        W --> X[Cascade declarations in priority order]
        X --> Y[Compute values: specified → computed]
        Y --> Z[finish_restyle → ElementData.styles]
        T --> Z
        S --> Z
    end

    subgraph "PHASE 4: SVG Style Debug Logging"
        Z --> AA{is_svg_element?}
        AA -->|Yes| AB[Log all SVG computed styles<br/>fill, stroke, markers, geometry...]
        AA -->|No| AC[Continue normal flow]
    end

    subgraph "PHASE 5: Box Tree Construction"
        AB --> AD[compute_damage_and_rebuild_box_tree]
        AC --> AD
        AD --> AE{display: none?}
        AE -->|Yes| AF[Unset boxes, return early]
        AE -->|No| AG[Build/reuse BoxTree]
        AG --> AH[SVG children: svg > * { display: none }<br/>in UA stylesheet]
        AG --> AI[SVGSVGElement: replaced content<br/>→ serialized → rasterized image]
    end

    subgraph "PHASE 6: Display List & Render"
        AI --> AJ[Fragment::Image with ImageKey]
        AH --> AJ
        AJ --> AK[StackingContextTree construction]
        AK --> AL[DisplayListBuilder::build]
        AL --> AM[WebRender push_image]
        AM --> AN[GPU renders final pixels]
    end

    style A fill:#4a9eff,color:#000
    style Z fill:#ffcc00,color:#000
    style AN fill:#44cc44,color:#000
```

### Detailed Step-by-Step Dataflow

```
STEP 1: PARSING
──────────────────────────────────────────────────────────────────────────────
Input:  HTML/XML source string containing <svg> elements
Process: html5ever/xml5ever tokenizes → builds DOM tree
Output: DOM tree with SVG nodes (namespace = ns!(svg))

Key code: components/script/dom/create.rs:create_svg_element()
  - local_name!("svg")   → SVGSVGElement
  - local_name!("image") → SVGImageElement
  - anything else        → SVGElement (generic)


STEP 2: STYLE ATTRIBUTE PARSING
──────────────────────────────────────────────────────────────────────────────
Input:  style="fill: red; stroke-width: 2" on SVG element
Process: parse_style_attribute() in stylo/properties/declaration_block.rs
  - CSS parser recognizes SVG properties (registered in longhands.toml)
  - Returns PropertyDeclarationBlock
Output: PropertyDeclarationBlock stored in Element.style_attribute field

Key code:
  - components/script/dom/element/element.rs:update_style_attribute()
  - stylo/style/properties/declaration_block.rs:parse_style_attribute()


STEP 3: PRESENTATION HINTS (width/height only)
──────────────────────────────────────────────────────────────────────────────
Input:  <svg width="100" height="200"> attributes
Process: synthesize_presentational_hints_for_legacy_attributes()
  - Only SVGSVGElement's width and height are mapped
  - Other SVG presentation attributes NOT yet implemented
Output: PropertyDeclaration at CascadeOrigin::PresHints level

Key code:
  - components/script/dom/element/element.rs (line ~1335)
  - stylo/style/rule_collector.rs:collect_presentational_hints()


STEP 4: STYLE RECALC (Traversal Setup)
──────────────────────────────────────────────────────────────────────────────
Input:  Dirty DOM tree (elements marked for restyle)
Process: Layout thread creates RecalcStyle, calls traverse_dom()
  - Parallel traversal: process_preorder called for each element
  - Initializes ElementData (style + damage)
  - Calls recalc_style_at() for each element
Output: Each element has ElementData with computed styles

Key code:
  - components/layout/layout_impl.rs: reflow entry point
  - components/layout/traversal.rs: RecalcStyle::process_preorder()
  - stylo/style/traversal.rs: recalc_style_at(), compute_style()


STEP 5: SELECTOR MATCHING
──────────────────────────────────────────────────────────────────────────────
Input:  Element + Stylist (all CSS rules)
Process: StyleResolverForElement::resolve_style_with_default_parents()
  - Match selectors against element (tag, class, id, attributes, pseudo-classes)
  - Collect applicable declarations from all origins
  - Try style sharing cache first (fast path)
Output: ApplicableDeclarationBlock list (ordered by cascade origin)

Key code:
  - stylo/style/style_resolver.rs
  - stylo/style/rule_collector.rs
  - components/script/layout_dom/servo_dangerous_style_element.rs (TElement impl)


STEP 6: CASCADE
──────────────────────────────────────────────────────────────────────────────
Input:  Applicable declarations + parent computed values
Process: Apply declarations in CascadeOrigin priority order:
  1. User Agent rules         (lowest)
  2. User rules
  3. Author rules (stylesheet)
  4. Presentational hints     (SVG width/height attributes)
  5. Style attribute          (inline style="...")
  6. SMIL animations          (SVG-specific)
  7. CSS Animations
  8. CSS Transitions          (highest)
Output: StyleBuilder populated with computed values

Key code:
  - stylo/style/properties/helpers.mako.rs: cascade_property()
  - stylo/style/properties/properties.mako.rs: StyleBuilder


STEP 7: VALUE COMPUTATION (Specified → Computed)
──────────────────────────────────────────────────────────────────────────────
Input:  Specified values from cascade
Process: ToComputedValue trait for each SVG type:
  - SVGPaint: resolves colors, URLs
  - SVGLength: resolves percentages, units
  - SVGOpacity: resolves opacity values
  - SVGStrokeDashArray: resolves dash array values
  - Keyword types: identity conversion (same as specified)
Output: Computed values stored in Arc<style_structs::*>

Key code:
  - stylo/style/values/generics/svg.rs: GenericSVGPaint, GenericSVGLength
  - stylo/style/values/specified/svg.rs: parse implementations
  - stylo/style/values/computed/svg.rs: computed type aliases


STEP 8: ELEMENT DATA STORAGE
──────────────────────────────────────────────────────────────────────────────
Input:  Computed values for element + pseudo-elements
Process: finish_restyle() → set_styles()
  - Stores Arc<ComputedValues> in ElementStyles::primary
  - Computes RestyleDamage from style changes
Output: ElementData { styles, damage, hint, flags }

Key code:
  - stylo/style/data.rs: ElementData, ElementStyles
  - stylo/style/matching.rs: finish_restyle()


STEP 9: SVG DEBUG LOGGING (Diagnostic)
──────────────────────────────────────────────────────────────────────────────
Input:  Computed styles for SVG elements
Process: In process_preorder(), if is_svg_element():
  - Reads get_inherited_svg(): fill, stroke, markers, paint-order, etc.
  - Reads get_svg(): cx, cy, r, vector-effect, flood-color, etc.
  - Reads get_inherited_box(), get_box(), get_inherited_text(), etc.
  - Reads get_font(), get_inherited_ui(), get_effects(), get_position()
Output: Debug log file (svg_styles.log)

Key code:
  - components/layout/traversal.rs (lines 73-177)


STEP 10: BOX TREE & LAYOUT
──────────────────────────────────────────────────────────────────────────────
Input:  DOM tree with ElementData (computed styles)
Process: compute_damage_and_rebuild_box_tree()
  - SVG children hidden by UA rule: svg > * { display: none; }
  - SVGSVGElement treated as replaced content:
    a) Subtree serialized to base64 data URL
    b) VectorImage cached via image cache
    c) Rasterized to ImageKey
    d) Fragment::Image created
Output: BoxTree with Fragment::Image for SVGs

Key code:
  - components/layout/traversal.rs: compute_damage_and_rebuild_box_tree()
  - components/layout/replaced.rs: svg_kind_size(), make_fragments()
  - components/layout/stylesheets/servo.css: svg > * { display: none }


STEP 11: DISPLAY LIST (Ready for Rendering)
──────────────────────────────────────────────────────────────────────────────
Input:  BoxTree with Fragment::Image nodes
Process: DisplayListBuilder::build()
  - Builds StackingContextTree from Fragment tree
  - Fragment::build_display_list() for each fragment
  - SVG images: wr::push_image() with ImageKey
Output: WebRender DisplayList → GPU rendering

Key code:
  - components/layout/display_list/mod.rs: DisplayListBuilder
  - components/layout/display_list/stacking_context.rs
  - components/layout/fragment_tree/fragment.rs: Fragment enum
```

---

## 3. Class Hierarchy and Relationships

### DOM Class Hierarchy

```
                 ┌───────────────────────────────┐
                 │           Node                 │
                 │  (components/script/dom/node)  │
                 └───────────────┬───────────────┘
                                 │
                 ┌───────────────▼───────────────┐
                 │          Element               │
                 │  • namespace (ns!(html/svg))    │
                 │  • local_name (tag name)        │
                 │  • attributes (AttrValue list)  │
                 │  • style_attribute:             │
                 │    Option<Locked<               │
                 │      PropertyDeclarationBlock>> │
                 │  • style_decl (CSSStyleDecl.)   │
                 └───────────────┬───────────────┘
                                 │
                 ┌───────────────▼───────────────┐
                 │         SVGElement              │
                 │  struct {                       │
                 │    element: Element,            │
                 │    style_decl:                  │
                 │      MutNullableDom<            │
                 │        CSSStyleDeclaration>,    │
                 │  }                              │
                 └───────────────┬───────────────┘
                                 │
                 ┌───────────────▼───────────────┐
                 │     SVGGraphicsElement          │
                 │  struct { svgelement: SVGElement } │
                 └───────┬───────────────┬───────┘
                         │               │
          ┌──────────────▼──┐    ┌──────▼──────────────┐
          │  SVGSVGElement  │    │  SVGImageElement     │
          │  • uuid: String │    │  struct {            │
          │  • cached_      │    │    svggraphicselement │
          │    serialized_  │    │  }                   │
          │    data_url     │    └─────────────────────┘
          └──────────────┬──┘
                         │
          data() method: SVGElementData
          ┌──────────────────────────────────────────┐
          │ SVGElementData {                         │
          │   source: base64 data: URL of subtree,   │
          │   width: Option<&AttrValue>,              │
          │   height: Option<&AttrValue>,             │
          │   svg_id: String,                        │
          │   view_box: Option<&AttrValue>            │
          │ }                                        │
          └──────────────────────────────────────────┘
```

### Layout Element Hierarchy

```
                  ┌──────────────────────────────┐
                  │   LayoutElement (safe)        │
                  │   Only access self data        │
                  └──────────────────────────────┘
                              │
                  ┌───────────▼────────────────────┐
                  │ DangerousStyleElement (unsafe)  │
                  │ Extends: TElement + Selectors   │
                  │ Reserved for stylo integration  │
                  │ • layout_element() → LayoutElement│
                  │ • is_svg_element() → bool        │
                  └───────────┬────────────────────┘
                              │
          ┌───────────────────┼──────────────────────┐
          │                   │                       │
  ┌───────▼───────┐  ┌───────▼───────┐  ┌───────────▼────┐
  │ ServoLayout   │  │ ServoDangerous│  │ ServoLayout    │
  │ Element       │  │ StyleElement  │  │ Node           │
  │ (Safe wrapper │  │ (Wraps        │  │ svg_data()     │
  │  for Element) │  │  LayoutDom<   │  │ → Option<      │
  └───────────────┘  │  Element>)    │  │   SVGElement   │
                     └───────────────┘  │   Data>        │
                                        └────────────────┘
```

### Style System Class Hierarchy (Stylo)

```
Stylo Property System (generated from longhands.toml):

  ┌───────────────────────────────────────────────────────────┐
  │                  ComputedValues                           │
  │  (d:\Projects\stylo\style\properties\properties.mako.rs)  │
  │                                                           │
  │  • inherited_svg: Arc<InheritedSVG>    [INHERITED]        │
  │  • svg: Arc<SVG>                       [NON-INHERITED]    │
  │  • inherited_box: Arc<InheritedBox>    [INHERITED]        │
  │  • box_: Arc<Box>                      [NON-INHERITED]    │
  │  • inherited_text: Arc<InheritedText>  [INHERITED]        │
  │  • text: Arc<Text>                     [NON-INHERITED]    │
  │  • font: Arc<Font>                     [INHERITED]        │
  │  • inherited_ui: Arc<InheritedUI>      [INHERITED]        │
  │  • effects: Arc<Effects>               [NON-INHERITED]    │
  │  • position: Arc<Position>             [NON-INHERITED]    │
  │  • (12 more structs: Background, Border, Column, ...)    │
  └───────────────────────────────────────────────────────────┘
```

### SVG-Specific Style Structs Detail

```
INHERITEDSVG (Inherited = YES)              SVG (Inherited = NO)
──────────────────────────────              ──────────────────────
fill: SVGPaint                              cx: LengthPercentage
fill_opacity: SVGOpacity                    cy: LengthPercentage
fill_rule: FillRule                         r: NonNegativeLengthPercentage
stroke: SVGPaint                            rx: NonNegativeLengthPercentage
stroke_width: SVGWidth                      ry: NonNegativeLengthPercentage
stroke_opacity: SVGOpacity                  x: LengthPercentage
stroke_linecap: StrokeLinecap               y: LengthPercentage
stroke_linejoin: StrokeLinejoin             d: DProperty
stroke_miterlimit: NonNegativeNumber        vector_effect: VectorEffect
stroke_dasharray: SVGStrokeDashArray        flood_color: Color
stroke_dashoffset: SVGLength                flood_opacity: Opacity
marker_start: UrlOrNone                     lighting_color: Color
marker_mid: UrlOrNone                       stop_color: Color
marker_end: UrlOrNone                       stop_opacity: Opacity
paint_order: SVGPaintOrder                  clip_path: ClipPath
text_anchor: TextAnchor                     mask_image: OwnedList<Image>
color_interpolation: ColorInterpolation     mask_type: UrlOrNone → MaskType
color_interpolation_filters: ColorInterp    mask_mode: OwnedList<MaskMode>
shape_rendering: ShapeRendering             mask_clip: OwnedList<GeometryBox>
clip_rule: FillRule                         mask_origin: OwnedList<GeometryBox>
                                            mask_composite: OwnedList<MaskComposite>
                                            mask_position_x: OwnedList<Position>
                                            mask_position_y: OwnedList<Position>
                                            mask_repeat: OwnedList<BackgroundRepeat>
                                            mask_size: OwnedList<BackgroundSize>
```

### Key Data Structures

```rust
// ElementData — stored on each DOM element after styling
// File: stylo/style/data.rs
pub struct ElementData {
    pub styles: ElementStyles,       // primary + pseudo styles
    pub damage: RestyleDamage,       // what layout phases to re-run
    pub hint: RestyleHint,           // selector re-matching hint
    pub flags: ElementDataFlags,     // WAS_RESTYLED, etc.
}

pub struct ElementStyles {
    pub primary: Option<Arc<ComputedValues>>,  // main computed style
    pub pseudos: EagerPseudoStyles,             // ::before, ::after, etc.
}

// SVGElementData — for <svg> as replaced element
// File: components/shared/layout/lib.rs
pub struct SVGElementData<'dom> {
    pub source: Option<Result<ServoUrl, ()>>,   // base64 data URL
    pub width: Option<&'dom AttrValue>,
    pub height: Option<&'dom AttrValue>,
    pub svg_id: String,
    pub view_box: Option<&'dom AttrValue>,
}

// RecalcStyle — the traversal entry point
// File: components/layout/traversal.rs
pub struct RecalcStyle<'a> {
    context: &'a LayoutContext<'a>,
}

// LayoutContext — holds all shared layout/state
// File: components/layout/context.rs
pub struct LayoutContext<'a> {
    pub style_context: SharedStyleContext<'a>,
    // ... image cache, font cache, etc.
}
```

---

## 4. Component Inputs/Outputs

### Parsing Layer

| Component | File | Input | Output |
|-----------|------|-------|--------|
| `create_element` | `create.rs:440` | `QualName` (ns, tag, prefix) | DOM Element (SVGElement, SVGSVGElement, etc.) |
| `create_svg_element` | `create.rs:96` | SVG tag name | SVG-specific DOM node |
| `parse_style_attribute` | `declaration_block.rs:1377` | Style string + parser context | `PropertyDeclarationBlock` |
| `update_style_attribute` | `element.rs:2287` | Attribute mutation event | Updates `element.style_attribute` field |

### Style System Layer

| Component | File | Input | Output |
|-----------|------|-------|--------|
| `recalc_style_at` | `traversal.rs:360` | Element + parent style + stylist | Computed styles in ElementData |
| `compute_style` | `traversal.rs:504` | Element + RestyleKind | Arc\<ComputedValues\> |
| `StyleResolverForElement` | `style_resolver.rs` | Element + parent style | Applicable declarations → computed values |
| `RuleCollector` | `rule_collector.rs` | Element + stylist | ApplicableDeclarationBlock list |
| `collect_presentational_hints` | `rule_collector.rs:199` | Element | SVG width/height as PresHints declarations |
| `cascade_property` | `helpers.mako.rs:26` | Declaration + StyleBuilder | Updated computed value in StyleBuilder |
| `finish_restyle` | `matching.rs` | Element + new styles | ElementData updated with damage |
| `StyleBuilder` | `properties.mako.rs:2271` | Parent style + reset style | Accumulates computed values during cascade |

### Layout Layer

| Component | File | Input | Output |
|-----------|------|-------|--------|
| `RecalcStyle::process_preorder` | `traversal.rs:42` | Node + StyleContext | Styled node |
| `compute_damage_and_rebuild_box_tree` | `traversal.rs:205` | DOM tree + ElementData | BoxTree |
| `svg_kind_size` | `replaced.rs:220` | SVGSVGElement node | `ReplacedContentKind::SVGElement` |
| `make_fragments` | `replaced.rs:478` | ReplacedContents | `Fragment::Image` with ImageKey |
| `rasterize_vector_image` | `replaced.rs:~500` | VectorImage + size | ImageKey (rasterized bitmap) |
| `DisplayListBuilder::build` | `display_list/mod.rs:173` | FragmentTree | WebRender DisplayList |
| `Fragment::build_display_list` | `display_list/mod.rs:622` | Fragment + builder | Display items in list |

---

## 5. Starting Point: Parsing

### Entry Point: HTML/XML Parser

The pipeline begins when Servo loads an HTML/SVG file:

```
File/URL → html5ever/xml5ever → Tokens → DOM Tree
```

**Critical dispatch in** `components/script/dom/create.rs`:

```rust
pub fn create_element(
    name: QualName,
    is: Option<LocalName>,
    document: &Document,
    can_gc: CanGc,
) -> DomRoot<Element> {
    match name.ns {
        ns!(html) => create_html_element(name, document, can_gc),
        ns!(svg) => create_svg_element(name, document),
        _ => Element::new(name, None, document),
    }
}
```

**SVG element creation** (line 96-117):
```rust
fn create_svg_element(name: QualName, document: &Document) -> DomRoot<Element> {
    match name.local {
        local_name!("svg") => SVGSVGElement::new(name, document),
        local_name!("image") => SVGImageElement::new(name, document),
        _ => SVGElement::new(name, document),  // generic fallback
    }
}
```

When the HTML parser encounters an `<svg>` tag, it:
1. Opens the SVG namespace (all children use `ns!(svg)`)
2. Creates `SVGSVGElement` for the `<svg>` root
3. Creates generic `SVGElement` for `<rect>`, `<circle>`, `<text>`, `<path>`, `<g>`, etc.
4. Content inside `<style>` blocks is parsed as CSS rules by the Stylist

### Style Attribute Parsing

When the parser encounters `style="fill: red"` on an SVG element:

1. Element stores the attribute value
2. `update_style_attribute()` triggers parsing:
   - Calls `parse_style_attribute()` from Stylo
   - Returns `PropertyDeclarationBlock` with CSS declarations
3. Result is cached in `element.style_attribute` field
4. Used during cascade at `CascadeOrigin::Author` inline level

---

## 6. End Point: Styled DOM Tree Ready for Rendering

### What "Styled DOM Tree" Means

The styled DOM tree is the state where:

1. **Every element has `ElementData`** with computed styles
2. **`ElementStyles::primary`** contains `Arc<ComputedValues>` with all 22 style structs
3. **Damage is computed** and stored in `ElementData::damage`
4. **The layout thread has the tree** (layout operates on a layout-level DOM tree, not the script DOM directly)

### Where It's Ready

The styled DOM tree is ready **after**:

1. `RecalcStyle::process_preorder()` completes for all elements
2. The parallel traversal (`driver::traverse_dom()`) finishes
3. `compute_damage_and_rebuild_box_tree()` runs

At this point, the layout thread has:
- A `BoxTree` with `LayoutBox` variants for each element
- SVG elements handled as `ReplacedContentKind::SVGElement`
- Fragments created (including `Fragment::Image` for SVGs)
- Damage fully propagated

### How It Passes to Rendering

```
                   Styled DOM Tree
                          │
                          ▼
            DisplayListBuilder::build()
                          │
                          ▼
            StackingContextTree construction
            ┌─────────────────────────────┐
            │  For each Fragment:         │
            │  • Fragment::Image          │
            │    → StackingContextContent::│
            │       Fragment              │
            │    → StackingContextSection::│
            │       Foreground            │
            └─────────────────────────────┘
                          │
                          ▼
            WebRender DisplayList
            ┌─────────────────────────────┐
            │  • Spatial nodes            │
            │  • Clip nodes               │
            │  • PushImage for SVG        │
            │    (with ImageKey from      │
            │     rasterized SVG)         │
            └─────────────────────────────┘
                          │
                          ▼
            WebRender Renderer
            ┌─────────────────────────────┐
            │  GPU batch building         │
            │  Shader compilation         │
            │  Rasterization              │
            │  Compositing                │
            └─────────────────────────────┘
                          │
                          ▼
                    Pixels on Screen
```

### Current SVG Rendering Path

Currently, SVG elements are rendered as **replaced content** (like images):

```
SVGSVGElement
    │
    ▼
serialize_subtree() → base64 data: URL
    │
    ▼
Image cache → VectorImage
    │
    ▼
rasterize_vector_image() → ImageKey (bitmap)
    │
    ▼
Fragment::Image { image_key }
    │
    ▼
DisplayList → WebRender push_image()
    │
    ▼
GPU renders rasterized SVG pixels
```

**Note:** Individual SVG child elements (`<rect>`, `<circle>`, etc.) are hidden from layout by the UA rule `svg > * { display: none; }` in `servo.css`. Their computed styles ARE calculated by Stylo (as confirmed by the debug logs), but no layout boxes are created for them. The entire SVG subtree is serialized and rasterized as a single image.

---

## 7. Current Implementation Status

| Feature | Status | Location |
|---------|--------|----------|
| SVG CSS properties in Stylo (42 properties) | ✅ **Done** | `stylo/style/properties/longhands.toml` |
| Inline `style="..."` on SVG elements | ✅ **Done** | Standard CSS parsing path |
| SVG class hierarchy | ⚠️ **Partial** | Only `<svg>`, `<image>`, and generic `SVGElement` |
| SVG `<rect>`, `<circle>`, `<text>`, `<path>`, `<g>` | ❌ **Generic** | All created as `SVGElement` (no specific types) |
| Presentation attributes (`fill="red"`) | ❌ **Not impl.** | Only `width`/`height` on `<svg>` via PresHints |
| Presentational hints for all SVG attributes | ❌ **Phase 2** | Planned but not implemented |
| SVG as replaced content | ✅ **Done** | `replaced.rs` serialization + rasterization |
| Per-element SVG layout (native SVG engine) | ❌ **Future** | Currently uses image-based rendering |
| SVG debug logging | ✅ **Done** | `traversal.rs` lines 73-177 |
| SVG property inheritance (InheritedSVG) | ✅ **Done** | Stylo struct-level inheritance |
| SVG property reset (SVG struct) | ✅ **Done** | Non-inherited via StyleBuilder reset |
| `color-interpolation` and `color-interpolation-filters` | ✅ **Done** | Defined in longhands.toml |
| `white-space-collapse` / `text-wrap-mode` in logs | ✅ **Done** | Added to debug log format |
| SVG `<use>` element | ⚠️ **Partial** | Shadow DOM handling in rule collector |
| SVG animations (SMIL) | ❌ **Not impl.** | Cascade level reserved but not functional |

---

## Appendix: Key File Reference

### Parsing & DOM

| File | Purpose |
|------|---------|
| `components/script/dom/create.rs` | Element creation dispatch (HTML vs SVG) |
| `components/script/dom/svg/svgelement.rs` | Base SVGElement struct |
| `components/script/dom/svg/svggraphicselement.rs` | SVGGraphicsElement (intermediate base) |
| `components/script/dom/svg/svgsvgelement.rs` | SVGSVGElement (subtree serialization, data) |
| `components/script/dom/svg/svgimageelement.rs` | SVGImageElement |
| `components/script/dom/element/element.rs` | Style attribute storage, PresHints synthesis |
| `components/shared/layout/lib.rs` | SVGElementData struct |

### Style System (Stylo)

| File | Purpose |
|------|---------|
| `stylo/style/properties/longhands.toml` | All SVG CSS property definitions |
| `stylo/style/properties/data.py` | Code generation: parses TOML, generates Rust |
| `stylo/style/properties/properties.mako.rs` | Template: ComputedValues, StyleBuilder, per-property code |
| `stylo/style/properties/helpers.mako.rs` | Template: cascade_property, keyword types |
| `stylo/style/properties/declaration_block.rs` | `parse_style_attribute()` for inline styles |
| `stylo/style/values/generics/svg.rs` | Generic SVG types (SVGPaint, SVGLength, etc.) |
| `stylo/style/values/specified/svg.rs` | Specified SVG types with Parse impls |
| `stylo/style/values/computed/svg.rs` | Computed SVG type aliases |
| `stylo/style/traversal.rs` | `recalc_style_at()`, `compute_style()`, `DomTraversal` |
| `stylo/style/context.rs` | SharedStyleContext, StyleContext |
| `stylo/style/data.rs` | ElementData, ElementStyles |
| `stylo/style/dom.rs` | TElement, TNode traits, `is_svg_element()` |
| `stylo/style/style_resolver.rs` | Selector matching + cascade orchestration |
| `stylo/style/rule_collector.rs` | Applicable declaration collection |
| `stylo/style/matching.rs` | `finish_restyle()`, damage computation |

### Layout

| File | Purpose |
|------|---------|
| `components/layout/traversal.rs` | RecalcStyle, SVG debug logging, box tree rebuild |
| `components/layout/layout_impl.rs` | Reflow entry point, traversal setup |
| `components/layout/context.rs` | LayoutContext |
| `components/layout/dom.rs` | LayoutBox enum, NodeExt, box storage |
| `components/layout/dom_traversal.rs` | Box tree construction traversal |
| `components/layout/replaced.rs` | SVG as replaced content (serialize, rasterize) |
| `components/layout/fragment_tree/fragment.rs` | Fragment enum (Image, Box, Text, etc.) |
| `components/layout/display_list/mod.rs` | DisplayListBuilder, Fragment::build_display_list |
| `components/layout/display_list/stacking_context.rs` | StackingContextTree construction |
| `components/layout/flow/root.rs` | BoxTree::construct, BoxTree::layout |
| `components/layout/stylesheets/servo.css` | UA stylesheet (svg > * { display: none }) |
| `components/layout/layout_box_base.rs` | add_damage, fragment cache management |

### Layout DOM Bridge

| File | Purpose |
|------|---------|
| `components/script/layout_dom/servo_dangerous_style_element.rs` | TElement impl, selector matching, is_svg_element() |
| `components/script/layout_dom/servo_layout_node.rs` | LayoutNode impl, svg_data() delegation |
| `components/shared/layout/layout_element.rs` | DangerousStyleElement trait definition |
| `components/shared/layout/layout_node.rs` | LayoutNode trait (svg_data method) |
| `components/shared/layout/layout_damage.rs` | LayoutDamage bitflag type |
