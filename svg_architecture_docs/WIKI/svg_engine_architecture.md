# SVG Engine — Architecture

> A native SVG rendering engine that replaces the current serialize-as-image approach. Keeps the SVG subtree in the document tree and produces vector display items directly.

---

## 1. System Boundary

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           SVG Engine                                     │
│                                                                          │
│  Input:                                                                  │
│    Styled SVG DOM subtree                                                │
│      (Arc<ComputedValues> on every element,                              │
│       resolved by Stylo cascade including                                │
│       parent-inherited properties)                                       │
│                                                                          │
│    Viewport size (Au × Au)                                               │
│      from the <svg> element's CSS width/height                           │
│                                                                          │
│    Device pixel ratio                                                    │
│      for mapping Au → physical pixels                                   │
│                                                                          │
│  Output:                                                                 │
│    SvgDisplayList — vector paint commands                                │
│      consumed by Servo's display list builder                            │
│                                                                          │
│  Non-goals (Phase 1):                                                    │
│    • Scripting (<script>, event handlers)                                │
│    • SMIL animation                                                      │
│    • <foreignObject> compositing                                         │
│    • Full SVG 2 spec compliance                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Integration Point

The engine plugs into `make_fragments()` in `layout/replaced.rs`, replacing the current `Fragment::Image` path:

```rust
// Current: serialize → rasterize → ImageKey → Fragment::Image
// New:    svg_engine.process() → SvgDisplayList → Fragment::Svg

pub fn make_fragments(...) -> Vec<Fragment> {
    match &self.kind {
        // ... existing cases ...
        ReplacedContentKind::SVGElement { .. } => {
            let display_list = SVG_ENGINE.process(
                styled_subtree,   // from DOM + Stylo
                viewport_size,    // from CSS width/height
                device_pixel_ratio,
            );
            vec![Fragment::Svg(ArcRefCell::new(SvgFragment {
                base,
                display_list,
                is_animating: false,  // set true when animations active
            }))]
        }
    }
}
```

Where `SvgFragment` is a new fragment variant:

```rust
pub(crate) struct SvgFragment {
    pub base: BaseFragment,
    pub display_list: SvgDisplayList,
    pub is_animating: bool,
}
```

---

## 2. Component Architecture

```
                         ┌─────────────────────┐
                         │    SvgEngine         │
                         │  (orchestrator)      │
                         └──────┬───────────────┘
                                │
              ┌─────────────────┼──────────────────┐
              ▼                 ▼                   ▼
    ┌──────────────────┐ ┌──────────────┐ ┌──────────────┐
    │  StyleResolver   │ │ LayoutEngine │ │ PaintBuilder │
    │                  │ │              │ │              │
    │ Extract SVG      │ │ Compute      │ │ Convert laid │
    │ properties from  │ │ viewBox      │ │ out tree to  │
    │ ComputedValues   │ │ mapping      │ │ paint cmds   │
    │                  │ │              │ │              │
    │ Resolve paint    │ │ Transform    │ │ Emit Fill,   │
    │ server URLs      │ │ stack        │ │ Stroke, Text,│
    │                  │ │              │ │ Gradient...  │
    │ Handle           │ │ Bounding     │ │              │
    │ inheritance      │ │ boxes        │ │ Handle clip/ │
    │                  │ │              │ │ mask/filter  │
    └──────────────────┘ └──────────────┘ └──────────────┘
                                │
                         ┌──────┴──────┐
                         │   Registry  │
                         │             │
                         │ Stores defs │
                         │ referenced  │
                         │ by url(#id) │
                         └─────────────┘
```

### 2.1 SvgEngine (Orchestrator)

Owns all sub-components and drives the pipeline per frame.

```rust
pub struct SvgEngine {
    styles: StyleResolver,
    layout: LayoutEngine,
    paint: PaintBuilder,
    registry: ReferenceRegistry,
}

impl SvgEngine {
    pub fn process(
        &mut self,
        root: &SvgStyledNode,      // styled SVG element tree
        viewport: Size2D<Au>,      // CSS pixel size of <svg>
        dpr: Scale<f32, CSSPixel, DevicePixel>,
    ) -> SvgDisplayList;
}
```

