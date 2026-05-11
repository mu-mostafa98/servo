# SVG Primer for Engine Architecture

> What you need to know about SVG to design a rendering engine.
> No web-design content. No CSS-animation tricks. Only the concepts that shape engine architecture.

---

## 1. SVG Document Structure — The Tree

SVG is an XML document tree. Every element belongs to one of three categories:

### Renderable Elements (produce visual output)

| Element | What it draws | Notes |
|---------|--------------|-------|
| `<rect>` | Rectangle (with optional rounded corners) | `x`, `y`, `width`, `height`, `rx`, `ry` |
| `<circle>` | Circle | `cx`, `cy`, `r` |
| `<ellipse>` | Ellipse | `cx`, `cy`, `rx`, `ry` |
| `<line>` | Straight line segment | `x1`, `y1`, `x2`, `y2` |
| `<polyline>` | Connected line segments | `points="x1,y1 x2,y2 x3,y3"` |
| `<polygon>` | Closed shape | Same as polyline but auto-closes |
| `<path>` | Any curve or shape (most powerful) | `d="M10,10 L50,50 C..."` — the `d` attribute is a mini-language |
| `<text>` | Text content | Font, positioning, text-anchor |
| `<image>` | Embedded raster image | `href`, `x`, `y`, `width`, `height` |
| `<use>` | Clone of another element | `href="#id"` — re-renders a referenced element at a new position |

### Container Elements (group or structure)

| Element | Purpose |
|---------|---------|
| `<svg>` | Root element. Creates a new viewport/coordinate system. Can be nested. |
| `<g>` | Group — applies inherited attributes to children. No visual output itself. |
| `<defs>` | Defines elements for later reuse. Children never render directly — they must be referenced. |
| `<symbol>` | Like `<defs>` but defines a reusable viewport (used by `<use>`). |

### Non-Renderable / Reference Elements

These are defined in `<defs>` and referenced by `url(#id)`:

| Element | Purpose | Referenced By |
|---------|---------|--------------|
| `<linearGradient>` | Color gradient along a line | `fill="url(#g)"` or `stroke="url(#g)"` |
| `<radialGradient>` | Color gradient radiating from a point | Same |
| `<pattern>` | Repeating tile pattern | `fill="url(#p)"` |
| `<clipPath>` | Clipping region | `clip-path="url(#c)"` |
| `<mask>` | Alpha mask | `mask="url(#m)"` |
| `<filter>` | Filter effect chain | `filter="url(#f)"` |
| `<marker>` | Arrowhead/bullet for path vertices | `marker-start`, `marker-mid`, `marker-end` |

### Key Engine Implication

```rust
enum SvgElementKind {
    // Renderable
    Rect, Circle, Ellipse, Line, Polyline, Polygon, Path,
    Text, Image, Use,
    // Container
    Svg, G, Defs, Symbol,
    // Reference (never render directly)
    LinearGradient, RadialGradient, Pattern,
    ClipPath, Mask, Filter, Marker,
}
```

Your engine must distinguish: **container** elements affect layout/traversal, **reference** elements are stored in a registry and resolved when needed, **renderable** elements produce paint commands.

---

## 2. The Coordinate System Model (Most Important)

SVG has **four layers of coordinate systems** through which every element passes:

```
Screen space (CSS pixels of the HTML document)
    ↑  CSS transform (transform: scale(), rotate() on the <svg> element)
SVG viewport space (the <svg>'s own coordinate system)
    ↑  viewBox + preserveAspectRatio
SVG user space (logical coordinates used by child elements)
    ↑  transform attribute on <g>, <rect>, etc.
Element local space (the element's own geometry)
```

### The viewBox Mapping

The `viewBox` attribute on `<svg>` maps user coordinates to the viewport:

```html
<svg width="200" height="200" viewBox="0 0 100 100">
```

This means: the user coordinate system is 100×100 (from 0,0 to 100,100), and it's **scaled up 2×** to fill the 200×200 viewport. A `<rect width="50">` draws at 50 user-units wide = 100px on screen.

The math:

```
scale_x = viewport_width / viewBox_width   = 200 / 100 = 2
scale_y = viewport_height / viewBox_height = 200 / 100 = 2
```

### preserveAspectRatio

Controls what happens when viewport and viewBox have different aspect ratios:

```
preserveAspectRatio="xMidYMid meet"   ← default
```

- **Alignment** (`xMidYMid`): Center the scaled content horizontally and vertically
- **Meet/Slice** (`meet`): Scale until everything fits (letterbox). `slice`: Scale until viewport fills (crop)

