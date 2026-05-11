# SVG Engine — Architecture

> A native SVG rendering engine that replaces the current serialize-as-image approach. Keeps the SVG subtree in the document tree and produces vector display items directly.

---

## 1. System Boundary

### 1.1 Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SVG ENGINE                                         │
│                                                                              │
│  Input:                                                                      │
│    Styled SVG DOM subtree                                                    │
│      (Arc<ComputedValues> on every element,                                  │
│       resolved by Stylo cascade including                                    │
│       parent-inherited properties)                                           │
│                                                                              │
│    Viewport size (Au × Au)                                                   │
│      from the <svg> element's CSS width/height                               │
│                                                                              │
│    Device pixel ratio                                                        │
│      for mapping Au → physical pixels                                        │
│                                                                              │
│  Output:                                                                     │
│    SvgDisplayList — vector paint commands                                    │
│      consumed by Servo's display list builder                                │
│                                                                              │
│  Non-goals (Phase 1):                                                        │
│    • Scripting (<script>, event handlers)                                    │
│    • SMIL animation                                                          │
│    • <foreignObject> compositing                                             │
│    • Full SVG 2 spec compliance                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Input Detail

#### Styled SVG DOM Subtree

The engine receives a tree of SVG elements where every node has its computed styles already resolved by Stylo. This is NOT the raw DOM — it is a prepared tree that the engine can walk without accessing Servo's DOM internals.

```rust
/// A styled SVG node — the fundamental input type to the engine.
/// This is constructed by Servo's layout during the box tree build,
/// by extracting the SVG subtree from the DOM and attaching styles.
pub struct SvgStyledNode {
    /// The type of SVG element (rect, circle, path, g, etc.)
    pub element: SvgElementType,
    /// Resolved CSS values from Stylo. These include:
    /// - Standard CSS properties (display, visibility, opacity)
    /// - SVG-specific properties (fill, stroke, stroke-width, etc.)
    /// - Inherited properties from parent chain
    pub styles: Arc<ComputedValues>,
    /// Child elements in DOM order
    pub children: Vec<SvgStyledNode>,
}

pub enum SvgElementType {
    // Container elements
    Svg(SvgParams),
    G,
    Defs,
    Symbol(SvgParams),
    Use(UseParams),

    // Renderable shape elements
    Rect(RectParams),
    Circle(CircleParams),
    Ellipse(EllipseParams),
    Line(LineParams),
    Polyline(PolylineParams),
    Polygon(PolygonParams),
    Path(PathParams),

    // Renderable content elements
    Text(TextParams),
    Image(ImageParams),

    // Reference elements (never rendered directly)
    LinearGradient,
    RadialGradient,
    Pattern,
    ClipPath,
    Mask,
    Filter,
    Marker,
}

// --- Element parameters ---

pub struct SvgParams {
    /// CSS width/height resolved to app units
    pub width: Option<Au>,
    pub height: Option<Au>,
    /// Raw viewBox string (parsed separately by LayoutEngine)
    pub view_box: Option<String>,
    pub preserve_aspect_ratio: String,
}

pub struct RectParams {
    pub x: LengthValue, pub y: LengthValue,
    pub width: LengthValue, pub height: LengthValue,
    pub rx: LengthValue, pub ry: LengthValue,
}

pub struct CircleParams {
    pub cx: LengthValue, pub cy: LengthValue, pub r: LengthValue,
}

pub struct EllipseParams {
    pub cx: LengthValue, pub cy: LengthValue,
    pub rx: LengthValue, pub ry: LengthValue,
}

pub struct LineParams {
    pub x1: LengthValue, pub y1: LengthValue,
    pub x2: LengthValue, pub y2: LengthValue,
}

pub struct PathParams {
    /// The `d` attribute — SVG path data mini-language
    pub path_data: String,
}

pub struct UseParams {
    pub href: String,      // reference to #id
    pub x: LengthValue, pub y: LengthValue,
    pub width: LengthValue, pub height: LengthValue,
}

pub struct TextParams {
    pub x: Vec<LengthValue>, pub y: Vec<LengthValue>,
    pub dx: Vec<LengthValue>, pub dy: Vec<LengthValue>,
    pub content: String,
    pub text_anchor: TextAnchor,
}

pub struct ImageParams {
    pub href: String,
    pub x: LengthValue, pub y: LengthValue,
    pub width: LengthValue, pub height: LengthValue,
}
```

**How this tree is constructed:** During Servo's box tree construction in `traverse_element()`, when the layout encounters an `<svg>` element, it extracts the SVG subtree from the DOM, resolves each element's type and geometry parameters from DOM attributes, and attaches the `Arc<ComputedValues>` from Stylo. This gives the SVG engine a clean, self-contained tree to work with — no DOM access needed.

#### Viewport Size

The viewport size is the CSS computed width and height of the `<svg>` element, in `Au` (App Units, where 1px = 60Au). This is what Stylo resolves from the CSS cascade:

```rust
pub struct ViewportSize {
    /// CSS computed width of the <svg> element (not the viewBox width)
    pub width: Au,
    /// CSS computed height of the <svg> element
    pub height: Au,
}
```

For `<svg width="200" height="200">`, this is `Au(12000) × Au(12000)`. For percentage values, Stylo already resolved them against the parent containing block before passing to the engine.

#### Device Pixel Ratio

The scale factor from CSS pixels to physical device pixels. Used when the engine needs to produce rasterized content (e.g., for `<image>` elements or when a filter requires pixel-level processing). For most vector commands, this is passed through to WebRender.

```rust
/// Device pixel ratio (e.g., 1.0 for standard, 2.0 for Retina)
pub dpr: Scale<f32, CSSPixel, DevicePixel>,
```

### 1.3 Output Detail

#### SvgDisplayList

The engine output is an intermediate representation — NOT Servo's display list directly. This keeps the SVG engine decoupled from Servo internals.

```rust
pub struct SvgDisplayList {
    /// Flat sequence of display commands in paint order
    pub commands: Vec<DisplayCommand>,
    /// Whether the SVG needs re-rendering next frame (for animations)
    pub is_animating: bool,
    /// The viewport rectangle this display list occupies
    pub viewport_rect: Rect<Au>,
    /// Intrinsic size from viewBox (for CSS sizing calculations)
    pub intrinsic_size: Option<Size2D<Au>>,
}
```

See §2.4 for the full `DisplayCommand` enum.

### 1.4 Error Handling