**Pipeline execution order per call:**

```
1. StyleResolver.resolve(root)
     → extracts fill, stroke, transform, etc. from ComputedValues
     → resolves paint server URLs → records references

2. LayoutEngine.compute(root, viewport)
     → computes viewBox → preserveAspectRatio → viewport transform
     → walks tree, maintains transform stack
     → computes bounding boxes for every element

3. PaintBuilder.build(root)
     → walks laid-out tree in DOM order
     → for each renderable element:
         - resolves paint servers (gradient, pattern) from registry
         - if bounding-box relative: computes from element's bbox
         - emits Fill / Stroke / Marker / Text commands
     → wraps in Group commands for transforms/opacity
     → returns SvgDisplayList
```

### 2.2 StyleResolver

**Responsibility:** Take a DOM subtree where Stylo has computed `Arc<ComputedValues>`, and extract SVG-specific properties into a flat struct the engine can use.

```rust
pub struct StyleResolver {
    /// Map of element UUID → resolved SVG styles
    styles: HashMap<Uuid, ResolvedSvgStyles>,
}

pub struct ResolvedSvgStyles {
    // Fill
    pub fill: PaintServerValue,       // None | Solid(color) | Url("id")
    pub fill_opacity: f32,
    pub fill_rule: FillRule,          // NonZero | EvenOdd

    // Stroke
    pub stroke: PaintServerValue,
    pub stroke_opacity: f32,
    pub stroke_width: Length,
    pub stroke_linecap: LineCap,      // Butt | Round | Square
    pub stroke_linejoin: LineJoin,     // Miter | Round | Bevel
    pub stroke_miterlimit: f32,
    pub stroke_dasharray: Vec<Length>,
    pub stroke_dashoffset: Length,

    // Transform
    pub transform: Option<Transform>,  // from CSS or presentation attr

    // Visibility
    pub display: Display,
    pub visibility: Visibility,

    // References
    pub clip_path: Option<String>,     // url(#id) → registry key
    pub mask: Option<String>,
    pub filter: Option<String>,
    pub marker_start: Option<String>,
    pub marker_mid: Option<String>,
    pub marker_end: Option<String>,

    // Text
    pub font_family: FontFamily,
    pub font_size: Length,
    pub font_style: FontStyle,
    pub font_weight: FontWeight,
    pub text_anchor: TextAnchor,       // Start | Middle | End
    pub dominant_baseline: Baseline,

    // Opacity
    pub opacity: f32,
}
```

**How it resolves paint server references:**

```
fill="url(#myGradient)"  →  PaintServerValue::Url("myGradient")
fill="red"               →  PaintServerValue::Solid(Color::red)
fill="none"              →  PaintServerValue::None
```