```html
<!-- viewport 200×100, viewBox 0 0 100 100 (square) -->
<svg width="200" height="100" viewBox="0 0 100 100"
     preserveAspectRatio="xMidYMid meet">
  <!-- scaled by min(200/100, 100/100) = 1×
       → 100×100 centered in 200×100 viewport -->
</svg>
```

**Your engine must implement this math.** It runs during `svg_kind_size()` and affects every child element's effective coordinates.

### Nested Viewports

An `<svg>` inside an SVG creates a new viewport with its own viewBox:

```html
<svg width="400" height="400">
  <svg x="50" y="50" width="100" height="100" viewBox="0 0 10 10">
    <!-- 10×10 user space mapped to 100×100 region at (50,50) -->
    <!-- scale = 100/10 = 10× -->
  </svg>
</svg>
```

Each nested `<svg>` pushes a new transform onto the stack.

### The Transform Attribute

Any element can have a `transform` attribute:

```html
<g transform="translate(50, 50) scale(2) rotate(45)">
  <rect x="0" y="0" width="50" height="50" />
</g>
```

Transforms are **right-to-left**: the rect is first translated, then scaled, then rotated. The full transform chain for a deeply nested element multiplies all parent transforms:

```
element_transform = viewport_transform × parent_group_transform × element_transform
```

### Units

- **No unit** = user units (the default coordinate system)
- `px`, `pt`, `cm`, `mm`, `in` — absolute CSS units
- `em`, `ex` — relative to font size
- **Percentages** — context-dependent (viewport width/height for `<svg>`, bounding box for gradients)

### Key Engine Implication

Your layout engine must maintain a **transform stack**. Every element gets a final accumulated transform that converts its local coordinates to viewport pixels. This is the same concept as a scene graph in graphics engines.

```rust
struct TransformStack {
    stack: Vec<AffineTransform>,
}

impl TransformStack {
    fn push(&mut self, t: AffineTransform) { ... }
    fn pop(&mut self) { ... }
    fn current(&self) -> AffineTransform {
        self.stack.iter().product()  // multiply all transforms
    }
}
```

---

## 3. Paint Servers — Fill and Stroke Are Not Just Colors

In SVG, `fill="red"` and `fill="url(#myGradient)"` are both valid. The second form is a **paint server reference**.

### Paint Server Types