```rust
pub enum SvgEngineError {
    /// Malformed path data in `d` attribute
    InvalidPathData(String),
    /// Missing referenced element (e.g., url(#missing) that doesn't exist)
    MissingReference(String),
    /// Circular reference in <use> elements
    CircularReference,
    /// Invalid length value
    InvalidLength(String),
    /// viewBox parsing failure
    InvalidViewBox(String),
    /// Paint server resolution failure (e.g., gradient references missing stop)
    PaintServerError(String),
}

pub enum SvgEngineResult<T> {
    Ok(T),
    /// Recoverable error — produce empty output but don't crash
    Recoverable(SvgEngineError),
    /// Fatal error — should not happen with valid SVG
    Fatal(SvgEngineError),
}
```

Recoverable errors (malformed path data, missing references) produce an empty `SvgDisplayList` for that element and continue processing siblings. Fatal errors propagate up to the layout thread.

### 1.5 Threading Model

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Threading Model                                                          │
│                                                                          │
│  Layout Thread (single)                                                  │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  make_fragments() {                                               │   │
│  │    svg_engine.process(root, viewport, dpr)                        │   │
│  │      │                                                            │   │
│  │      ├─ StyleResolver.resolve()     ← synchronous                │   │
│  │      ├─ LayoutEngine.compute()      ← synchronous, single walk   │   │
│  │      ├─ PaintBuilder.build()        ← synchronous, single walk   │   │
│  │      │                                                            │   │
│  │      └─ return SvgDisplayList                                    │   │
│  │  }                                                                │   │
│  │                                                                     │
│  │  → Convert SvgDisplayList → Vec<Fragment>                            │   │
│  │  → Return fragments to box tree builder                              │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  The engine runs entirely on the Layout Thread.                          │
│  No cross-thread communication. No async operations.                     │
│  The entire process() call completes in one synchronous pass.            │
└──────────────────────────────────────────────────────────────────────────┘
```

The engine is stateless between `process()` calls. If the SVG is re-laid out (e.g., due to viewport resize), the engine runs again from scratch with the new viewport size. No state persists between frames except the `ReferenceRegistry`, which can be rebuilt from the DOM subtree.

### 1.6 Lifecycle

```
┌────────────────────────────────────────────────────────────┐
│ Lifecycle                                                   │
│                                                             │
│  Page Load/Resize/Repaint:                                  │
│                                                             │
│  1. Servo's update_the_rendering() fires reflow()          │
│  2. Box tree construction: traverse_element()               │
│     → encounters <svg> → SvgStyledNode constructed          │
│  3. Contents::for_element() → ReplacedContentKind::SVGElement │
│  4. svg_kind_size() → natural sizes + viewport               │
│  5. make_fragments() → SVG_ENGINE.process() called           │
│     a. StyleResolver.resolve() — 1 tree walk                 │
│     b. LayoutEngine.compute() — 1 tree walk                  │
│     c. PaintBuilder.build() — 1 tree walk                    │
│  6. Returns Fragment::Svg { display_list }                   │
│  7. Display list builder converts to Servo DisplayItems      │
│  8. WebRender renders                                        │
│                                                             │
│  Total: 3 synchronous tree walks, 1 pass.                  │
└────────────────────────────────────────────────────────────┘
```

### 1.7 Integration Point

The engine plugs into `make_fragments()` in `layout/replaced.rs`, replacing the current `Fragment::Image` path:

```rust
// Current: serialize → rasterize → ImageKey → Fragment::Image
// New:    svg_engine.process() → SvgDisplayList → Fragment::Svg

fn make_fragments(...) -> Vec<Fragment> {
    match &self.kind {
        // ... existing cases (Image, Video, IFrame, Canvas) ...
        ReplacedContentKind::SVGElement { .. } => {
            // Build the styled subtree from DOM
            let styled_root = build_svg_subtree(node);

            // Run the engine
            let result = SVG_ENGINE.process(
                &styled_root,
                ViewportSize { width, height },
                dpr,
            );

            match result {
                SvgEngineResult::Ok(display_list) => {
                    vec![Fragment::Svg(ArcRefCell::new(SvgFragment {
                        base,
                        display_list,
                    }))]
                }
                SvgEngineResult::Recoverable(display_list, _error) => {
                    // Paint what we can, log the error
                    vec![Fragment::Svg(ArcRefCell::new(SvgFragment {
                        base,
                        display_list,
                    }))]
                }
                SvgEngineResult::Fatal(error) => {
                    // Broken image icon
                    vec![Fragment::Image(broken_image_fragment)]
                }
            }
        }
    }
}
```

Where `SvgFragment` is a new fragment variant added to the `Fragment` enum:

```rust
/// In components/layout/fragment_tree/fragment.rs

pub(crate) enum Fragment {
    Box(ArcRefCell<BoxFragment>),
    Float(ArcRefCell<BoxFragment>),
    Positioning(ArcRefCell<PositioningFragment>),
    AbsoluteOrFixedPositioned(ArcRefCell<HoistedSharedFragment>),
    Text(ArcRefCell<TextFragment>),
    Image(ArcRefCell<ImageFragment>),
    IFrame(ArcRefCell<IFrameFragment>),
    /// NEW: SVG fragment carrying vector display commands
    Svg(ArcRefCell<SvgFragment>),
}

pub(crate) struct SvgFragment {
    pub base: BaseFragment,
    pub display_list: SvgDisplayList,
}
```

### 1.8 Display List Conversion (Adapter Layer)

When the display list builder encounters `Fragment::Svg`, it converts each `DisplayCommand` into Servo `DisplayItem`s:

```rust
// In components/layout/display_list/mod.rs

