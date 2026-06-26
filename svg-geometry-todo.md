# SVG Geometry Attributes — Branch Overview

## Inheritance Hierarchy

```
Node → Element → SVGElement → SVGGraphicsElement → SVGGeometryElement → {SVGRectElement, SVGCircleElement, SVGEllipseElement}
```

## Servo Changes

### 1. SVGGeometryElement (NEW — intermediate abstract type)

Per spec, `<rect>`, `<circle>`, `<ellipse>` inherit from `SVGGeometryElement`, not directly from `SVGGraphicsElement`.

**`components/script_bindings/webidls/SVGGeometryElement.webidl`**
- `[Exposed=Window, Abstract]` — not directly instantiated
- Inherits from `SVGGraphicsElement`
- `pathLength` + geometry methods commented out (SVGAnimatedNumber not implemented)

**`components/script/dom/svg/svggeometryelement.rs`**
- Wraps `SVGGraphicsElement` field
- `new_inherited` / `new_inherited_with_state` (no `new()` — abstract)
- `VirtualMethods::super_type` → upcast to `SVGGraphicsElement`

### 2. WebIDL definitions (3 files)

```
components/script_bindings/webidls/SVGRectElement.webidl
components/script_bindings/webidls/SVGCircleElement.webidl
components/script_bindings/webidls/SVGEllipseElement.webidl
```

- All inherit from `SVGGeometryElement` (was `SVGGraphicsElement` — corrected)
- Animated length attributes commented out (SVGAnimatedLength not yet implemented in Servo)
- Pattern follows `SVGImageElement.webidl`

### 3. Rust DOM element structs (3 files)

```
components/script/dom/svg/svgrectelement.rs
components/script/dom/svg/svgcircleelement.rs
components/script/dom/svg/svgellipseelement.rs
```

- `#[dom_struct]` wrapping `SVGGeometryElement` field
- `new_inherited()` calls `SVGGeometryElement::new_inherited()`
- `new()` constructor with `Node::reflect_node_with_proto()`
- `VirtualMethods` impl → `super_type()` upcasts to `SVGGeometryElement`

### 4. Module registration

**`components/script/dom/svg/mod.rs`** — added:
```rust
pub(crate) mod svggeometryelement;  // new
```

### 5. Element creation routing

**`components/script/dom/element/create.rs`** — `create_svg_element()`:
```rust
local_name!("circle") => make!(SVGCircleElement),
local_name!("ellipse") => make!(SVGEllipseElement),
local_name!("rect") => make!(SVGRectElement),
```
Previously fell through to generic `SVGElement`.

### 6. Vtable dispatch

**`components/script/dom/node/virtualmethods.rs`** — pattern matches through the full hierarchy:
```rust
SVGElementTypeId::SVGGraphicsElement(
    SVGGraphicsElementTypeId::SVGGeometryElement(
        SVGGeometryElementTypeId::SVGCircleElement,
    ),
)
```

### 7. Presentational hints for geometry attributes

**`components/script/dom/svg/svgelement.rs`**

(a) `attribute_affects_presentational_hints` — SVGElement checks if attribute changes should trigger re-synthesis. Added `cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`.

(b) `synthesize_presentational_hints` (in `LayoutDom` impl) — called by layout thread. Parses each geometry attribute into a CSS property declaration:
```rust
self.parse_svg_attribute(&parser_context, "cx", longhands::cx::parse_declared, push);
self.parse_svg_attribute(&parser_context, "cy", longhands::cy::parse_declared, push);
// ... r, rx, ry, x, y
```
Uses the same `parse_svg_attribute<F>` helper function as fill/stroke properties. Each attribute value is parsed as a CSS value in `ALLOW_UNITLESS_LENGTH | ALLOW_ALL_NUMERIC_VALUES` mode.

(c) `SVGSVGElement` width/height handling — checked first via `element.downcast::<SVGSVGElement>()`, separate from the geometry attributes.

### 8. SVG delegation in element.rs

**`components/script/dom/element/element.rs`** — delegates to `SVGElement`:
```rust
if let Some(svg_element) = self.downcast::<SVGElement>() {
    svg_element.synthesize_presentational_hints(document, &mut push);
}
```
This catches all SVG elements (rect, circle, ellipse, svg, etc.) since they all inherit through SVGElement.

## How Presentational Hints Flow

1. Layout thread calls `synthesize_presentational_hints_for_legacy_attributes`
2. → `LayoutDom<Element>::synthesize_presentational_hints_for_legacy_attributes`
3. Handles XLang → HTML attributes → delegates to **SVGElement** via downcast
4. `SVGElement::synthesize_presentational_hints`: checks SVGSVGElement (width/height), then parses each geometry + fill/stroke attribute
5. Each parsed attribute becomes a `PropertyDeclaration` pushed into the declaration block
6. Block wrapped in `CascadeLevel::PresHints`, added to applicable declarations

## What's NOT Needed at This Stage

- `SVGAnimatedLength` / `SVGAnimatedNumber` types — commented out in WebIDL
- `isPointInFill`, `isPointInStroke`, `getTotalLength`, `getPointAtLength` methods — commented out
- `transform` attribute handling — already on SVGGraphicsElement
- Layout/rendering of the actual shapes — just the DOM types + presentational attributes

## Stylo — TODO

Remove `engine = "gecko"` from geometry properties in `style/properties/longhands.toml`:

| Property | Type |
|----------|------|
| `[cx]` | LengthPercentage |
| `[cy]` | LengthPercentage |
| `[r]` | NonNegativeLengthPercentage |
| `[rx]` | NonNegativeLengthPercentageOrAuto |
| `[ry]` | NonNegativeLengthPercentageOrAuto |
| `[x]` | LengthPercentage |
| `[y]` | LengthPercentage |

Steps:
1. Edit `style/properties/longhands.toml` in stylo fork to remove `engine = "gecko"` from the 7 properties
2. Push to `mu-mostafa98/stylo` branch `enable-svg-styling-for-servo`
3. Update `servo/Cargo.toml` to point to the updated fork
4. Run: `cargo update -p stylo -p stylo_atoms -p stylo_dom -p stylo_malloc_size_of -p stylo_static_prefs -p stylo_traits -p selectors -p servo_arc`
5. Run `./mach build` to verify
