# SVG Geometry Attributes — Changes Summary

## Branch: `svg-geometry-attributes`

### Goal
Add DOM element types for `<rect>`, `<circle>`, `<ellipse>` and wire up geometry CSS properties (`cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`) as presentational hints from SVG attributes.

---

## Geometric Attributes per Shape

| Shape | Geometry attributes | CSS properties |
|-------|-------------------|----------------|
| `<rect>` | `x`, `y`, `width`, `height`, `rx`, `ry` | `x`, `y`, `width`, `height`, `rx`, `ry` |
| `<circle>` | `cx`, `cy`, `r` | `cx`, `cy`, `r` |
| `<ellipse>` | `cx`, `cy`, `rx`, `ry` | `cx`, `cy`, `rx`, `ry` |

**New properties to enable in stylo:** `cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`
(`width`/`height` already work via CSS `width`/`height` properties)

---

## Changes

### 1. WebIDL definitions (3 new files)

- `components/script_bindings/webidls/SVGRectElement.webidl`
- `components/script_bindings/webidls/SVGCircleElement.webidl`
- `components/script_bindings/webidls/SVGEllipseElement.webidl`

All inherit from `SVGGraphicsElement`. Animated length attributes are commented out (SVGAnimatedLength not yet implemented in Servo). Pattern follows `SVGImageElement.webidl`.

When the build runs, codegen auto-generates `SVGGraphicsElementTypeId` variants (`SVGRectElement`, `SVGCircleElement`, `SVGEllipseElement`) in `InheritTypes.rs`.

### 2. Rust DOM element files (3 new files)

- `components/script/dom/svg/svgrectelement.rs`
- `components/script/dom/svg/svgcircleelement.rs`
- `components/script/dom/svg/svgellipseelement.rs`

Minimal structs following `SVGImageElement` pattern:
- `#[dom_struct]` with `svggraphicselement: SVGGraphicsElement` field
- `new_inherited()` + `new()` constructors
- `VirtualMethods` impl with `super_type()` → `SVGGraphicsElement`

### 3. Module registration

**`components/script/dom/svg/mod.rs`** — Added `pub(crate) mod` declarations for all 3 new types (alphabetically sorted).

### 4. Element creation routing

**`components/script/dom/create.rs`** — Added match arms in `create_svg_element()`:
```rust
local_name!("circle") => make!(SVGCircleElement),
local_name!("ellipse") => make!(SVGEllipseElement),
local_name!("rect") => make!(SVGRectElement),
```

Previously these fell through to the generic `SVGElement`.

### 5. Vtable dispatch

**`components/script/dom/virtualmethods.rs`** — Added vtable entries for all 3 new `SVGGraphicsElementTypeId` variants:
```rust
SVGGraphicsElementTypeId::SVGCircleElement => SVGCircleElement,
SVGGraphicsElementTypeId::SVGEllipseElement => SVGEllipseElement,
SVGGraphicsElementTypeId::SVGRectElement => SVGRectElement,
```

### 6. Geometry presentational attributes (SVGElement level)

**`components/script/dom/svg/svgelement.rs`**:

**(a) `attribute_affects_presentational_hints`** — Added geometry attributes alongside existing fill/stroke:
```rust
&local_name!("cx") | &local_name!("cy") | &local_name!("r") |
&local_name!("rx") | &local_name!("ry") | &local_name!("x") |
&local_name!("y")
```

**(b) `synthesize_presentational_hints`** (in `LayoutDom` impl) — Added geometry properties using the existing `parse_svg_attribute` function-pointer pattern:
```rust
self.parse_svg_attribute(&parser_context, "cx", longhands::cx::parse_declared, push);
self.parse_svg_attribute(&parser_context, "cy", longhands::cy::parse_declared, push);
// ... r, rx, ry, x, y
```

The `parse_svg_attribute` helper takes `attr_name: &str` and a function pointer `F: for<'i, 't> FnOnce(&ParserContext, &mut Parser<'i, 't>) -> Result<PropertyDeclaration, ParseError<'i>>` — each `longhands::*::parse_declared` matches this signature.

### 7. SVG delegation in element.rs

**`components/script/dom/element/element.rs`** — The old SVGSVGElement-specific width/height handling (using `SVGElementData`) was replaced with a single SVGElement delegation call:
```rust
if let Some(svg_element) = self.downcast::<SVGElement>() {
    svg_element.synthesize_presentational_hints(document, &mut push);
}
```

The SVGSVGElement width/height handling is now inside SVGSVGElement's own `synthesize_presentational_hints` via the `element.downcast::<SVGSVGElement>()` check.

---

## How Presentational Hints Flow

1. **Layout thread** calls `ServoDangerousStyleElement::synthesize_presentational_hints_for_legacy_attributes` → `LayoutDom<Element>::synthesize_presentational_hints_for_legacy_attributes`
2. This function handles XLang, then HTML attributes, then **delegates to SVGElement**
3. `SVGElement::synthesize_presentational_hints` checks for `SVGSVGElement` (width/height), then parses each geometry attribute via `parse_svg_attribute`
4. Each parsed attribute becomes a `PropertyDeclaration` pushed into the declaration block
5. The block is wrapped in `CascadeLevel::PresHints` and added to the applicable declarations for the element

---

## Stylo Dependency

The geometry CSS properties (`cx`, `cy`, `r`, `rx`, `ry`, `x`, `y`) are currently `engine = "gecko"` in `servo/stylo` longhands.toml. To compile, the stylo dependency must point to a fork/rev where these properties have the engine restriction removed.

Required change in stylo fork's `style/properties/longhands.toml`:
```toml
[cx]
# Remove or change: engine = "gecko"
```