Fragment::Svg(svg_fragment) => {
    let svg = svg_fragment.borrow();
    for command in &svg.display_list.commands {
        match command {
            DisplayCommand::Fill { path, paint, fill_rule, opacity } => {
                match paint {
                    ResolvedPaint::Solid(color) => {
                        // Convert path to mesh and push as filled rect/region
                        builder.push_mesh(path_to_mesh(path), color, opacity);
                    }
                    ResolvedPaint::Gradient(gradient) => {
                        // Push a gradient with clip to path
                        builder.push_gradient(
                            gradient.start, gradient.end,
                            gradient.stops, Some(clip_path),
                        );
                    }
                    // ...
                }
            }
            DisplayCommand::Group { children, transform, opacity } => {
                builder.push_stacking_context(transform, opacity);
                // Recurse into children
                pop_stacking_context();
            }
            // ...
        }
    }
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

The `SvgEngine` owns all sub-components and drives the three-phase pipeline on every `process()` call. It is created once when the layout thread initializes and reused across frames.

```rust
pub struct SvgEngine {
    /// Resolves SVG styles from ComputedValues
    styles: StyleResolver,
    /// Computes geometry and transforms
    layout: LayoutEngine,
    /// Generates paint commands from laid-out tree
    paint: PaintBuilder,
    /// Stores definitions referenced by url(#id)
    registry: ReferenceRegistry,
    /// Debug: enables trace output
    debug_trace: bool,
}

impl SvgEngine {
    /// Create a new SVG engine instance.
    pub fn new() -> Self {
        Self {
            styles: StyleResolver::new(),
            layout: LayoutEngine::new(),
            paint: PaintBuilder::new(),
            registry: ReferenceRegistry::new(),
            debug_trace: false,
        }
    }

    /// Process a styled SVG subtree for one frame.
    ///
    /// Called from make_fragments() on the layout thread.
    /// The tree walk order is:
    ///   Pass 1 (StyleResolver):  top-down inheritance + reference registration
    ///   Pass 2 (LayoutEngine):   top-down transform accumulation + bbox computation
    ///   Pass 3 (PaintBuilder):   top-down paint command emission
    ///
    /// Each pass walks the entire tree once. Total: 3 synchronous tree walks.
    pub fn process(
        &mut self,
        root: &SvgStyledNode,
        viewport: ViewportSize,
        dpr: Scale<f32, CSSPixel, DevicePixel>,
    ) -> SvgEngineResult<SvgDisplayList> {
        if self.debug_trace {
            eprintln!("[SVG_ENGINE] process() start viewport={:?}", viewport);
        }

        // Phase 1: Resolve styles + populate registry
        let mut inherit_state = InheritState::default();
        if let Err(e) = self.styles.resolve(root, &mut inherit_state, &mut self.registry) {
            return SvgEngineResult::Fatal(e);
        }

        // Phase 2: Layout — compute transforms + bounding boxes
        let mut transform_stack = TransformStack::new();
        let viewbox_transform = self.layout.compute_viewbox_transform(
            viewport,
            root,
        );
        transform_stack.push(viewbox_transform);

        if let Err(e) = self.layout.layout_tree(root, &mut transform_stack) {
            return SvgEngineResult::Recoverable(SvgDisplayList::empty(), e);
        }
        transform_stack.pop();

        // Phase 3: Paint — generate display commands
        let mut commands = Vec::new();
        if let Err(e) = self.paint.build_tree(
            root,
            &self.styles.styles,
            &self.layout.layouts,
            &self.registry,
            &mut commands,
        ) {
            return SvgEngineResult::Recoverable(SvgDisplayList::empty(), e);
        }

        if self.debug_trace {
            eprintln!("[SVG_ENGINE] process() done: {} commands", commands.len());
        }

        SvgEngineResult::Ok(SvgDisplayList {
            commands,
            is_animating: false,
            viewport_rect: viewport.into(),
            intrinsic_size: root.intrinsic_size(),
        })
    }
}
```

#### Stateful vs Stateless

| Component | Stateful? | Reason |
|-----------|-----------|--------|
| `StyleResolver` | No per-frame state | All state is in the `HashMap` passed back. The resolver itself holds no frame-to-frame data. |
| `LayoutEngine` | No per-frame state | Same — layout output is in the returned `HashMap`. |
| `PaintBuilder` | Pure | Stateless function of its inputs. |
| `ReferenceRegistry` | **Yes** | Registry is populated fresh each frame from the DOM subtree. Could be cached if DOM hasn't changed. |

#### Debug Tracing

The engine supports per-frame debug output controlled by a flag or environment variable:

```rust
impl SvgEngine {
    pub fn enable_debug_trace(&mut self) {
        self.debug_trace = true;
    }

    pub fn disable_debug_trace(&mut self) {
        self.debug_trace = false;
    }
}
```

When enabled, the engine prints:
- Number of elements processed per phase
- viewBox → viewport mapping math
- Paint server resolution steps (which reference resolved to what)
- Missing references
- Final command count

### 2.2 StyleResolver

**Responsibility:** Given a DOM subtree with `Arc<ComputedValues>` on each node, extract SVG-specific properties into a flat struct. Also populates the `ReferenceRegistry` by registering `<defs>` children.

```rust
pub struct StyleResolver {
    /// Map from element pointer/ID → resolved styles
    pub(crate) styles: HashMap<*const SvgStyledNode, ResolvedSvgStyles>,
    /// Temporary stack for inheritance tracking
    inherit_stack: Vec<InheritState>,
}

/// Values that cascade from parent to child in SVG
#[derive(Clone)]
pub struct InheritState {
    pub fill: PaintServerValue,
    pub stroke: PaintServerValue,
    pub fill_opacity: f32,
    pub stroke_opacity: f32,
    pub fill_rule: FillRule,
    pub stroke_width: Length,
    pub stroke_linecap: LineCap,
    pub stroke_linejoin: LineJoin,
    pub stroke_miterlimit: f32,
    pub font_family: FontFamily,
    pub font_size: Length,
    pub opacity: f32,
    pub visibility: Visibility,
}

impl Default for InheritState {
    fn default() -> Self {
        Self {
            fill: PaintServerValue::Solid(Color::black),  // SVG default
            stroke: PaintServerValue::None,                // SVG default
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            fill_rule: FillRule::NonZero,
            stroke_width: Length::Px(1.0),
            stroke_linecap: LineCap::Butt,
            stroke_linejoin: LineJoin::Miter,
            stroke_miterlimit: 4.0,
            font_family: FontFamily::Serif,
            font_size: Length::Px(16.0),
            opacity: 1.0,
            visibility: Visibility::Visible,
        }
    }
}
```

#### Full Resolved Style

```rust
pub struct ResolvedSvgStyles {
    // Fill (from fill CSS property or presentation attribute)
    pub fill: PaintServerValue,
    pub fill_opacity: f32,
    pub fill_rule: FillRule,

    // Stroke
    pub stroke: PaintServerValue,
    pub stroke_opacity: f32,
    pub stroke_width: Length,
    pub stroke_linecap: LineCap,
    pub stroke_linejoin: LineJoin,
    pub stroke_miterlimit: f32,
    pub stroke_dasharray: Vec<Length>,
    pub stroke_dashoffset: Length,

    // Transform (from transform CSS property or presentation attribute)
    pub transform: Option<Transform>,

    // Visibility
    pub display: Display,
    pub visibility: Visibility,

    // References to other elements
    pub clip_path: Option<String>,       // the id part of url(#id)
    pub mask: Option<String>,
    pub filter: Option<String>,
    pub marker_start: Option<String>,
    pub marker_mid: Option<String>,
    pub marker_end: Option<String>,

    // Text properties
    pub font_family: FontFamily,
    pub font_size: Length,
    pub font_style: FontStyle,
    pub font_weight: FontWeight,
    pub text_anchor: TextAnchor,
    pub dominant_baseline: Baseline,

    // Compositing
    pub opacity: f32,
}

/// How a paint server value is represented before resolution
pub enum PaintServerValue {
    /// fill="none" / stroke="none"
    None,
    /// fill="red" / stroke="#ff0000" / stroke="currentColor"
    Solid(AbsoluteColor),
    /// fill="url(#myGradient)" — stores the reference id
    Url(String),
    /// fill="currentColor"
    CurrentColor,
}
```

#### Tree Walk Algorithm

The resolver walks the tree top-down, depth-first. At each node:

```
resolve(node, inherit_state, registry):
  1. If node is <defs>:
       For each child of <defs>:
         Parse the child element
         Register it in registry by its id attribute
       Skip children (they are not rendered)
       Return (no styles for <defs> itself)

  2. If node is a reference element (gradient, clipPath, etc.):
       Parse and register in registry
       Return (no rendering styles)

  3. For renderable/container elements:
       a. Extract fill from ComputedValues:
            - If ComputedValues has explicit fill → use it
            - Else → inherit from parent's inherit_state.fill
            - Parse the fill value:
              "red"         → PaintServerValue::Solid(red)
              "url(#g1)"    → PaintServerValue::Url("g1")
              "currentColor" → resolve currentColor from ComputedValues
              "none"        → PaintServerValue::None
              "inherit"     → use parent value

       b. Extract stroke similarly (default: none)

       c. Extract all other SVG properties from ComputedValues:
            stroke-width, stroke-linecap, opacity, transform, etc.

       d. Extract reference URLs (clip-path, mask, filter, markers):
            "url(#clip)" → Some("clip")
            "none"       → None

       e. Store resolved style in self.styles[node_ptr]

       f. Update inherit_state with this element's values
          (children inherit from this element)

       g. Recurse to children

       h. Restore inherit_state to previous values
          (siblings don't inherit from each other)
```

#### Paint Server Reference Resolution

```
fill="url(#myGradient)"  →  PaintServerValue::Url("myGradient")
fill="red"               →  PaintServerValue::Solid(Color::red)
fill="#ff0000"           →  PaintServerValue::Solid(Color::from_hex(#ff0000))
fill="none"              →  PaintServerValue::None
fill="currentColor"      →  PaintServerValue::CurrentColor (resolved at paint time)
fill="inherit"           →  (use parent's value)
```

The StyleResolver does NOT resolve `Url("myGradient")` to the actual gradient definition. It just records the reference id. The PaintBuilder does full resolution, because:
- The element's bounding box must be computed first (for objectBoundingBox mode)
- The gradient definition lives in the ReferenceRegistry

#### Inheritance Details

SVG property inheritance follows the CSS cascade with SVG-specific defaults:

| Property | Initial Value | Inherited? |
|----------|--------------|------------|
| `fill` | black | Yes |
| `stroke` | none | Yes |
| `stroke-width` | 1px | Yes |
| `stroke-linecap` | butt | Yes |
| `stroke-linejoin` | miter | Yes |
| `fill-rule` | nonzero | Yes |
| `opacity` | 1 | No (but applies to element and children compositing) |
| `visibility` | visible | Yes |
| `font-family` | depends on UA | Yes |
| `font-size` | medium (16px) | Yes |

**Cascade order (lowest to highest priority):**
1. Inherited value from parent element
2. SVG presentation attribute (e.g., `fill="red"` on the element)
3. CSS inline style (`style="fill: red;"`)
4. CSS class/style rule (from `<style>` or external stylesheet)
5. CSS `!important` rules

Since Stylo already computes the full cascade before the SVG engine sees the tree, the engine receives the final computed value. It does NOT need to implement its own cascade — just inheritance fallback.

### 2.3 LayoutEngine

**Responsibility:** Compute the spatial geometry of every element: bounding boxes, viewport transforms, and the accumulated affine transform from root to element screen space.

```rust
pub struct LayoutEngine {
    /// Per-element layout data computed during the tree walk
    pub(crate) layouts: HashMap<*const SvgStyledNode, ElementLayout>,
}

/// The geometry result for a single element
pub struct ElementLayout {
    /// Bounding box in the element's own local coordinate space
    /// (before any transforms are applied)
    pub local_bbox: Rect<f32>,

    /// Bounding box in viewport (screen) coordinate space
    /// (after all ancestor transforms including viewBox)
    pub viewport_bbox: Rect<f32>,

    /// Accumulated transform from this element's local space to viewport space
    /// Element point P_viewport = transform * P_local
    pub transform: AffineTransform,

    /// This element's own transform attribute (translate, scale, rotate, ...)
    /// Does NOT include parent transforms
    pub local_transform: AffineTransform,
}
```

#### The Transform Stack

The engine maintains a stack of affine transforms as it walks the tree depth-first. Each element's accumulated transform is the product of all active transforms from the root to that element.

```
Transform layer order (applied right-to-left):
  viewport_bbox = viewport_transform
                ∘ svg_viewbox_transform
                ∘ group_transform
                ∘ element_local_transform
                ∘ local_bbox
```

```rust
struct TransformStack {
    /// The stack of transforms. Index 0 = root (closest to viewport).
    stack: Vec<AffineTransform>,
    /// Cached product of all transforms on the stack.
    /// Updated on push/pop to avoid recomputing each time.
    current: AffineTransform,
}

impl TransformStack {
    fn new() -> Self {
        Self {
            stack: vec![AffineTransform::identity()],
            current: AffineTransform::identity(),
        }
    }

    /// Push a new transform. It is applied AFTER the current transform.
    /// new_point = current_transform * new_transform * point
    fn push(&mut self, t: AffineTransform) {
        self.current = self.current.then(&t);
        self.stack.push(t);
    }

    /// Pop the most recent transform.
    fn pop(&mut self) {
        if let Some(t) = self.stack.pop() {
            // Recompute current from scratch
            self.current = AffineTransform::identity();
            for t in &self.stack {
                self.current = self.current.then(t);
            }
        }
    }

    /// Current accumulated transform
    fn current(&self) -> &AffineTransform {
        &self.current
    }
}
```

#### Tree Walk Algorithm

```
layout_tree(node, transform_stack):
  1. If node is <defs> or a reference element:
       Skip — no layout needed (already registered)

  2. Apply this element's transform:
       element_transform = parse_transform(node.styles.transform)

  3. Push element_transform onto transform_stack

  4. Compute local_bbox from element geometry:
       match node.element:
         Rect(r)     → bbox = (r.x, r.y, r.width, r.height)
         Circle(c)   → bbox = (c.cx - c.r, c.cy - c.r, 2*r, 2*r)
         Ellipse(e)  → bbox = (e.cx - e.rx, e.cy - e.ry, 2*rx, 2*ry)
         Line(l)     → bbox = (min(x1,x2), min(y1,y2), |dx|, |dy|)
         Polyline(p) → bbox = bounding box of all points
         Polygon(p)  → bbox = bounding box of all points
         Path(p)     → bbox = compute_path_bbox(p.path_data)
         Text(t)     → bbox = compute_text_bbox(t, font_metrics)
         G           → bbox = union of children's bboxes (computed later)
         Use(u)      → bbox = bbox of referenced element + (u.x, u.y)

  5. Compute viewport_bbox:
       viewport_bbox = transform_stack.current() ∘ local_bbox

  6. Store ElementLayout { local_bbox, viewport_bbox, transform, local_transform }

  7. Recurse to children (depth-first)

  8. For <g>: update local_bbox to union of children's viewport bboxes
     (This requires a two-pass approach: first compute children,
      then update the group's bbox.)

  9. Pop transform_stack
```

#### viewBox Mapping

The viewBox maps user-space coordinates to the viewport. This is the first transform pushed onto the stack (after any CSS transform on the `<svg>` element).

```
scale_x = viewport_width  / viewBox_width
scale_y = viewport_height / viewBox_height
```

```rust
fn compute_viewbox_transform(
    viewport: Size2D<f32>,
    viewbox: Rect<f32>,
    preserve_aspect_ratio: &PreserveAspectRatio,
) -> AffineTransform {
    // Step 1: Compute base scale
    let scale_x = viewport.width / viewbox.width;
    let scale_y = viewport.height / viewbox.height;

    // Step 2: Apply meet/slice
    let scale = match preserve_aspect_ratio.align {
        Align::None => Vector2D::new(scale_x, scale_y),
        _ => {
            let s = if preserve_aspect_ratio.meet_or_slice == MeetOrSlice::Meet {
                scale_x.min(scale_y)  // letterbox — everything visible
            } else {
                scale_x.max(scale_y)  // crop — fill viewport entirely
            };
            Vector2D::new(s, s)  // uniform scale
        }
    };

    // Step 3: Apply alignment offset
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

    AffineTransform::translation(tx, ty)
        .then(&AffineTransform::scale(scale.x, scale.y))
}
```

**preserveAspectRatio values:**

| Attribute | Scale | Align X | Align Y | Behavior |
|-----------|-------|---------|---------|----------|
| `xMidYMid meet` (default) | min(sx,sy) | center | center | Letterbox, centered |
| `xMinYMin meet` | min(sx,sy) | left | top | Letterbox, top-left |
| `xMaxYMax meet` | min(sx,sy) | right | bottom | Letterbox, bottom-right |
| `xMidYMid slice` | max(sx,sy) | center | center | Crop, centered |
| `none` | (sx, sy) | n/a | n/a | Stretch to fill |

#### Bounding Box by Element Type

```
Element       Local Bounding Box
─────────────────────────────────────────────────────────────
<rect>        (x, y, width, height)
              If rx/ry > 0, same bbox — rounding doesn't
              change the extent

<circle>      (cx - r, cy - r, 2*r, 2*r)

<ellipse>     (cx - rx, cy - ry, 2*rx, 2*ry)

<line>        (min(x1,x2), min(y1,y2), |x2-x1|, |y2-y1|)

<polyline>    (min_x, min_y, max_x - min_x, max_y - min_y)
              where min/max over all points in the points list

<polygon>     Same as polyline

<path>        Compute from path segments:
              - MoveTo: update cursor
              - LineTo: expand bbox to include endpoint
              - CurveTo: expand bbox to include bezier control
                points and endpoint (conservative)
              - ArcTo: expand bbox to include arc extents
              - ClosePath: line to start point

<g>           Union of all children's viewport_bbox
              (computed after children are processed)

<use>         Bbox of the referenced element + offset (x, y)
              (references resolved via registry)

<text>        Requires font metrics:
              width = sum of glyph advances
              height = font ascent + descent + leading
              bbox = (x, y - ascent, width, ascent + descent)
```

#### Path Bounding Box (Conservative)

For `<path>` elements, the bounding box is computed by iterating through path segments and tracking the min/max X and Y:

```rust
fn compute_path_bbox(path_data: &str) -> Rect<f32> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut cursor = Point2D::zero();

    for segment in parse_path_commands(path_data) {
        match segment {
            MoveTo(p) => { cursor = p; },
            LineTo(p) => {
                expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, p);
                cursor = p;
            },
            CurveTo(c1, c2, p) => {
                // Include control points for conservative bbox
                expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, c1);
                expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, c2);
                expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, p);
                cursor = p;
            },
            ArcTo(rx, ry, x_axis_rotation, large_arc, sweep, p) => {
                // Include arc endpoints and extents
                expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, p);
                // Complex arcs may extend beyond endpoints —
                // compute critical points from ellipse extents
                for critical in compute_arc_extremities(cursor, rx, ry, x_axis_rotation, large_arc, sweep, p) {
                    expand(&mut min_x, &mut min_y, &mut max_x, &mut max_y, critical);
                }
                cursor = p;
            },
            ClosePath => { /* line to start of current subpath */ },
        }
    }

    Rect::new(Point2D::new(min_x, min_y),
              Size2D::new(max_x - min_x, max_y - min_y))
}
```

#### Nested Viewports

An `<svg>` inside an SVG creates a new viewport with its own coordinate system:

```
<svg width="400" height="400">                    ← viewport 1 (root)
  <svg x="50" y="50" width="100" height="100"      ← viewport 2 (nested)
       viewBox="0 0 10 10">
    <rect x="2" y="2" width="6" height="6"/>       ← in 10×10 user space
  </svg>
</svg>
```

The nested `<svg>` pushes an additional viewport transform onto the stack:

```rust
fn layout_svg_element(
    element: &SvgStyledNode,
    transform_stack: &mut TransformStack,
    layouts: &mut HashMap<*const SvgStyledNode, ElementLayout>,
) {
    // Step 1: Compute viewport size from the <svg>'s CSS width/height
    let viewport_size = Size2D::new(
        element.svg_params.width.unwrap_or(Au::from_px(300)),  // SVG default
        element.svg_params.height.unwrap_or(Au::from_px(150)),
    );

    // Step 2: Compute viewBox transform
    if let Some(ref viewbox) = element.svg_params.view_box {
        let parsed_viewbox = parse_viewbox(viewbox);
        let par = parse_preserve_aspect_ratio(&element.svg_params.preserve_aspect_ratio);
        let vb_transform = compute_viewbox_transform(
            viewport_size.map(|a| a.to_f32_px()),
            parsed_viewbox,
            &par,
        );
        transform_stack.push(vb_transform);
    }

    // Step 3: (x, y) offset for the nested viewport
    let translate = AffineTransform::translation(
        element.svg_params.x.unwrap_or(0.0),
        element.svg_params.y.unwrap_or(0.0),
    );
    transform_stack.push(translate);

    // Step 4: Layout children in this new coordinate space
    for child in &element.children {
        layout_element(child, transform_stack, layouts);
    }

    // Step 5: Restore stack
    transform_stack.pop();  // pop translate
    transform_stack.pop();  // pop viewBox transform
}
```

### 2.4 PaintBuilder

**Responsibility:** Walk the laid-out, styled SVG element tree in DOM order and produce a flat sequence of display commands for Servo's display list builder.

```rust
pub struct PaintBuilder {
    /// Stroke simulation resolution (for non-native stroke support)
    stroke_tolerance: f32,
}

impl PaintBuilder {
    pub fn new() -> Self {
        Self {
            stroke_tolerance: 0.5,  // default
        }
    }

    /// Walk the laid-out tree and produce display commands.
    ///
    /// The tree is walked in DOM order (depth-first, children in order).
    /// This matches SVG's painter's algorithm — first in DOM = painted first.
    ///
    /// Returns a flat list of DisplayCommands in paint order.
    pub fn build_tree(
        &self,
        node: &SvgStyledNode,
        styles: &HashMap<*const SvgStyledNode, ResolvedSvgStyles>,
        layouts: &HashMap<*const SvgStyledNode, ElementLayout>,
        registry: &ReferenceRegistry,
        commands: &mut Vec<DisplayCommand>,
    ) -> Result<(), SvgEngineError>;
}
```

#### SvgDisplayList and DisplayCommand

```rust
/// The output of the SVG engine — a flat sequence of paint commands
/// that can be converted to Servo DisplayItems.
pub struct SvgDisplayList {
    /// Paint commands in order (painter's algorithm order)
    pub commands: Vec<DisplayCommand>,
    /// True if any animation is active (engine should be called next frame)
    pub is_animating: bool,
    /// The viewport rectangle filled by this SVG
    pub viewport_rect: Rect<Au>,
    /// Intrinsic size from viewBox (used by CSS sizing)
    pub intrinsic_size: Option<Size2D<Au>>,
}

impl SvgDisplayList {
    pub fn empty() -> Self {
        Self {
            commands: vec![],
            is_animating: false,
            viewport_rect: Rect::zero(),
            intrinsic_size: None,
        }
    }
}

/// A single paint command in the SVG engine's intermediate representation.
/// These are NOT Servo DisplayItems — they are converted in an adapter layer.
pub enum DisplayCommand {
    /// Fill a path with a solid color, gradient, or pattern
    Fill {
        path: PathData,
        paint: ResolvedPaint,
        fill_rule: FillRule,
        opacity: f32,
    },

    /// Stroke a path (outline) with a paint
    Stroke {
        path: PathData,
        paint: ResolvedPaint,
        params: StrokeParams,
        opacity: f32,
    },

    /// A group with shared transform and opacity.
    /// Children are in paint order.
    Group {
        children: Vec<DisplayCommand>,
        transform: AffineTransform,
        /// Group opacity (applied to the composite of all children)
        opacity: f32,
    },

    /// Define a clipping path for subsequent commands
    ClipDefine {
        id: ClipId,
        paths: Vec<PathData>,
        fill_rule: FillRule,
    },

    /// Activate a previously defined clipping path
    ClipApply {
        id: ClipId,
        children: Vec<DisplayCommand>,
    },

    /// Apply a mask (luminance-to-alpha)
    Mask {
        id: MaskId,
        content: Vec<DisplayCommand>,
        mask_content: Vec<DisplayCommand>,
    },

    /// Apply a filter effect
    Filter {
        id: FilterId,
        primitives: Vec<FilterPrimitive>,
        children: Vec<DisplayCommand>,
    },

    /// Render text
    Text {
        position: Point2D<f32>,
        text: String,
        font: FontDescriptor,
        paint: ResolvedPaint,
    },

    /// Render an embedded raster image
    Image {
        data: ImageData,
        rect: Rect<f32>,
    },

    /// Clear the current render target (for <svg> viewport background)
    Clear {
        rect: Rect<f32>,
        color: AbsoluteColor,
    },
}

/// Resolved paint server — ready for display list generation
pub enum ResolvedPaint {
    None,
    Solid(AbsoluteColor),
    Gradient {
        start: Point2D<f32>,
        end: Point2D<f32>,
        stops: Vec<GradientStop>,
        spread: SpreadMethod,
    },
    RadialGradient {
        center: Point2D<f32>,
        radius: f32,
        stops: Vec<GradientStop>,
        spread: SpreadMethod,
    },
}

pub struct StrokeParams {
    pub width: f32,
    pub linecap: LineCap,
    pub linejoin: LineJoin,
    pub miter_limit: f32,
    pub dash_array: Vec<f32>,
    pub dash_offset: f32,
}

pub struct PathData {
    pub segments: Vec<PathSegment>,
}

pub enum PathSegment {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    CurveTo { cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32 },
    QuadTo { cx: f32, cy: f32, x: f32, y: f32 },
    ArcTo { rx: f32, ry: f32, x_axis_rotation: f32, large_arc: bool, sweep: bool, x: f32, y: f32 },
    ClosePath,
}
```

#### Per-Element Paint Sequence

Each renderable element produces up to 3 draw operations (fill, stroke, markers), wrapped in clip/mask/filter groups as needed.

```
build_tree(node, styles, layouts, registry, commands):
  1. If node is <defs> or a reference element → skip (already registered)

  2. Get style = styles[node]
  3. Get layout = layouts[node]

  4. If style.display == none → skip
  5. If node is <g> or <svg> (container):
       → Recurse to children
       → return

  6. // Start building commands for this element

  7. Let element_commands = Vec::new()

  8. // Compute path from geometry
  9. Let path = compute_path(node)

  10. // Resolve paint servers
  11. Let bbox = layout.viewport_bbox
  12. Let fill_paint = resolve_paint(style.fill, bbox, registry)
  13. Let stroke_paint = resolve_paint(style.stroke, bbox, registry)

  14. // Emit fill (if not none)
  15. if fill_paint != None && style.fill_opacity > 0.0 {
  16.   element_commands.push(Fill { path, fill_paint, style.fill_rule, style.fill_opacity * style.opacity })
  17. }

  18. // Emit stroke (if not none)
  19. if stroke_paint != None && style.stroke_opacity > 0.0 {
  20.   element_commands.push(Stroke { path, stroke_paint, style.stroke_params, style.stroke_opacity * style.opacity })
  21. }

  22. // Emit markers (if any)
  23. if has_markers(style) {
  24.   for marker_cmd in build_markers(node, style, path, registry) {
  25.     element_commands.push(marker_cmd)
  26.   }
  27. }

  28. // Wrap in clip if needed
  29. if let Some(clip_id) = style.clip_path {
  30.   let clip_def = registry.get_clip(clip_id)
  31.   element_commands = vec![ClipApply {
  32.     id: clip_id,
  33.     children: element_commands
  34.   }]
  35. }

  36. // Wrap in mask if needed
  37. if let Some(mask_id) = style.mask {
  38.   let mask_def = registry.get_mask(mask_id)
  39.   element_commands = vec![Mask {
  40.     id: mask_id,
  41.     content: element_commands,
  42.     mask_content: mask_def.content,
  43.   }]
  44. }

  45. // Wrap in filter if needed
  46. if let Some(filter_id) = style.filter {
  47.   element_commands = vec![Filter {
  48.     id: filter_id,
  49.     primitives: registry.get_filter(filter_id).primitives,
  50.     children: element_commands,
  51.   }]
  52. }

  53. // Apply element transform and opacity
  54. if !layout.transform.is_identity() || style.opacity < 1.0 {
  55.   commands.push(Group {
  56.     children: element_commands,
  57.     transform: layout.transform,
  58.     opacity: style.opacity,
  59.   })
  60. } else {
  61.   commands.extend(element_commands)
  62. }
```

#### Paint Server Resolution

This is the hardest part. Paint servers are stored in the `ReferenceRegistry` by id and resolved when the PaintBuilder encounters them — after bounding boxes are available.

```rust
fn resolve_paint(
    value: &PaintServerValue,
    element_bbox: Rect<f32>,
    registry: &ReferenceRegistry,
) -> ResolvedPaint {
    match value {
        PaintServerValue::None => ResolvedPaint::None,
        PaintServerValue::Solid(color) => ResolvedPaint::Solid(*color),
        PaintServerValue::CurrentColor => {
            // Resolved at style resolution time — treat as solid
            // (currentColor is already resolved by Stylo)
            ResolvedPaint::Solid(Color::black)  // placeholder
        }
        PaintServerValue::Url(id) => {
            match registry.get(id) {
                Some(ReferenceDefinition::LinearGradient(g)) => {
                    let (start, end) = if g.coordinate_mode == CoordinateMode::ObjectBoundingBox {
                        // Map gradient coordinates from [0,1] to element's bbox
                        (Point2D::new(
                            element_bbox.min.x + g.x1.as_fraction() * element_bbox.width(),
                            element_bbox.min.y + g.y1.as_fraction() * element_bbox.height(),
                        ), Point2D::new(
                            element_bbox.min.x + g.x2.as_fraction() * element_bbox.width(),
                            element_bbox.min.y + g.y2.as_fraction() * element_bbox.height(),
                        ))
                    } else {
                        // userSpaceOnUse — coordinates are absolute in user space
                        (Point2D::new(g.x1.to_px(), g.y1.to_px()),
                         Point2D::new(g.x2.to_px(), g.y2.to_px()))
                    };

                    ResolvedPaint::Gradient {
                        start,
                        end,
                        stops: g.stops.clone(),
                        spread: g.spread_method,
                    }
                }
                // ... similar for RadialGradient, Pattern ...
                None => {
                    // Missing reference → treat as none (SVG spec: no paint)
                    eprintln!("[SVG_ENGINE] Warning: missing reference '{}'", id);
                    ResolvedPaint::None
                }
            }
        }
    }
}

/// Apply objectBoundingBox gradient coordinates to an element's bounding box
fn apply_bbox_to_gradient(
    gradient: &LinearGradientDef,
    bbox: Rect<f32>,
) -> (Point2D<f32>, Point2D<f32>) {
    let start = Point2D::new(
        bbox.min.x + gradient.x1.as_fraction() * bbox.width(),
        bbox.min.y + gradient.y1.as_fraction() * bbox.height(),
    );
    let end = Point2D::new(
        bbox.min.x + gradient.x2.as_fraction() * bbox.width(),
        bbox.min.y + gradient.y2.as_fraction() * bbox.height(),
    );
    (start, end)
}
```

### 2.5 ReferenceRegistry

**Responsibility:** Stores definitions from `<defs>` that are referenced by `url(#id)` in paint servers, clip paths, masks, filters, and markers. Populated during the style resolution pass, queried during the paint pass.

```rust
pub struct ReferenceRegistry {
    /// Map from id string → reference definition
    references: HashMap<String, ReferenceDefinition>,
    /// Circular reference detection (tracks currently-resolving ids)
    resolving: HashSet<String>,
}

impl ReferenceRegistry {
    pub fn new() -> Self {
        Self {
            references: HashMap::new(),
            resolving: HashSet::new(),
        }
    }

    /// Register a reference definition
    pub fn register(&mut self, id: String, def: ReferenceDefinition) {
        self.references.insert(id, def);
    }

    /// Look up a reference by id
    pub fn get(&self, id: &str) -> Option<&ReferenceDefinition> {
        self.references.get(id)
    }

    /// Check if an id exists
    pub fn contains(&self, id: &str) -> bool {
        self.references.contains_key(id)
    }
}
```

#### All Reference Types

```rust
/// Everything that can be stored in <defs> and referenced by url(#id)
pub enum ReferenceDefinition {
    LinearGradient(LinearGradientDef),
    RadialGradient(RadialGradientDef),
    Pattern(PatternDef),
    ClipPath(ClipPathDef),
    Mask(MaskDef),
    Filter(FilterDef),
    Marker(MarkerDef),
    Symbol(SymbolDef),
}

/// Linear gradient definition
pub struct LinearGradientDef {
    pub id: String,
    pub x1: LengthValue,           // gradient vector start X
    pub y1: LengthValue,           // gradient vector start Y
    pub x2: LengthValue,           // gradient vector end X
    pub y2: LengthValue,           // gradient vector end Y
    pub stops: Vec<GradientStop>,  // color stops in order
    pub coordinate_mode: CoordinateMode,
    pub gradient_transform: Option<AffineTransform>,
    pub spread_method: SpreadMethod,  // pad | reflect | repeat
}

/// Radial gradient definition
pub struct RadialGradientDef {
    pub id: String,
    pub cx: LengthValue,           // center X
    pub cy: LengthValue,           // center Y
    pub r: LengthValue,            // radius
    pub fx: LengthValue,           // focal point X
    pub fy: LengthValue,           // focal point Y
    pub stops: Vec<GradientStop>,
    pub coordinate_mode: CoordinateMode,
    pub gradient_transform: Option<AffineTransform>,
    pub spread_method: SpreadMethod,
}

pub struct GradientStop {
    pub offset: f32,               // 0.0 to 1.0 (or empty for auto-distribute)
    pub color: AbsoluteColor,
    pub opacity: f32,
}

/// Pattern definition
pub struct PatternDef {
    pub id: String,
    pub x: LengthValue,
    pub y: LengthValue,
    pub width: LengthValue,
    pub height: LengthValue,
    pub pattern_units: CoordinateMode,
    pub content: Vec<SvgStyledNode>,    // pattern child elements
}

/// Clip path definition
pub struct ClipPathDef {
    pub id: String,
    pub content: Vec<SvgStyledNode>,    // children defining the clip shape
    pub clip_rule: FillRule,           // nonzero | evenodd
}

/// Mask definition
pub struct MaskDef {
    pub id: String,
    pub x: LengthValue,                // mask bounds
    pub y: LengthValue,
    pub width: LengthValue,
    pub height: LengthValue,
    pub mask_units: CoordinateMode,
    pub mask_content_units: CoordinateMode,
    pub content: Vec<SvgStyledNode>,   // children rendered as luminance mask
}

/// Filter effect definition
pub struct FilterDef {
    pub id: String,
    pub x: LengthValue,                // filter bounds
    pub y: LengthValue,
    pub width: LengthValue,
    pub height: LengthValue,
    pub filter_units: CoordinateMode,
    pub primitives: Vec<FilterPrimitive>,
}

/// A single primitive in a filter pipeline
pub enum FilterPrimitive {
    GaussianBlur {
        std_deviation: f32,
        input: FilterInput,
        result: String,
    },
    ColorMatrix {
        matrix: [f32; 20],             // 4×5 color matrix
        input: FilterInput,
        result: String,
    },
    DropShadow {
        dx: f32, dy: f32,
        std_deviation: f32,
        color: AbsoluteColor,
        input: FilterInput,
        result: String,
    },
    Blend {
        mode: BlendMode,
        input1: FilterInput,
        input2: FilterInput,
        result: String,
    },
    Composite {
        operator: CompositeOperator,
        input1: FilterInput,
        input2: FilterInput,
        result: String,
    },
    Offset {
        dx: f32, dy: f32,
        input: FilterInput,
        result: String,
    },
    Flood {
        color: AbsoluteColor,
        result: String,
    },
    Merge {
        inputs: Vec<FilterInput>,
        result: String,
    },
}

pub enum FilterInput {
    SourceGraphic,     // the element being filtered
    SourceAlpha,       // alpha channel of the element
    BackgroundImage,   // the backdrop
    BackgroundAlpha,   // alpha of backdrop
    Result(String),    // output of a previous primitive
}

/// Marker definition (arrowheads, bullets at path vertices)
pub struct MarkerDef {
    pub id: String,
    pub viewbox: Option<Rect<f32>>,
    pub ref_x: LengthValue,      // alignment point
    pub ref_y: LengthValue,
    pub marker_width: LengthValue,
    pub marker_height: LengthValue,
    pub orient: MarkerOrient,    // auto | fixed-angle
    pub content: Vec<SvgStyledNode>,
}

pub enum MarkerOrient {
    Auto,              // rotate to match path direction
    Angle(f32),        // fixed rotation
}

/// Symbol definition (reusable viewport, used by <use>)
pub struct SymbolDef {
    pub id: String,
    pub viewbox: Option<Rect<f32>>,
    pub preserve_aspect_ratio: String,
    pub content: Vec<SvgStyledNode>,
}

pub enum CoordinateMode {
    ObjectBoundingBox,  // values 0..1 relative to element's bbox
    UserSpaceOnUse,     // absolute in user coordinate system
}

pub enum SpreadMethod {
    Pad,      // use the last stop color beyond the gradient range
    Reflect,  // reflect the gradient
    Repeat,   // repeat the gradient
}
```

#### Population Strategy

The registry is populated during the style resolution pass. When the resolver encounters children of `<defs>`:

```
On encountering <defs> element:
  1. Push special state: "children go to registry, not paint"
  2. For each child of <defs>:
     a. Parse element type and parameters
     b. Extract id attribute (required — used as key)
     c. Create appropriate ReferenceDefinition variant
     d. Call registry.register(id, definition)
     e. Do NOT recurse into children for style resolution
        (their children are part of the definition, not rendered separately)
  3. Pop state — return to normal rendering mode
```

This ensures that:
- `<linearGradient>`, `<radialGradient>`, `<pattern>`, `<clipPath>`, `<mask>`, `<filter>`, `<marker>`, `<symbol>` are stored and never rendered directly
- Their internal child elements (like `<stop>` in a gradient, or shapes in a clipPath) are preserved as part of the definition
- The registry acts as a flat key-value store — no nesting, no hierarchy

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