| Server | How It Works | Needs From Layout |
|--------|-------------|-------------------|
| Solid color | `fill="red"` / `fill="#ff0000"` | Nothing |
| `linearGradient` | Color transition along a line. Can use `objectBoundingBox` (relative to element's bounding box) or `userSpaceOnUse` (absolute coordinates). | Element bounding box for `objectBoundingBox` mode |
| `radialGradient` | Color transition radiating from a point. Same coordinate modes. | Element bounding box |
| `pattern` | Repeat a tile. Coordinate modes. | Element bounding box + viewport size |
| `currentColor` | Takes the value of the CSS `color` property | Style resolution |

### The Reference Problem

Paint servers live in `<defs>` and are referenced by URL:

```html
<defs>
  <linearGradient id="g1" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0%" stop-color="red"/>
    <stop offset="100%" stop-color="blue"/>
  </linearGradient>
</defs>
<rect fill="url(#g1)" .../>
```

**Engine implication:** The paint builder cannot resolve `fill` until the rendering phase. It must:
1. Parse the fill value → recognize it as a URL reference
2. Look up the referenced element by ID in the reference registry
3. If the paint server uses `objectBoundingBox` — need the element's computed bounding box first
4. If the paint server also references elements (gradient can reference a pattern...) — recursive resolution

This means a **dependent evaluation order**: bounding boxes must be computed before paint servers can be resolved for elements that use `objectBoundingBox`.

---

## 4. Rendering Tree vs. DOM Tree

Not all DOM elements become rendering tree nodes. The rendering model works like this:

### Default Draw Order

SVG uses the **painter's algorithm**: elements draw in DOM order (first in DOM = drawn first, may be covered by later elements).

```html
<circle cx="50" cy="50" r="40" fill="red"/>     ← drawn first
<circle cx="70" cy="50" r="40" fill="blue"/>    ← drawn on top
```

### Elements That Never Render

These elements are in the DOM but produce no rendering tree node:
- `<defs>`, `<linearGradient>`, `<radialGradient>`, `<pattern>`, `<clipPath>`, `<mask>`, `<filter>`, `<marker>`, `<script>`, `<style>`

They exist only to be referenced by renderable elements.

### Elements That Are Not Rendered

- `display: none` — removed from rendering tree
- `visibility: hidden` — in rendering tree but not painted (still affects bounding boxes)

### Stacking Contexts (z-index in SVG 2)

SVG 1.1 had no z-index — DOM order was the only stacking order. SVG 2 adds `z-index`.

A new stacking context is created by:
- Root `<svg>` element
- Any element with `opacity < 1`
- Any element with `filter`, `mask`, `clip-path`
- Any element with `z-index != auto`

This is the same concept as CSS stacking contexts.

### Key Engine Implication

```rust
fn build_rendering_tree(dom_node: &Node) -> Vec<RenderingNode> {
    match dom_node.element_type {
        Defs | LinearGradient | ... => vec![],  // skip, register as reference
        Element if display == none => vec![],    // skip
        Element => {
            let children = dom_node.children()
                .flat_map(build_rendering_tree)
                .collect();
            vec![RenderingNode { element, children }]
        }
    }
}
```

---

## 5. The `<use>` Element — Element Reuse

`<use>` is SVG's cloning mechanism:

```html
<defs>
  <circle id="c1" cx="50" cy="50" r="40" fill="red"/>
</defs>
<use href="#c1" x="100" y="100" fill="blue"/>
```

### How `<use>` Works Semantically

1. The referenced element is deep-cloned into a **shadow tree** (like Shadow DOM)
2. The clone is positioned at the `<use>` element's location
3. Attributes on `<use>` override corresponding attributes on the cloned element
4. Styles from the `<use>` parent cascade into the shadow tree

### Key Challenges

- **Circular references**: `<use>` cannot reference an ancestor that contains the `<use>` itself
- **Attribute overriding**: The `<use>` element's `x`, `y` become `translate(x,y)` on the cloned content; `fill`, `stroke`, etc. override the clone's attributes
- **Event target**: Events on the cloned content appear to come from the `<use>` element (spec behavior)

### Engine Implication

Your engine needs a **shadow tree resolver** that:
1. When encountering `<use>`, finds the referenced element by ID
2. Clones the referenced subtree (or reuses with attribute override)
3. Wraps the clone in a transformed group (from `<use>`'s position)
4. Applies attribute overrides

This is essentially implementing a minimal Shadow DOM for SVG.

---

## 6. Fills, Strokes, and Markers

### Fill

The fill paints the interior of a shape:

```html
<rect fill="blue" fill-opacity="0.5" fill-rule="evenodd"/>
```

- `fill-rule`: `nonzero` (default) or `evenodd` — determines how overlapping paths are filled
- `fill-opacity`: alpha multiplier for the fill paint

### Stroke

The stroke paints along the boundary of a shape:

```html
<path stroke="black" stroke-width="4"
      stroke-linecap="round"         // butt | round | square
      stroke-linejoin="miter"        // miter | round | bevel
      stroke-miterlimit="4"
      stroke-dasharray="10,5"        // dash pattern
      stroke-dashoffset="2"          // where dash pattern starts
      stroke-opacity="0.8"/>
```

### Markers

Markers are arrowheads/bullets attached to path vertices:

```html
<defs>
  <marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5"
          markerWidth="6" markerHeight="6" orient="auto">
    <path d="M0,0 L10,5 L0,10" fill="black"/>
  </marker>
</defs>
<path d="M10,10 L90,90" marker-end="url(#arrow)"/>
```

Markers are positioned and oriented at the start, middle, and/or end of each path segment.

### Engine Implication

Each renderable element produces **up to three draw operations**:
1. **Fill** the interior (if fill is not `none`)
2. **Stroke** the boundary (if stroke is not `none`)
3. **Markers** at vertices (if markers are specified)

Each of these can be a solid color, a gradient, a pattern, or a reference to be resolved.

---

## 7. SVG Text — Brief Overview

Text in SVG is more complex than shapes:

```html
<text x="10" y="30" font-family="Arial" font-size="20"
      font-weight="bold" text-anchor="middle"
      fill="black">
  Hello
  <tspan x="10" dy="25" fill="red">World</tspan>
</text>
```

### Key properties:
- `x`, `y` — absolute position (can be a list: `x="10 20 30"` positions each character)
- `dx`, `dy` — relative position
- `text-anchor` — `start | middle | end` — horizontal alignment
- `dominant-baseline` — vertical alignment
- `<tspan>` — inline text span with its own formatting

### Engine Implication

Text rendering requires:
- Font loading and selection (integrate with Servo's font system)
- Text shaping (Harfbuzz or equivalent)
- Glyph positioning (kerning, alignment, multi-line)
- This is the most complex single feature — do not attempt until shapes work

---

## 8. Clipping, Masking, and Filters (Overview)

### Clip Path

```html
<clipPath id="clip">
  <circle cx="50" cy="50" r="40"/>
</clipPath>
<rect x="0" y="0" width="100" height="100" clip-path="url(#clip)"/>
```

The clipped element is only visible inside the clip path's shape.

### Mask

```html
<mask id="fade">
  <rect width="100" height="100" fill="url(#gradient)"/>  <!-- alpha = luminance -->
</mask>
<rect width="100" height="100" mask="url(#fade)" fill="red"/>
```

A mask uses the luminance of its content as an alpha mask. White = opaque, black = transparent.

### Filter

```html
<filter id="blur">
  <feGaussianBlur stdDeviation="3"/>
</filter>
<rect filter="url(#blur)" .../>
```

Filters are a **pipeline of primitives** that process pixel buffers:

```
Input → feGaussianBlur → feColorMatrix → feMerge → Output
```

Each primitive can take input from the previous step, a reference graphic, or a background image.

### Engine Implication

Filters require **offscreen render surfaces** — the element is first rendered to a temporary buffer, then the filter pipeline processes it. This is expensive and requires careful resource management.

---

## 9. SMIL Animation Model (Brief)

SVG has its own animation system (SMIL) independent of CSS:

```html
<circle cx="100" cy="100" r="50" fill="blue">
  <animate attributeName="cx" from="100" to="300"
           dur="2s" repeatCount="indefinite"/>
</circle>
```

### Key concepts:
- `attributeName` — which attribute to animate
- `from`/`to`/`values` — keyframe values
- `dur` — duration
- `repeatCount` — how many times to repeat
- `fill="freeze"` — hold final value
- `calcMode` — `linear | discrete | spline | paced` — interpolation method
- `keyTimes`/`keySplines` — custom easing

### Animation types:
| Element | Purpose |
|---------|---------|
| `<animate>` | Animate a scalar or color attribute |
| `<set>` | Set an attribute to a value (no interpolation) |
| `<animateTransform>` | Animate a transform attribute (rotate, scale, etc.) |
| `<animateMotion>` | Animate along a path |

### Engine Implication

Each animation creates a **time-varying override** of the target attribute. The engine must:
1. Maintain an animation clock synchronized with the rendering frame rate
2. Store active animations with their timing state
3. On each frame: evaluate all active animations, compute current values, temporarily override the base attributes
4. Re-render if any animation is active → `needs_rendering_update() = true`
5. Remove expired animations

---

## 10. Summary — What Your Engine Must Do

```
Input: Styled SVG DOM subtree + viewport size from CSS

1. Build rendering tree
   ─ Skip non-renderable elements (defs, gradients, etc.)
   ─ Register references (paint servers, clip paths, masks, filters)
   ─ Resolve <use> shadow trees

2. Layout / compute bounding boxes
   ─ Maintain transform stack (viewBox → group → element)
   ─ Compute viewBox mapping + preserveAspectRatio
   ─ Compute each element's bounding box in user space
   ─ Propagate bounding boxes up for paint server resolution

3. Resolve paint servers
   ─ For each element's fill, stroke, marker, clip-path, mask, filter
   ─ Look up references by ID
   ─ If objectBoundingBox: compute effective gradient/pattern from bounding box

4. Build paint commands
   ─ For each renderable element in DOM order:
     ─ Push current group transform (from transform stack)
     ─ If has filter → create offscreen surface
     ─ If has clip/mask → create clipping region
     ─ If has fill → emit Fill command (color, gradient, or pattern)
     ─ If has stroke → emit Stroke command (color, gradient, or pattern)
     ─ If has markers → emit Marker commands at vertices
     ─ If has filter → apply filter pipeline, composite result
     ─ Pop transforms / surfaces

5. Animation tick (if any active)
   ─ Advance animation clock
   ─ Evaluate active animations
   ─ Override animated attributes on target elements
   ─ Flag for re-render
```

The actual rendering pipeline is:

```
Build rendering tree
    ↓
Compute layouts + transforms
    ↓
Resolve paint servers
    ↓
Generate paint commands
    ↓
Convert to Servo display list items
    ↓
WebRender
```