The StyleResolver does NOT resolve the URL to the actual gradient — it just records the reference. The PaintBuilder does full resolution when it needs the gradient definition (which may depend on the element's bounding box).

**Inheritance:**

SVG properties cascade from parent to child. The resolver walks the tree top-down:
- If element has explicit `fill` → use it
- If not → inherit from parent's `fill`
- If parent doesn't have one → use SVG default (black for fill, none for stroke)

### 2.3 LayoutEngine

**Responsibility:** Compute the geometry of every element — bounding boxes, viewport transforms, and the accumulated transform from root to element.

```rust
pub struct LayoutEngine {
    /// Per-element layout data, keyed by element UUID
    layouts: HashMap<Uuid, ElementLayout>,
}

pub struct ElementLayout {
    /// Local bounding box in element's own coordinate space
    pub local_bbox: Rect<f32>,
    /// Bounding box in viewport (screen) space
    pub viewport_bbox: Rect<f32>,
    /// Accumulated transform: element local → viewport space
    pub transform: AffineTransform,
    /// The element's own transform (from the transform attribute)
    pub local_transform: AffineTransform,
}
```

#### The Transform Stack

The engine maintains a stack of affine transforms as it walks the tree:

```
For each element (depth-first):
  1. Push element's local transform
  2. Compute element's local_bbox (from geometry + current transform)
  3. Compute element's viewport_bbox = transform ∘ local_bbox
  4. Recurse to children
  5. Pop element's transform
```

```rust
struct TransformStack {
    stack: Vec<AffineTransform>,
    current: AffineTransform,  // cached product of all transforms
}

impl TransformStack {
    fn push(&mut self, t: AffineTransform) {
        self.current = self.current.pre_transform(t);
        self.stack.push(t);
    }
    fn pop(&mut self) {
        let t = self.stack.pop().unwrap();
        self.current = self.current.post_transform(t.inverse());
    }
    fn current(&self) -> &AffineTransform { &self.current }
}
```

#### viewBox Mapping

The viewBox maps user coordinates to the viewport:

```
scale_x = viewport_width  / viewBox_width
scale_y = viewport_height / viewBox_height
```

With preserveAspectRatio:

```rust
fn compute_viewbox_transform(
    viewport: Size2D<f32>,
    viewbox: Rect<f32>,
    preserve_aspect_ratio: &PreserveAspectRatio,
) -> AffineTransform {
    // 1. Compute base scale
    let scale_x = viewport.width / viewbox.width;
    let scale_y = viewport.height / viewbox.height;

    // 2. Apply meet/slice
    let scale = match preserve_aspect_ratio.align {
        Align::None => Vector2D::new(scale_x, scale_y),
        _ => {
            let s = if preserve_aspect_ratio.meet_or_slice == MeetOrSlice::Meet {
                scale_x.min(scale_y)  // letterbox
            } else {
                scale_x.max(scale_y)  // crop
            };
            Vector2D::new(s, s)
        }
    };

    // 3. Apply alignment (xMidYMid, xMinYMin, etc.)
    let tx = match preserve_aspect_ratio.align_x {
        AlignX::Min => 0.0,
        AlignX::Mid => (viewport.width - viewbox.width * scale.x) / 2.0,
        AlignX::Max => viewport.width - viewbox.width * scale.x,
    };
    let ty = match preserve_aspect_ratio.align_y {
        AlignY::Min => 0.0,
        AlignY::Mid => (viewport.height - viewbox.height * scale.y) / 2.0,
        AlignY::Max => viewport.height - viewbox.height * scale.y,
    };

    AffineTransform::translate(tx, ty) * AffineTransform::scale(scale.x, scale.y)
}
```

#### Bounding Box Computation

Each element's bounding box depends on its type:

| Element | Bounding Box |
|---------|-------------|
| `<rect>` | From `x`, `y`, `width`, `height` |
| `<circle>` | From `cx`, `cy`, `r` |
| `<path>` | From path data (call `path.bbox()`) |
| `<g>` | Union of all children's bounding boxes |
| `<use>` | Bounding box of the cloned content |

#### Nested Viewports

An `<svg>` inside SVG creates a new viewport. Push a new viewport transform onto the stack:

```rust
fn layout_svg_element(
    element: &SvgNode,
    parent_transform: &AffineTransform,
    stack: &mut TransformStack,
) {
    let viewport_size = Size2D::new(element.width, element.height);
    let viewbox_transform = compute_viewbox_transform(
        viewport_size,
        element.viewbox,
        element.preserve_aspect_ratio,
    );

    // Push viewport transform on top of parent
    stack.push(viewbox_transform);
    layout_children(element, stack);
    stack.pop();
}
```

### 2.4 PaintBuilder

**Responsibility:** Walk the laid-out SVG element tree in DOM order and produce a flat sequence of paint commands for Servo's display list.

```rust
pub struct PaintBuilder;

impl PaintBuilder {
    pub fn build(
        &self,
        root: &SvgStyledNode,
        layouts: &HashMap<Uuid, ElementLayout>,
        styles: &HashMap<Uuid, ResolvedSvgStyles>,
        registry: &ReferenceRegistry,
    ) -> SvgDisplayList;
}
```

#### SvgDisplayList

```rust
pub struct SvgDisplayList {
    pub commands: Vec<DisplayCommand>,
}

pub enum DisplayCommand {
    Fill {
        path: PathData,
        paint: ResolvedPaint,     // SolidColor | Gradient | Pattern
        fill_rule: FillRule,
        opacity: f32,
    },
    Stroke {
        path: PathData,
        paint: ResolvedPaint,
        params: StrokeParams,
        opacity: f32,
    },
    Group {
        children: Vec<DisplayCommand>,
        transform: AffineTransform,
        opacity: f32,
    },
    Clip {
        id: ClipId,
        paths: Vec<PathData>,
        fill_rule: FillRule,
    },
    ClipUse {
        id: ClipId,
        children: Vec<DisplayCommand>,
    },
    Mask {
        id: MaskId,
        children: Vec<DisplayCommand>,  // luminance → alpha
    },
    Filter {
        id: FilterId,
        primitives: Vec<FilterPrimitive>,
        children: Vec<DisplayCommand>,
    },
    Text {
        position: Point2D<f32>,
        text: String,
        font: FontDescriptor,
        paint: ResolvedPaint,
    },
    Image {
        data: ImageData,
        rect: Rect<f32>,
    },
}
```

#### Paint Server Resolution

This is the most complex part. Paint servers (gradients, patterns) are defined in `<defs>` and referenced by `url(#id)`. Some use `objectBoundingBox` and need the element's bounding box.

```rust
fn resolve_paint(
    value: &PaintServerValue,
    element_bbox: Rect<f32>,
    registry: &ReferenceRegistry,
) -> ResolvedPaint {
    match value {
        PaintServerValue::None => ResolvedPaint::None,
        PaintServerValue::Solid(color) => ResolvedPaint::Solid(*color),
        PaintServerValue::Url(id) => {
            match registry.get(id) {
                Reference::LinearGradient(g) => {
                    if g.coordinate_mode == BoundingBoxMode::ObjectBoundingBox {
                        // Map gradient coordinates from [0,1] to element's bbox
                        ResolvedPaint::Gradient(apply_bbox_to_gradient(g, element_bbox))
                    } else {
                        // userSpaceOnUse — gradient coords are absolute
                        ResolvedPaint::Gradient(g.clone())
                    }
                },
                Reference::RadialGradient(g) => {
                    // Same bounding box logic
                    if g.coordinate_mode == BoundingBoxMode::ObjectBoundingBox {
                        ResolvedPaint::RadialGradient(apply_bbox_to_radial(g, element_bbox))
                    } else {
                        ResolvedPaint::RadialGradient(g.clone())
                    }
                },
                Reference::Pattern(p) => {
                    // Pattern can reference other paint servers recursively
                    resolve_pattern(p, element_bbox, registry)
                },
                _ => ResolvedPaint::None,
            }
        }
    }
}
```

#### Per-Element Paint Sequence

Each renderable element produces up to 3 draw operations in order:

```
For each renderable element:
  1. Push a Group with the element's transform
  2. If has clip-path:
       Push Clip(registry.get(clip-path-id))
  3. If has mask:
       Push Mask(registry.get(mask-id))
  4. If has filter:
       Push Filter(registry.get(filter-id))
  5. If fill is not None:
       Emit Fill { path, paint: resolve_paint(fill, bbox), fill_rule }
  6. If stroke is not None:
       Emit Stroke { path, paint: resolve_paint(stroke, bbox), params }
  7. If has markers:
       For each marker:
         Load marker from registry
         Position and orient at path vertices
         Emit marker's content as a Group with position+rotation
  8. If has filter:
       Pop Filter
  9. If has mask:
       Pop Mask
  10. If has clip-path:
        Pop Clip
  11. Pop Group
```

### 2.5 ReferenceRegistry

**Responsibility:** Stores definitions from `<defs>` that are referenced by `url(#id)`.

```rust
pub struct ReferenceRegistry {
    references: HashMap<String, ReferenceDefinition>,
}

pub enum ReferenceDefinition {
    LinearGradient(LinearGradientDef),
    RadialGradient(RadialGradientDef),
    Pattern(PatternDef),
    ClipPath(ClipPathDef),
    Mask(MaskDef),
    Filter(FilterDef),
    Marker(MarkerDef),
    Symbol(SymbolDef),    // used by <use>
}

pub struct LinearGradientDef {
    pub id: String,
    pub x1: LengthValue,
    pub y1: LengthValue,
    pub x2: LengthValue,
    pub y2: LengthValue,
    pub stops: Vec<GradientStop>,
    pub coordinate_mode: CoordinateMode,  // ObjectBoundingBox | UserSpaceOnUse
    pub gradient_transform: Option<AffineTransform>,
    pub spread_method: SpreadMethod,
}

pub struct GradientStop {
    pub offset: f32,
    pub color: AbsoluteColor,
    pub opacity: f32,
}

pub struct ClipPathDef {
    pub id: String,
    pub content: Vec<SvgStyledNode>,  // children that define the clip shape
    pub clip_rule: FillRule,
}
```

**Population:** During the style resolution pass, the engine walks the DOM subtree. When it encounters child elements of `<defs>`, it parses them and stores them in the registry by `id`. `<defs>` children never produce rendering output.

**Resolution:** The PaintBuilder queries the registry during paint command construction.

---

## 3. Data Flow Diagram

```
DOM subtree (with Arc<ComputedValues> from Stylo)
    │
    ▼
┌──────────────────────────────┐
│  1. StyleResolver.resolve()  │
│                              │
│  Walk DOM tree:              │
│  • Extract fill, stroke,     │
│    transform, opacity, ...   │
│    from ComputedValues       │
│  • Record reference URLs     │
│  • Register <defs> children  │
│    in ReferenceRegistry      │
│  • Handle inheritance        │
│    (parent → child cascade)  │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│  2. LayoutEngine.compute()   │
│                              │
│  Walk styled tree:           │
│  • Compute viewBox mapping   │
│    + preserveAspectRatio     │
│  • Maintain transform stack  │
│  • Compute local_bbox per    │
│    element (from geometry)   │
│  • Compute viewport_bbox     │
│    (transform × local_bbox)  │
│  • Propagate bboxes upward   │
│    for parent groups         │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│  3. PaintBuilder.build()     │
│                              │
│  Walk laid-out tree          │
│  in DOM order:               │
│  • For each renderable:      │
│    - Get style + bbox        │
│    - Resolve paint servers   │
│      (registry lookup +      │
│       bbox application)      │
│    - Emit Fill/Stroke/Text   │
│    - Handle clip/mask/filter │
│    - Handle markers          │
│  • Wrap in Group for         │
│    transforms/opacity        │
│  • Flatten to command list   │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│  4. SvgDisplayList           │
│                              │
│  Vec<DisplayCommand>         │
│  ready for Servo display     │
│  list conversion             │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│  5. Display List Conversion  │
│                              │
│  Convert DisplayCommand to   │
│  Servo DisplayItem:          │
│                              │
│  Fill{color}  → push_rect   │
│    or push_mesh with fill    │
│  Fill{grad}   → push_        │
│    gradient with clip mask   │
│  Text{font}   → push_text    │
│  Group{op}    → push_        │
│    stacking_context(opacity) │
│  Clip{path}   → push_clip/   │
│    pop_clip                  │
│  Image{key}   → push_image   │
└──────────────────────────────┘
```

---

## 4. Key Design Decisions

### 4.1 Why a Separate PaintBuilder Pass?

Instead of combining layout and paint (which some SVG renderers do), they are separate passes:

- **Layout is stateless** — it only computes geometry. No paint server resolution, no display list construction.
- **Paint uses layout output** — it needs bounding boxes to resolve `objectBoundingBox` paint servers.
- **Separation enables caching** — layout output can be cached; only the paint pass needs re-execution when styles change.

### 4.2 Why Registry-Based References Instead of Direct Pointers?

- **Circular references** — `<use>` can reference elements that reference other elements. A registry with deferred resolution handles this naturally.
- **Cross-document references** — SVG can reference elements in other SVG documents.
- **Dynamic DOM changes** — Elements can be added/removed; the registry can be updated independently of the paint command list.

### 4.3 How the Engine Integrates with Servo's Display List

The `SvgDisplayList` is NOT Servo's `DisplayList`. It is an intermediate representation that gets converted in one step:

```
SvgDisplayList::DisplayCommand → Servo DisplayItem
```

This keeps the SVG engine decoupled from Servo's internal display list format. The conversion happens in a thin adapter layer within `make_fragments()`.

Each `DisplayCommand` variant maps to one or more Servo display items:

| Svg DisplayCommand | Servo DisplayItem(s) |
|---|---|
| `Fill { path, SolidColor(c) }` | `push_rect()` or `push_mesh()` |
| `Fill { path, Gradient(g) }` | `push_gradient()` with clip to path |
| `Stroke { path, ... }` | Not directly supported → mesh with stroke simulation |
| `Text { font, text, paint }` | `push_text()` with shaped glyphs |
| `Group { children, opacity }` | `push_stacking_context()` with opacity |
| `Clip { id, paths }` | `push_clip() / pop_clip()` |
| `Image { data, rect }` | `push_image()` |

### 4.4 How the Engine Replaces the Current Pipeline

**Current (6 stages, 4 passes):**

```
Stage 1: DOM Construction      (Script)
Stage 2: Layout Traversal       (Layout)   → queue serialization
Stage 3: SVG Serialization      (Script)   → data URL
Stage 4: Cache Wait             (Layout)   → pending → async load
Stage 5: Vector Hit + Rasterize (Layout)   → rasterize
Stage 6: Fragment::Image → GPU  (Layout)   → done
```

**New (1 pass):**

```
SvgEngine::process()
  └── StyleResolver.resolve()    (same pass)
  └── LayoutEngine.compute()     (same pass)
  └── PaintBuilder.build()       (same pass)
  └── SvgDisplayList → Fragment::Svg
```

No serialization. No image cache. No async rasterization. No 4-pass cycle.

---

## 5. Phase 1 Scope (Minimum Viable)

What the engine handles in Phase 1:

### Supported Elements
- `<svg>`, `<g>`, `<defs>` (containers)
- `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`, `<path>` (shapes)

### Supported Properties
- `fill` (solid colors only — no gradients)
- `stroke` (solid colors only)
- `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit`
- `stroke-dasharray`, `stroke-dashoffset`
- `fill-rule` (nonzero, evenodd)
- `fill-opacity`, `stroke-opacity`
- `opacity`
- `transform` (translate, scale, rotate, skewX, skewY, matrix)
- `display`, `visibility`

### Layout
- `viewBox` + `preserveAspectRatio` (meet, slice, none)
- Nested `<svg>` viewports
- Transform stack
- Bounding boxes for all supported shapes

### What Phase 1 Does NOT Include
- Gradients (linear, radial)
- Patterns
- `<text>`
- `<use>`
- `<clipPath>`, `<mask>`, `<filter>`
- Markers
- `<image>`
- Animations
- `<foreignObject>`
- Scripting

---

## 6. Concrete Walkthrough

### Example: `<svg width="200" height="200" viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="blue"/></svg>`

**Step 1 — StyleResolver.resolve():**

```
<svg>   → styles: { }
<circle> → styles: { fill: SolidColor(blue), stroke: None }

Registry: empty (no defs)
```

**Step 2 — LayoutEngine.compute():**

```
viewBox transform:
  scale_x = 200 / 100 = 2
  scale_y = 200 / 100 = 2
  translate = (0, 0)  // xMidYMid centered
  transform = scale(2, 2)

<svg>   → local_bbox: (0, 0, 200, 200),  transform: identity
<circle> → local_bbox: (10, 10, 80, 80),  transform: scale(2,2)
          viewport_bbox: (20, 20, 160, 160)
```

**Step 3 — PaintBuilder.build():**

```
DisplayCommands: [
  Fill {
    path: Circle { cx: 50, cy: 50, r: 40 },
    paint: SolidColor(blue),
    fill_rule: NonZero,
    opacity: 1.0,
  }
]
```

**Step 4 — Display list conversion:**

```
push_stacking_context()  // from <svg> viewport
  push_rect(x=0, y=0, w=200, h=200, color=blue)
pop_stacking_context()
```

No image cache. No serialization. No rasterization. One pass.
